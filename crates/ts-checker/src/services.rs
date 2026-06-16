use ts_evaluator as evaluator;
use ts_printer as printer;

use crate::checker::*;
use crate::jsx::JSX_NAMES_INTRINSIC_ELEMENTS;
use crate::{ast, astnav, core, debug, scanner};

fn node_matches_name(store: &ast::AstStore, left: ast::Node, right: ast::Node) -> bool {
    left == right || store.kind(left) == store.kind(right) && store.text(left) == store.text(right)
}

fn is_external_or_common_js_module_node(checker: &Checker<'_, '_>, node: ast::Node) -> bool {
    checker.source_file_is_external_or_common_js_module(checker.source_file_for_node(node))
}

fn is_global_source_file_node(
    checker: &Checker<'_, '_>,
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    store.kind(node) == ast::Kind::SourceFile
        && !is_external_or_common_js_module_node(checker, node)
}

fn to_checker_symbol(symbol: ast::SymbolIdentity) -> SymbolIdentity {
    SymbolIdentity::from(symbol)
}

fn to_ast_symbol(symbol: SymbolIdentity) -> ast::SymbolIdentity {
    symbol.into()
}

fn to_ast_symbols(symbols: Vec<SymbolIdentity>) -> Vec<ast::SymbolIdentity> {
    symbols.into_iter().map(to_ast_symbol).collect()
}

fn copy_symbol(
    checker: &Checker<'_, '_>,
    symbols: &mut SymbolIdentityTable,
    symbol: SymbolIdentity,
    meaning: ast::SymbolFlags,
) {
    let flags = checker.symbol_identity_combined_local_and_export_flags(symbol);
    if flags & meaning != 0 {
        let id = checker.symbol_identity_name(symbol);
        // We copy reserved-name symbols here; symbols_to_array filters them later.
        symbols.entry(id).or_insert(symbol);
    }
}

fn copy_symbols(
    checker: &Checker<'_, '_>,
    symbols_out: &mut SymbolIdentityTable,
    source: impl IntoIterator<Item = SymbolIdentity>,
    meaning: ast::SymbolFlags,
) {
    if meaning != 0 {
        for symbol in source {
            copy_symbol(checker, symbols_out, symbol, meaning);
        }
    }
}

fn copy_locally_visible_export_symbols(
    checker: &Checker<'_, '_>,
    initial_store: &ast::AstStore,
    symbols_out: &mut SymbolIdentityTable,
    source: impl IntoIterator<Item = SymbolIdentity>,
    meaning: ast::SymbolFlags,
) {
    if meaning == 0 {
        return;
    }
    for symbol in source {
        // Similar condition as in `resolveNameHelper`.
        let locally_visible = checker.with_symbol_identity_declarations(symbol, |declarations| {
            declarations.iter().all(|&declaration| {
                initial_store.kind(declaration) != ast::Kind::ExportSpecifier
                    && initial_store.kind(declaration) != ast::Kind::NamespaceExport
            })
        });
        if locally_visible
            && checker.symbol_identity_name(symbol) != ast::INTERNAL_SYMBOL_NAME_DEFAULT
        {
            copy_symbol(checker, symbols_out, symbol, meaning);
        }
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn symbol_identity_flags(&self, symbol: SymbolIdentity) -> ast::SymbolFlags {
        self.symbol_handle_flags(symbol.symbol_handle())
    }

    pub(crate) fn symbol_identity_check_flags(&self, symbol: SymbolIdentity) -> ast::CheckFlags {
        self.symbol_handle_check_flags(symbol.symbol_handle())
    }

    pub(crate) fn symbol_identity_combined_local_and_export_flags(
        &self,
        symbol: SymbolIdentity,
    ) -> ast::SymbolFlags {
        let handle = symbol.symbol_handle();
        let mut flags = self.symbol_handle_flags(handle);
        if let Some(export_symbol) = self.symbol_handle_export_symbol(handle) {
            flags |= self.symbol_handle_flags(export_symbol);
        }
        flags
    }

    pub(crate) fn symbol_identity_name(&self, symbol: SymbolIdentity) -> ast::SymbolName {
        self.symbol_handle_name(symbol.symbol_handle())
    }

    pub(crate) fn symbol_identity_name_ref(&self, symbol: SymbolIdentity) -> &ast::SymbolName {
        self.symbol_handle_name_ref(symbol.symbol_handle())
    }

    pub(crate) fn collect_symbol_identity_member_table(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Option<SymbolIdentityTable> {
        self.with_symbol_identity_member_table(symbol, |members| {
            members.map(SymbolIdentityTableView::collect)
        })
    }

    pub(crate) fn with_symbol_identity_member_table<R>(
        &self,
        symbol: SymbolIdentity,
        f: impl FnOnce(Option<SymbolIdentityTableView<'_>>) -> R,
    ) -> R {
        self.with_symbol_handle_members(symbol.symbol_handle(), |members| {
            f(members.map(SymbolIdentityTableView::Handles))
        })
    }

    pub(crate) fn symbol_identity_member_len(&self, symbol: SymbolIdentity) -> usize {
        self.with_symbol_handle_members(symbol.symbol_handle(), |members| {
            members.map_or(0, ast::SymbolHandleTable::len)
        })
    }

    pub(crate) fn symbol_identity_member_at(
        &self,
        symbol: SymbolIdentity,
        index: usize,
    ) -> Option<SymbolIdentity> {
        self.with_symbol_handle_members(symbol.symbol_handle(), |members| {
            members.and_then(|members| {
                members
                    .get_index(index)
                    .map(|(_, &symbol)| SymbolIdentity::from_symbol_handle(symbol))
            })
        })
    }

    pub(crate) fn lookup_symbol_identity_member(
        &self,
        symbol: SymbolIdentity,
        name: &str,
    ) -> Option<SymbolIdentity> {
        self.lookup_symbol_handle_member(symbol.symbol_handle(), name)
            .map(SymbolIdentity::from_symbol_handle)
    }

    pub(crate) fn collect_symbol_identity_export_table(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Option<SymbolIdentityTable> {
        self.with_symbol_identity_export_table(symbol, |exports| {
            exports.map(SymbolIdentityTableView::collect)
        })
    }

    pub(crate) fn with_symbol_identity_export_table<R>(
        &self,
        symbol: SymbolIdentity,
        f: impl FnOnce(Option<SymbolIdentityTableView<'_>>) -> R,
    ) -> R {
        self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
            f(exports.map(|exports| match exports {
                SymbolHandleExportsView::Handles(exports) => {
                    SymbolIdentityTableView::Handles(exports)
                }
                SymbolHandleExportsView::Globals(exports) => {
                    SymbolIdentityTableView::Globals(exports)
                }
            }))
        })
    }

    pub(crate) fn symbol_identity_export_len(&self, symbol: SymbolIdentity) -> usize {
        self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
            exports.map_or(0, |exports| match exports {
                SymbolHandleExportsView::Handles(exports) => exports.len(),
                SymbolHandleExportsView::Globals(exports) => exports.len(),
            })
        })
    }

    pub(crate) fn symbol_identity_export_at(
        &self,
        symbol: SymbolIdentity,
        index: usize,
    ) -> Option<SymbolIdentity> {
        self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
            exports.and_then(|exports| match exports {
                SymbolHandleExportsView::Handles(exports) => exports
                    .get_index(index)
                    .map(|(_, &symbol)| SymbolIdentity::from_symbol_handle(symbol)),
                SymbolHandleExportsView::Globals(exports) => {
                    exports.get_index(index).map(|(_, symbol)| symbol)
                }
            })
        })
    }

    pub(crate) fn lookup_symbol_identity_export(
        &self,
        symbol: SymbolIdentity,
        name: &str,
    ) -> Option<SymbolIdentity> {
        self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
            exports.and_then(|exports| match exports {
                SymbolHandleExportsView::Handles(exports) => exports
                    .get(name)
                    .copied()
                    .map(SymbolIdentity::from_symbol_handle),
                SymbolHandleExportsView::Globals(exports) => exports.get(name),
            })
        })
    }

    pub(crate) fn symbol_identity_exports_are_empty(&self, symbol: SymbolIdentity) -> bool {
        self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
            exports.is_none_or(SymbolHandleExportsView::is_empty)
        })
    }

    pub(crate) fn symbol_identity_export_symbol(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        self.symbol_handle_export_symbol(symbol.symbol_handle())
            .map(SymbolIdentity::from_symbol_handle)
    }

    pub(crate) fn symbol_identities_to_array(
        &self,
        symbols: &SymbolIdentityTable,
    ) -> Vec<SymbolIdentity> {
        let mut result = Vec::new();
        for (id, &symbol) in symbols {
            if !is_reserved_member_name(id) {
                result.push(symbol);
            }
        }
        result
    }

    pub(crate) fn get_symbol_identity(
        &mut self,
        symbols: &SymbolIdentityTable,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<SymbolIdentity> {
        self.get_symbol_identity_from_raw(symbols.get(name).copied(), meaning)
    }

    pub(crate) fn get_symbol_identity_from_raw(
        &mut self,
        symbol: Option<SymbolIdentity>,
        meaning: ast::SymbolFlags,
    ) -> Option<SymbolIdentity> {
        if !meaning.intersects(ast::SYMBOL_FLAGS_ALL) {
            return None;
        }
        let symbol = self.get_merged_symbol_identity(symbol)?;
        let flags = self.symbol_identity_flags(symbol);
        if flags.intersects(meaning) {
            return Some(symbol);
        }
        if flags.intersects(ast::SYMBOL_FLAGS_ALIAS) {
            let target = self.resolve_alias_identity(symbol);
            if self.symbol_identity_flags(target).intersects(meaning) {
                return Some(symbol);
            }
        }
        None
    }

    pub(crate) fn node_local_len(&self, node: ast::Node) -> usize {
        let Some(source_file) = self.try_source_file_for_node(node) else {
            return 0;
        };
        if node.store_id() != source_file.store().store_id() {
            return 0;
        }
        self.source_file_binding_state(source_file).locals_len(node)
    }

    pub(crate) fn node_local_handle_at(
        &self,
        node: ast::Node,
        index: usize,
    ) -> Option<ast::SymbolHandle> {
        let source_file = self.try_source_file_for_node(node)?;
        if node.store_id() != source_file.store().store_id() {
            return None;
        }
        self.source_file_binding_state(source_file)
            .local_at(node, index)
    }

    pub(crate) fn node_local_entry_at(
        &self,
        node: ast::Node,
        index: usize,
    ) -> Option<(ast::SymbolName, ast::SymbolHandle)> {
        let source_file = self.try_source_file_for_node(node)?;
        if node.store_id() != source_file.store().store_id() {
            return None;
        }
        self.source_file_binding_state(source_file)
            .local_entry_at(node, index)
    }

    pub(crate) fn global_symbol_identity_len(&self) -> usize {
        self.semantic_state.global_symbol_identity_len()
    }

    pub(crate) fn global_symbol_identity_at(&self, index: usize) -> Option<SymbolIdentity> {
        self.semantic_state.global_symbol_identity_at(index)
    }

    pub(crate) fn global_symbol_identity_entry_at(
        &self,
        index: usize,
    ) -> Option<(ast::SymbolName, SymbolIdentity)> {
        self.semantic_state.global_symbol_identity_entry_at(index)
    }

    fn structured_type_member_len(&mut self, t: TypeHandle) -> usize {
        self.resolve_structured_type_members(t).members.len()
    }

    fn structured_type_member_at(
        &mut self,
        t: TypeHandle,
        index: usize,
    ) -> Option<(ast::SymbolName, SymbolIdentity)> {
        self.resolve_structured_type_members(t)
            .members
            .get_index(index)
            .map(|(name, &symbol)| (name.clone(), symbol))
    }

    pub fn get_symbols_in_scope_public(
        &mut self,
        location: ast::Node,
        meaning: ast::SymbolFlags,
    ) -> Vec<ast::SymbolIdentity> {
        let location = location;
        to_ast_symbols(self.get_symbols_in_scope(location, meaning))
    }

    fn get_symbols_in_scope(
        &mut self,
        mut location: ast::Node,
        meaning: ast::SymbolFlags,
    ) -> Vec<SymbolIdentity> {
        let initial_store = self.store_for_node(location);
        if initial_store.flags(location) & ast::NODE_FLAGS_IN_WITH_STATEMENT != 0 {
            // We cannot answer semantic questions within a with block, do not proceed any further
            return Vec::new();
        }

        let mut symbols = SymbolIdentityTable::default();
        let mut is_static_symbol = false;

        while let Some(current) = Some(location) {
            let store = self.store_for_node(current);
            if can_have_locals(store, current) && !is_global_source_file_node(self, store, current)
            {
                self.with_node_locals(current, |locals| {
                    copy_symbols(
                        self,
                        &mut symbols,
                        locals
                            .values()
                            .copied()
                            .map(SymbolIdentity::from_symbol_handle),
                        meaning,
                    );
                });
            }
            match store.kind(current) {
                ast::Kind::SourceFile => {
                    if is_external_or_common_js_module_node(self, current) {
                        let symbol = self.get_symbol_of_declaration(current).unwrap();
                        let symbol = SymbolIdentity::from_symbol_handle(symbol);
                        self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
                            if let Some(exports) = exports {
                                exports.for_each_handle(|_, symbol| {
                                    copy_locally_visible_export_symbols(
                                        self,
                                        initial_store,
                                        &mut symbols,
                                        std::iter::once(SymbolIdentity::from_symbol_handle(symbol)),
                                        meaning & ast::SYMBOL_FLAGS_MODULE_MEMBER,
                                    );
                                });
                            }
                        });
                    }
                }
                ast::Kind::ModuleDeclaration => {
                    let symbol = self.get_symbol_of_declaration(current).unwrap();
                    let symbol = SymbolIdentity::from_symbol_handle(symbol);
                    self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
                        if let Some(exports) = exports {
                            exports.for_each_handle(|_, symbol| {
                                copy_locally_visible_export_symbols(
                                    self,
                                    initial_store,
                                    &mut symbols,
                                    std::iter::once(SymbolIdentity::from_symbol_handle(symbol)),
                                    meaning & ast::SYMBOL_FLAGS_MODULE_MEMBER,
                                );
                            });
                        }
                    });
                }
                ast::Kind::EnumDeclaration => {
                    let symbol = self.get_symbol_of_declaration(current).unwrap();
                    let symbol = SymbolIdentity::from_symbol_handle(symbol);
                    self.with_symbol_handle_exports(symbol.symbol_handle(), |exports| {
                        if let Some(exports) = exports {
                            exports.for_each_handle(|_, symbol| {
                                copy_symbol(
                                    self,
                                    &mut symbols,
                                    SymbolIdentity::from_symbol_handle(symbol),
                                    meaning & ast::SYMBOL_FLAGS_ENUM_MEMBER,
                                );
                            });
                        }
                    });
                }
                ast::Kind::ClassExpression => {
                    if store.name(current).is_some() {
                        if let Some(symbol) = self.get_symbol_of_declaration(current) {
                            let symbol = SymbolIdentity::from_symbol_handle(symbol);
                            copy_symbol(self, &mut symbols, symbol, meaning);
                        }
                    }
                    if !is_static_symbol {
                        let symbol = self.get_symbol_of_declaration(current).unwrap();
                        let symbol = SymbolIdentity::from_symbol_handle(symbol);
                        self.with_symbol_handle_members(symbol.symbol_handle(), |members| {
                            if let Some(members) = members {
                                copy_symbols(
                                    self,
                                    &mut symbols,
                                    members
                                        .values()
                                        .copied()
                                        .map(SymbolIdentity::from_symbol_handle),
                                    meaning & ast::SYMBOL_FLAGS_TYPE,
                                );
                            }
                        });
                    }
                }
                ast::Kind::ClassDeclaration | ast::Kind::InterfaceDeclaration => {
                    if !is_static_symbol {
                        let symbol = self.get_symbol_of_declaration(current).unwrap();
                        let symbol = SymbolIdentity::from_symbol_handle(symbol);
                        self.with_symbol_handle_members(symbol.symbol_handle(), |members| {
                            if let Some(members) = members {
                                copy_symbols(
                                    self,
                                    &mut symbols,
                                    members
                                        .values()
                                        .copied()
                                        .map(SymbolIdentity::from_symbol_handle),
                                    meaning & ast::SYMBOL_FLAGS_TYPE,
                                );
                            }
                        });
                    }
                }
                ast::Kind::FunctionExpression => {
                    if store.name(current).is_some() {
                        if let Some(symbol) = self.get_symbol_of_declaration(current) {
                            let symbol = SymbolIdentity::from_symbol_handle(symbol);
                            copy_symbol(self, &mut symbols, symbol, meaning);
                        }
                    }
                }
                _ => {}
            }
            if introduces_arguments_exotic_object(store, current) {
                let arguments_symbol = self.arguments_symbol_identity();
                copy_symbol(self, &mut symbols, arguments_symbol, meaning);
            }
            is_static_symbol = ast::is_static(store, current);
            if store.parent(current).is_none() {
                break;
            }
            let parent = store.parent(current).unwrap();
            location = parent;
        }
        for index in 0..self.semantic_state.global_symbol_identity_len() {
            if let Some(symbol) = self.semantic_state.global_symbol_identity_at(index) {
                copy_symbol(self, &mut symbols, symbol, meaning);
            }
        }
        symbols.shift_remove(ast::INTERNAL_SYMBOL_NAME_THIS); // Not a symbol, a keyword
        self.symbol_identities_to_array(&symbols)
    }

    pub fn get_exports_of_module_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<ast::SymbolIdentity> {
        let exports = self.collect_exports_of_module_identity(to_checker_symbol(symbol));
        to_ast_symbols(self.symbol_identities_to_array(&exports))
    }

    pub(crate) fn for_each_export_and_property_of_module(
        &mut self,
        module_symbol: SymbolIdentity,
        mut cb: impl FnMut(SymbolIdentity, String),
    ) {
        for (key, &exported_symbol) in self
            .collect_exports_of_module_identity(module_symbol)
            .iter()
        {
            if !is_reserved_member_name(&key) {
                cb(exported_symbol, key.to_string());
            }
        }
        let export_equals = self.resolve_external_module_symbol_identity(
            module_symbol,
            false, /*dontResolveAlias*/
        );
        if self.same_optional_symbol_identity(export_equals, Some(module_symbol)) {
            return;
        }
        let Some(export_equals) = export_equals else {
            return;
        };
        let type_of_symbol = self.get_type_of_symbol_identity_at_location(export_equals, None);
        if !self.should_treat_properties_of_external_module_as_exports(type_of_symbol) {
            return;
        }
        // forEachPropertyOfType
        let reduced_type = self.get_reduced_apparent_type(type_of_symbol);
        if self.type_flags(reduced_type) & TYPE_FLAGS_STRUCTURED_TYPE == 0 {
            return;
        }
        let member_len = self.structured_type_member_len(reduced_type);
        for index in 0..member_len {
            let Some((name, symbol_identity)) = self.structured_type_member_at(reduced_type, index)
            else {
                continue;
            };
            let is_named_member = !is_reserved_member_name(&name)
                && self
                    .symbol_identity_flags(symbol_identity)
                    .intersects(ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_ALIAS);
            if is_named_member {
                cb(symbol_identity, name.to_string());
            }
        }
    }

    pub fn is_valid_property_access_public(
        &mut self,
        node: ast::Node,
        property_name: &str,
    ) -> bool {
        let node = node;
        self.is_valid_property_access(node, property_name)
    }

    fn is_valid_property_access(&mut self, node: ast::Node, property_name: &str) -> bool {
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::PropertyAccessExpression => {
                let expression = store.expression(node).unwrap();
                let is_super = store.kind(expression) == ast::Kind::SuperKeyword;
                let expression_type = self.check_expression(expression);
                let widened_type = self.get_widened_type(expression_type);
                self.is_valid_property_access_with_type(node, is_super, property_name, widened_type)
            }
            ast::Kind::QualifiedName => {
                let left = store.left(node).unwrap();
                let left_type = self.check_expression(left);
                let widened_type = self.get_widened_type(left_type);
                self.is_valid_property_access_with_type(
                    node,
                    false, /*isSuper*/
                    property_name,
                    widened_type,
                )
            }
            ast::Kind::ImportType => {
                let node_type = self.get_type_from_type_node(node);
                self.is_valid_property_access_with_type(
                    node,
                    false, /*isSuper*/
                    property_name,
                    node_type,
                )
            }
            _ => panic!(
                "Unexpected node kind in isValidPropertyAccess: {}",
                store.kind(node).to_string()
            ),
        }
    }

    fn is_valid_property_access_with_type(
        &mut self,
        node: ast::Node,
        is_super: bool,
        property_name: &str,
        t: TypeHandle,
    ) -> bool {
        // Short-circuiting for improved performance.
        if is_type_any(self, Some(t)) {
            return true;
        }
        let prop = self.get_property_of_type(t, property_name);
        if let Some(prop) = prop {
            self.is_property_accessible_identity(node, is_super, false /*isWrite*/, t, prop)
        } else {
            false
        }
    }

    // Checks if an existing property access is valid for completions purposes.
    // node: a property access-like node where we want to check if we can access a property.
    // This node does not need to be an access of the property we are checking.
    // e.g. in completions, this node will often be an incomplete property access node, as in `foo.`.
    // Besides providing a location (i.e. scope) used to check property accessibility, we use this node for
    // computing whether this is a `super` property access.
    // type: the type whose property we are checking.
    // property: the accessed property's symbol.
    pub fn is_valid_property_access_for_completions_public(
        &mut self,
        node: ast::Node,
        t: TypeHandle,
        property: ast::SymbolIdentity,
    ) -> bool {
        let property = to_checker_symbol(property);
        let store = self.store_for_node(node);
        self.is_property_accessible_identity(
            node,
            store.kind(node) == ast::Kind::PropertyAccessExpression
                && store
                    .expression(node)
                    .is_some_and(|expression| store.kind(expression) == ast::Kind::SuperKeyword),
            false, /*isWrite*/
            t,
            property,
        )
        // Previously we validated the 'this' type of methods but this adversely affected performance. See #31377 for more context.
    }

    pub fn get_all_possible_properties_of_types(
        &mut self,
        types: Vec<TypeHandle>,
    ) -> Vec<ast::SymbolIdentity> {
        let union_type = self.get_union_type(types.clone());
        if self.type_flags(union_type) & TYPE_FLAGS_UNION == 0 {
            return to_ast_symbols(self.get_augmented_properties_of_type(union_type));
        }
        let mut props = SymbolIdentityTable::default();
        for member_type in types {
            for p in self.get_augmented_properties_of_type(member_type) {
                let p_name = self.symbol_identity_name(p);
                if !props.contains_key(&p_name) {
                    if let Some(prop) = self.create_union_or_intersection_property(
                        union_type, &p_name, false, /*skipObjectFunctionPropertyAugment*/
                    ) {
                        props.insert(p_name, prop);
                    }
                }
            }
        }
        to_ast_symbols(self.symbol_identities_to_array(&props))
    }

    pub fn is_unknown_symbol(&self, symbol: ast::SymbolIdentity) -> bool {
        to_checker_symbol(symbol) == self.unknown_symbol_identity()
    }

    pub fn is_undefined_symbol(&self, symbol: ast::SymbolIdentity) -> bool {
        to_checker_symbol(symbol) == self.undefined_symbol_identity()
    }

    pub fn is_arguments_symbol(&self, symbol: ast::SymbolIdentity) -> bool {
        to_checker_symbol(symbol) == self.arguments_symbol_identity()
    }

    // Originally from services.ts
    pub(crate) fn get_non_optional_type(&mut self, t: TypeHandle) -> TypeHandle {
        self.remove_optional_type_marker(t)
    }

    pub(crate) fn get_string_index_type(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        self.get_index_type_of_type(t, self.semantic_state.semantic_handles().string_type)
    }

    pub(crate) fn get_number_index_type(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        self.get_index_type_of_type(t, self.semantic_state.semantic_handles().number_type)
    }

    pub fn get_element_type_of_array_type_public(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        self.get_element_type_of_array_type(t)
    }

    pub fn get_call_signatures(&mut self, t: TypeHandle) -> Vec<SignatureHandle> {
        self.get_signatures_of_type(t, SIGNATURE_KIND_CALL)
    }

    pub fn get_construct_signatures(&mut self, t: TypeHandle) -> Vec<SignatureHandle> {
        self.get_signatures_of_type(t, SIGNATURE_KIND_CONSTRUCT)
    }

    pub fn get_apparent_properties(&mut self, t: TypeHandle) -> Vec<ast::SymbolIdentity> {
        to_ast_symbols(self.get_augmented_properties_of_type(t))
    }

    pub(crate) fn get_property_identities_of_type_for_services(
        &mut self,
        t: TypeHandle,
    ) -> Vec<SymbolIdentity> {
        self.get_property_identities_of_type(t)
    }

    fn get_augmented_properties_of_type(&mut self, mut t: TypeHandle) -> Vec<SymbolIdentity> {
        t = self.get_apparent_type(t);
        let properties = self.get_property_identities_of_type_for_services(t);
        let mut props_by_name = create_symbol_identity_table_for_services(self, properties);
        let mut function_type = None;
        if !self
            .get_signatures_of_type(t, SIGNATURE_KIND_CALL)
            .is_empty()
        {
            function_type = Some(
                self.semantic_state
                    .semantic_handles()
                    .global_callable_function_type,
            );
        } else if !self
            .get_signatures_of_type(t, SIGNATURE_KIND_CONSTRUCT)
            .is_empty()
        {
            function_type = Some(
                self.semantic_state
                    .semantic_handles()
                    .global_newable_function_type,
            );
        }
        if let Some(function_type) = function_type {
            for p in self.get_property_identities_of_type_for_services(function_type) {
                let p_name = self.symbol_identity_name(p);
                props_by_name.entry(p_name).or_insert(p);
            }
        }
        get_named_member_identities_for_services(self, &props_by_name)
    }

    pub fn try_get_member_in_module_exports_and_properties(
        &mut self,
        member_name: &str,
        module_symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        let module_symbol = to_checker_symbol(module_symbol);
        let symbol = self.try_get_member_in_module_exports(member_name, module_symbol);
        if symbol.is_some() {
            return symbol.map(to_ast_symbol);
        }
        let export_equals = self.resolve_external_module_symbol_identity(
            module_symbol,
            false, /*dontResolveAlias*/
        );
        if self.same_optional_symbol_identity(export_equals, Some(module_symbol)) {
            return None;
        }
        let export_equals = export_equals?;
        let t = self.get_type_of_symbol_identity_at_location(export_equals, None);
        if self.should_treat_properties_of_external_module_as_exports(t) {
            return self.get_property_of_type(t, member_name).map(to_ast_symbol);
        }
        None
    }

    pub(crate) fn try_get_member_in_module_exports(
        &mut self,
        member_name: &str,
        module_symbol: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        self.lookup_symbol_identity_export(module_symbol, member_name)
    }

    fn should_treat_properties_of_external_module_as_exports(
        &mut self,
        resolved_external_module_type: TypeHandle,
    ) -> bool {
        self.type_flags(resolved_external_module_type) & TYPE_FLAGS_PRIMITIVE == 0
            || self.object_flags(resolved_external_module_type) & OBJECT_FLAGS_CLASS != 0
            // `isArrayOrTupleLikeType` is too expensive to use in this auto-imports hot path.
            || self.is_array_type(resolved_external_module_type)
            || self.is_tuple_type(resolved_external_module_type)
    }

    pub fn get_contextual_type_public(
        &mut self,
        node: ast::Expression,
        context_flags: ContextFlags,
    ) -> Option<TypeHandle> {
        let node = node;
        if context_flags & CONTEXT_FLAGS_IGNORE_NODE_INFERENCES != 0 {
            return run_with_inference_blocked_from_source_node(self, node, |c| {
                c.get_contextual_type(node, context_flags)
            });
        }
        self.get_contextual_type(node, context_flags)
    }

    pub(crate) fn get_root_symbols(&mut self, symbol: SymbolIdentity) -> Vec<SymbolIdentity> {
        let roots = self.get_immediate_root_symbols(symbol);
        if roots.is_empty() {
            return vec![symbol];
        }
        let mut result = Vec::new();
        for root in roots {
            result.extend(self.get_root_symbols(root));
        }
        result
    }

    pub fn get_root_symbols_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Vec<ast::SymbolIdentity> {
        to_ast_symbols(self.get_root_symbols(to_checker_symbol(symbol)))
    }

    pub(crate) fn get_mapped_type_symbol_of_property(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        self.semantic_state
            .try_value_symbol_containing_type(symbol)
            .and_then(|containing_type| self.type_symbol_identity(containing_type))
    }

    pub fn get_mapped_type_symbol_of_property_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        self.get_mapped_type_symbol_of_property(to_checker_symbol(symbol))
            .map(to_ast_symbol)
    }

    fn get_immediate_root_symbols(&mut self, symbol: SymbolIdentity) -> Vec<SymbolIdentity> {
        if self.symbol_identity_check_flags(symbol) & ast::CHECK_FLAGS_SYNTHETIC != 0 {
            let Some(containing_type) = self.semantic_state.value_symbol_containing_type(symbol)
            else {
                return Vec::new();
            };
            let name = self.symbol_identity_name(symbol);
            let mut roots = Vec::new();
            for t in self.type_types(containing_type) {
                if let Some(symbol) = self.get_property_of_type(t, &name) {
                    roots.push(symbol);
                }
            }
            return roots;
        }
        if self.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_TRANSIENT != 0 {
            if self.semantic_state.has_spread_link(symbol) {
                if let (Some(left), Some(right)) = self.spread_symbols(symbol) {
                    return vec![left, right];
                }
            }
            if self.semantic_state.has_mapped_symbol_link(symbol) {
                if let Some(synthetic_origin) = self.mapped_synthetic_origin_symbol(symbol) {
                    return vec![synthetic_origin];
                }
            }
            if let Some(target) = self.try_get_target(symbol) {
                return vec![target];
            }
        }
        Vec::new()
    }

    fn try_get_target(&mut self, symbol: SymbolIdentity) -> Option<SymbolIdentity> {
        let mut target = None;
        let mut next = Some(symbol);
        while let Some(n) = next.take() {
            if self.semantic_state.has_value_symbol_link(n) {
                next = self.value_symbol_target(n);
            } else if self.semantic_state.has_export_type_link(n) {
                next = self.export_type_target_symbol(n);
            } else {
                next = None;
            }
            if let Some(n) = next.as_ref() {
                target = Some(*n);
            }
        }
        target
    }

    pub fn get_export_symbol_public(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<ast::SymbolIdentity> {
        let symbol = to_checker_symbol(symbol);
        let export_symbol = self.symbol_identity_export_symbol(symbol).unwrap_or(symbol);
        self.get_merged_symbol_identity(Some(export_symbol))
            .map(to_ast_symbol)
    }

    pub(crate) fn get_export_specifier_local_target_symbol(
        &mut self,
        node: ast::Node,
    ) -> Option<SymbolIdentity> {
        // node should be ExportSpecifier | Identifier
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::ExportSpecifier => {
                let export_declaration = store.parent(node).and_then(|parent| store.parent(parent));
                if export_declaration
                    .is_some_and(|declaration| store.module_specifier(declaration).is_some())
                {
                    let export_declaration = export_declaration.unwrap();
                    return self.get_external_module_member(
                        export_declaration,
                        node,
                        false, /*dontResolveAlias*/
                    );
                }
                let name = store.property_name_or_name(node).unwrap();
                if store.kind(name) == ast::Kind::StringLiteral {
                    // Skip for invalid syntax like this: export { "x" }
                    return None;
                }
                self.resolve_entity_name(
                    name,
                    ast::SYMBOL_FLAGS_VALUE
                        | ast::SYMBOL_FLAGS_TYPE
                        | ast::SYMBOL_FLAGS_NAMESPACE
                        | ast::SYMBOL_FLAGS_ALIAS,
                    true, /*ignoreErrors*/
                    false,
                    None,
                )
            }
            ast::Kind::Identifier => self.resolve_entity_name(
                node,
                ast::SYMBOL_FLAGS_VALUE
                    | ast::SYMBOL_FLAGS_TYPE
                    | ast::SYMBOL_FLAGS_NAMESPACE
                    | ast::SYMBOL_FLAGS_ALIAS,
                true, /*ignoreErrors*/
                false,
                None,
            ),
            _ => panic!(
                "Unhandled case in getExportSpecifierLocalTargetSymbol, node should be ExportSpecifier | Identifier"
            ),
        }
    }

    pub fn get_export_specifier_local_target_symbol_public(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::SymbolIdentity> {
        self.get_export_specifier_local_target_symbol(node)
            .map(to_ast_symbol)
    }

    pub(crate) fn get_shorthand_assignment_value_symbol(
        &mut self,
        location: Option<ast::Node>,
    ) -> Option<SymbolIdentity> {
        if let Some(location) = location {
            let store = self.store_for_node(location);
            if store.kind(location) == ast::Kind::ShorthandPropertyAssignment {
                let name = store.name(location).unwrap();
                return self.resolve_entity_name(
                    name,
                    ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_ALIAS,
                    true, /*ignoreErrors*/
                    false,
                    None,
                );
            }
        }
        None
    }

    pub fn get_shorthand_assignment_value_symbol_identity_public(
        &mut self,
        location: Option<ast::Node>,
    ) -> Option<ast::SymbolIdentity> {
        self.get_shorthand_assignment_value_symbol(location)
            .map(to_ast_symbol)
    }

    /**
     * Get symbols that represent parameter-property-declaration as parameter and as property declaration
     * @param parameter a parameterDeclaration node
     * @param parameterName a name of the parameter to get the symbols for.
     * @return a tuple of two symbols
     */
    pub(crate) fn get_symbols_of_parameter_property_declaration(
        &mut self,
        parameter: ast::Node, /*ParameterPropertyDeclaration*/
        parameter_name: &str,
    ) -> (SymbolIdentity, SymbolIdentity) {
        let store = self.store_for_node(parameter);
        let constructor_declaration = store.parent(parameter).unwrap();
        let class_declaration = store.parent(constructor_declaration).unwrap();
        let parameter_symbol = self
            .lookup_node_local(
                constructor_declaration,
                parameter_name,
                ast::SYMBOL_FLAGS_VALUE,
            )
            .map(SymbolIdentity::from_symbol_handle);
        let class_symbol = self.get_symbol_of_declaration(class_declaration).unwrap();
        let property_symbol = self
            .lookup_symbol_handle_member(class_symbol, parameter_name)
            .and_then(|symbol| {
                self.merged_symbol_handle_matches_meaning(symbol, ast::SYMBOL_FLAGS_VALUE)
            })
            .map(SymbolIdentity::from_symbol_handle);
        if parameter_symbol.is_some() && property_symbol.is_some() {
            return (parameter_symbol.unwrap(), property_symbol.unwrap());
        }
        panic!(
            "There should exist two symbols, one as property declaration and one as parameter declaration"
        );
    }

    // IsDeclarationUsed checks if an import declaration identifier is used in the source file.
    // This is primarily used for organizing imports to determine which imports can be removed.
    pub fn is_declaration_used(
        &mut self,
        source_file: &'a ast::SourceFile,
        identifier: ast::Node,
        jsx_elements_present: bool,
        jsx_mode_needs_explicit_import: bool,
    ) -> bool {
        let store = source_file.store();
        if jsx_elements_present && jsx_mode_needs_explicit_import {
            let source_file_node = source_file.root();
            let jsx_namespace = self.get_jsx_namespace(Some(source_file_node));
            let jsx_fragment_factory = self.get_jsx_fragment_factory(source_file_node);
            let identifier_text = store.text(identifier);
            if identifier_text == jsx_namespace {
                return true;
            }
            if !jsx_fragment_factory.is_empty() && identifier_text == jsx_fragment_factory {
                return true;
            }
        }
        let symbol = self.get_symbol_at_location_for_public_api(identifier);
        if symbol.is_none() {
            return true;
        }
        self.is_symbol_referenced_in_file(source_file, identifier, symbol.unwrap())
    }

    // IsSymbolReferencedInFile checks if a symbol is referenced in the source file (besides its definition).
    // This is used as a quick check for whether a symbol is used at all in a file.
    pub(crate) fn is_symbol_referenced_in_file(
        &mut self,
        source_file: &'a ast::SourceFile,
        definition: ast::Node,
        symbol: SymbolIdentity,
    ) -> bool {
        let store = source_file.store();
        let identifier_text = store.text(definition);
        let source_file_node = source_file.root();
        for token in
            get_possible_symbol_reference_nodes(source_file, &identifier_text, source_file_node)
        {
            if !ast::is_identifier(store, token) {
                continue;
            }
            if token == definition || store.text(token) != identifier_text {
                continue;
            }
            let ref_symbol = self.get_symbol_at_location_for_public_api(token);
            if self.same_optional_symbol_identity(ref_symbol, Some(symbol)) {
                return true;
            }
            let token_parent = store.parent(token);
            if token_parent
                .is_some_and(|parent| store.kind(parent) == ast::Kind::ShorthandPropertyAssignment)
            {
                let parent = token_parent.unwrap();
                let shorthand_symbol = self.get_shorthand_assignment_value_symbol(Some(parent));
                if self.same_optional_symbol_identity(shorthand_symbol, Some(symbol)) {
                    return true;
                }
            }
            let token_parent = store.parent(token);
            if token_parent.is_some_and(|parent| ast::is_export_specifier(store, parent)) {
                let parent = token_parent.unwrap();
                let local_symbol =
                    self.get_local_symbol_for_export_specifier(token, ref_symbol, parent);
                if self.same_optional_symbol_identity(local_symbol, Some(symbol)) {
                    return true;
                }
            }
        }
        false
    }

    fn get_local_symbol_for_export_specifier(
        &mut self,
        reference_location: ast::Node,
        reference_symbol: Option<SymbolIdentity>,
        export_specifier: ast::Node,
    ) -> Option<SymbolIdentity> {
        let store = self.store_for_node(export_specifier);
        if is_export_specifier_alias(store, reference_location, export_specifier) {
            if let Some(symbol) = self.get_export_specifier_local_target_symbol(export_specifier) {
                return Some(symbol);
            }
        }
        reference_symbol
    }

    pub fn get_type_argument_constraint_public(&mut self, node: ast::Node) -> Option<TypeHandle> {
        if !ast::is_type_node(self.store_for_node(node), node) {
            return None;
        }
        self.get_type_argument_constraint(node)
    }

    // getUninstantiatedSignatures gets generic signatures from the function's/constructor's type.
    fn get_uninstantiated_signatures(&mut self, node: ast::Node) -> Vec<SignatureHandle> {
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::CallExpression | ast::Kind::Decorator => {
                let expression_type = self.get_type_of_expression(store.expression(node).unwrap());
                self.get_signatures_of_type(expression_type, SIGNATURE_KIND_CALL)
            }
            ast::Kind::NewExpression => {
                let expression_type = self.get_type_of_expression(store.expression(node).unwrap());
                self.get_signatures_of_type(expression_type, SIGNATURE_KIND_CONSTRUCT)
            }
            ast::Kind::JsxSelfClosingElement | ast::Kind::JsxOpeningElement => {
                let tag_name = store.tag_name(node).unwrap();
                if is_jsx_intrinsic_tag_name(store, tag_name) {
                    Vec::new()
                } else {
                    let tag_type = self.get_type_of_expression(tag_name);
                    self.get_signatures_of_type(tag_type, SIGNATURE_KIND_CALL)
                }
            }
            ast::Kind::TaggedTemplateExpression => {
                let tag = store.tag(node).unwrap();
                let tag_type = self.get_type_of_expression(tag);
                self.get_signatures_of_type(tag_type, SIGNATURE_KIND_CALL)
            }
            ast::Kind::BinaryExpression | ast::Kind::JsxOpeningFragment => Vec::new(),
            _ => Vec::new(),
        }
    }

    fn get_type_parameter_constraint_for_position_across_signatures(
        &mut self,
        signatures: Vec<SignatureHandle>,
        position: usize,
    ) -> TypeHandle {
        let mut relevant_constraints = Vec::new();
        for signature in signatures {
            let type_parameters = self.signature_record(signature).type_parameters.clone();
            if position >= type_parameters.len() {
                continue;
            }
            let relevant_type_parameter = type_parameters[position];
            if let Some(relevant_constraint) =
                self.get_constraint_of_type_parameter(relevant_type_parameter)
            {
                relevant_constraints.push(relevant_constraint);
            }
        }
        self.get_union_type(relevant_constraints)
    }

    fn get_type_argument_constraint(&mut self, node: ast::Node) -> Option<TypeHandle> {
        let mut type_argument_position: isize = -1;
        let store = self.store_for_node(node);
        let node_parent = store.parent(node).unwrap();
        if ast::has_type_arguments(store, &node_parent) {
            let type_args = store.type_arguments(node_parent);
            for (i, arg) in type_args.into_iter().flatten().enumerate() {
                if node_matches_name(store, arg, node) {
                    type_argument_position = i as isize;
                    break;
                }
            }
        }
        if type_argument_position >= 0 {
            let pos = type_argument_position as usize;
            // The node could be a type argument of a call, a `new` expression, a decorator, an
            // instantiation expression, or a generic type instantiation.
            let parent = store.parent(node).unwrap();
            if ast::is_call_like_expression(store, &parent) {
                // PORT NOTE: reshaped for borrowck; TS-Go evaluates signatures before passing them.
                let signatures = self.get_uninstantiated_signatures(parent);
                return Some(
                    self.get_type_parameter_constraint_for_position_across_signatures(
                        signatures, pos,
                    ),
                );
            }
            let grandparent = store.parent(parent);
            if grandparent.is_some_and(|grandparent| ast::is_decorator(store, grandparent)) {
                let decorator = grandparent.unwrap();
                // PORT NOTE: reshaped for borrowck; TS-Go evaluates signatures before passing them.
                let signatures = self.get_uninstantiated_signatures(decorator);
                return Some(
                    self.get_type_parameter_constraint_for_position_across_signatures(
                        signatures, pos,
                    ),
                );
            }
            if ast::is_expression_with_type_arguments(store, parent)
                && grandparent
                    .is_some_and(|grandparent| ast::is_expression_statement(store, grandparent))
            {
                let uninstantiated_type = self.check_expression(store.expression(parent).unwrap());
                // PORT NOTE: reshaped for borrowck; preserve TS-Go argument evaluation order.
                let call_signatures =
                    self.get_signatures_of_type(uninstantiated_type, SIGNATURE_KIND_CALL);
                let call_constraint = self
                    .get_type_parameter_constraint_for_position_across_signatures(
                        call_signatures,
                        pos,
                    );
                let construct_signatures =
                    self.get_signatures_of_type(uninstantiated_type, SIGNATURE_KIND_CONSTRUCT);
                let construct_constraint = self
                    .get_type_parameter_constraint_for_position_across_signatures(
                        construct_signatures,
                        pos,
                    );
                if self.type_flags(construct_constraint) & TYPE_FLAGS_NEVER != 0 {
                    return Some(call_constraint);
                }
                if self.type_flags(call_constraint) & TYPE_FLAGS_NEVER != 0 {
                    return Some(construct_constraint);
                }
                return Some(
                    self.get_intersection_type(vec![call_constraint, construct_constraint]),
                );
            }
            if ast::is_type_reference_type(store, &parent) {
                let type_parameters = self
                    .get_type_parameters_for_type_reference_or_import(parent)
                    .unwrap_or_default();
                if type_parameters.is_empty() || pos >= type_parameters.len() {
                    return None;
                }
                let relevant_type_parameter = type_parameters[pos];
                if let Some(constraint) =
                    self.get_constraint_of_type_parameter(relevant_type_parameter)
                {
                    let effective_type_arguments =
                        self.get_effective_type_arguments(parent, &type_parameters);
                    let mapper =
                        self.new_type_mapper_handle(type_parameters, effective_type_arguments);
                    return self
                        .instantiate_type_with_mapper_handle(Some(constraint), Some(mapper));
                }
            }
        }
        None
    }

    pub fn is_type_invalid_due_to_union_discriminant(
        &mut self,
        contextual_type: TypeHandle,
        obj: ast::Node,
    ) -> bool {
        let store = self.store_for_node(obj);
        store.properties(obj).into_iter().flatten().any(|property| {
            let property = property;
            let mut name_type = None;
            if let Some(property_name) = store.name(property) {
                let property_name = property_name;
                if ast::is_jsx_namespaced_name(store, property_name) {
                    name_type = Some(self.get_string_literal_type(&store.text(property_name)));
                } else {
                    name_type = Some(self.get_literal_type_from_property_name(property_name));
                }
            }
            let mut name = String::new();
            if name_type.is_some() && self.is_type_usable_as_property_name(name_type.unwrap()) {
                name = self.get_property_name_from_type(name_type.unwrap());
            }
            let expected = if !name.is_empty() {
                self.get_type_of_property_of_type(contextual_type, &name)
            } else {
                None
            };
            if let Some(expected) = expected {
                let property_type = self.get_type_of_node(property);
                return self.is_literal_type(expected)
                    && !self.is_type_assignable_to(property_type, expected);
            }
            false
        })
    }

    // Unlike `getExportsOfModule`, this includes properties of an `export =` value.
    pub fn get_exports_and_properties_of_module(
        &mut self,
        module_symbol: ast::SymbolIdentity,
    ) -> Vec<ast::SymbolIdentity> {
        let module_symbol = to_checker_symbol(module_symbol);
        let mut exports = self.get_exports_of_module_as_array(module_symbol);
        let export_equals = self.resolve_external_module_symbol_identity(
            module_symbol,
            false, /*dontResolveAlias*/
        );
        if !self.same_optional_symbol_identity(export_equals, Some(module_symbol)) {
            let Some(export_equals) = export_equals else {
                return to_ast_symbols(exports);
            };
            let t = self.get_type_of_symbol_identity_at_location(export_equals, None);
            if self.should_treat_properties_of_external_module_as_exports(t) {
                exports.extend(self.get_property_identities_of_type_for_services(t));
            }
        }
        to_ast_symbols(exports)
    }

    fn get_exports_of_module_as_array(
        &mut self,
        module_symbol: SymbolIdentity,
    ) -> Vec<SymbolIdentity> {
        let exports = self.collect_exports_of_module_identity(module_symbol);
        self.symbol_identities_to_array(&exports)
    }

    // Returns all the properties of the Jsx.IntrinsicElements interface.
    pub fn get_jsx_intrinsic_tag_names_at(
        &mut self,
        location: ast::Node,
    ) -> Vec<ast::SymbolIdentity> {
        let location = location;
        let intrinsics = self.get_jsx_type(JSX_NAMES_INTRINSIC_ELEMENTS, location);
        to_ast_symbols(self.get_property_identities_of_type_for_services(intrinsics))
    }

    pub fn get_contextual_type_for_jsx_attribute_public(
        &mut self,
        attribute: ast::JsxAttributeLike,
        context_flags: ContextFlags,
    ) -> Option<TypeHandle> {
        self.get_contextual_type_for_jsx_attribute(attribute, context_flags)
    }

    pub(crate) fn get_constant_value(&mut self, node: ast::Node) -> Option<evaluator::Value> {
        let store = self.store_for_node(node);
        if store.kind(node) == ast::Kind::EnumMember {
            return Some(self.get_enum_member_value(node).value);
        }
        if self.node_resolved_symbol_identity(node).is_none() {
            self.check_expression_cached(node); // ensure cached resolved symbol is set
        }
        let mut symbol = self.node_resolved_symbol_identity(node);
        if symbol.is_none() && ast::is_entity_name_expression(store, node) {
            symbol = self.resolve_entity_name(node, ast::SYMBOL_FLAGS_VALUE, true, false, None);
        }
        if let Some(symbol) = symbol {
            if self.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0 {
                // inline property\index accesses only for const enums
                let member = self
                    .symbol_handle_value_declaration(symbol.symbol_handle())
                    .unwrap();
                let member_store = self.store_for_node(member);
                if member_store
                    .parent(member)
                    .is_some_and(|parent| ast::is_enum_const(member_store, parent))
                {
                    return Some(self.get_enum_member_value(member).value);
                }
            }
        }
        None
    }

    pub fn get_constant_value_public(&mut self, node: ast::Node) -> Option<evaluator::Value> {
        self.get_constant_value(node)
    }

    fn get_resolved_signature_worker(
        &mut self,
        node: ast::Node,
        check_mode: CheckMode,
        argument_count: usize,
    ) -> (Option<SignatureHandle>, Vec<SignatureHandle>) {
        let parsed_node = printer::new_emit_context().parse_node(&node);
        self.set_apparent_argument_count(argument_count as isize);
        let mut candidates_out_array = Vec::new();
        let mut res = None;
        if let Some(parsed_node) = parsed_node {
            let parsed_node = parsed_node;
            res = Some(self.get_resolved_signature(
                parsed_node,
                Some(&mut candidates_out_array),
                check_mode,
            ));
        }
        self.clear_apparent_argument_count();
        (res, candidates_out_array)
    }
}

fn run_with_inference_blocked_from_source_node<'a, T>(
    c: &mut Checker<'a, '_>,
    node: ast::Node,
    mut f: impl FnMut(&mut Checker<'a, '_>) -> T,
) -> T {
    let store = c.store_for_node(node);
    let containing_call = ast::find_ancestor(store, Some(node), |store, node| {
        ast::is_call_like_expression(store, node)
    });
    if let Some(containing_call) = containing_call {
        let mut to_mark_skip = Some(node);
        while let Some(n) = to_mark_skip {
            c.record_skip_direct_inference_node(n);
            to_mark_skip = store.parent(n);
            if to_mark_skip.is_none()
                || to_mark_skip.is_some_and(|n| node_matches_name(store, n, containing_call))
            {
                break;
            }
        }
    }
    c.set_inference_partially_blocked(true);
    let result = run_without_resolved_signature_caching(c, node, |c| f(c));
    c.set_inference_partially_blocked(false);
    c.clear_skip_direct_inference_nodes();
    result
}

pub fn get_resolved_signature_for_signature_help<'a>(
    node: ast::Node,
    argument_count: usize,
    c: &mut Checker<'a, '_>,
) -> (Option<SignatureHandle>, Vec<SignatureHandle>) {
    let node = node;
    run_without_resolved_signature_caching(c, node, |c| {
        c.get_resolved_signature_worker(node, CHECK_MODE_IS_FOR_SIGNATURE_HELP, argument_count)
    })
}

fn run_without_resolved_signature_caching<'a, T>(
    c: &mut Checker<'a, '_>,
    node: ast::Node,
    mut f: impl FnMut(&mut Checker<'a, '_>) -> T,
) -> T {
    let store = c.store_for_node(node);
    let ancestor_node = ast::find_ancestor(store, Some(node), |store, node| {
        ast::is_call_like_or_function_like_expression(store, node)
    });
    if let Some(ancestor_node) = ancestor_node {
        let mut ancestor_node = ancestor_node;
        let mut cached_resolved_signatures = Vec::new();
        let mut cached_types = Vec::new();
        loop {
            let resolved_signature = c
                .semantic_state
                .replace_resolved_signature(ancestor_node, None);
            cached_resolved_signatures.push((ancestor_node, resolved_signature));
            if ast::is_function_expression_or_arrow_function(
                c.store_for_node(ancestor_node),
                ancestor_node,
            ) {
                if let Some(symbol) = c.get_symbol_of_declaration(ancestor_node) {
                    let resolved_type = c.semantic_state.value_symbol_resolved_type(&symbol);
                    c.semantic_state
                        .set_value_symbol_resolved_type(&symbol, None);
                    cached_types.push((symbol, resolved_type));
                }
            }
            let next = store.parent(ancestor_node).and_then(|p| {
                ast::find_ancestor(store, Some(p), |store, node| {
                    ast::is_call_like_or_function_like_expression(store, node)
                })
            });
            if next.is_none() {
                break;
            }
            ancestor_node = next.unwrap();
        }
        let result = f(c);
        for (ancestor_node, resolved_signature) in cached_resolved_signatures {
            c.semantic_state
                .set_resolved_signature(ancestor_node, resolved_signature);
        }
        for (symbol, resolved_type) in cached_types {
            c.semantic_state
                .set_value_symbol_resolved_type(symbol, resolved_type);
        }
        return result;
    }
    f(c)
}

fn is_export_specifier_alias(
    store: &ast::AstStore,
    reference_location: ast::Node,
    export_specifier: ast::Node,
) -> bool {
    let property_name = store.property_name(export_specifier);
    let name = store.name(export_specifier);
    debug::assert(
        property_name.is_some_and(|property_name| {
            node_matches_name(store, property_name, reference_location)
        }) || name.is_some_and(|name| node_matches_name(store, name, reference_location)),
        Some("referenceLocation is not export specifier name or property name".to_string()),
    );
    if let Some(property_name) = property_name {
        // Given `export { foo as bar } [from "someModule"]`: It's an alias at `foo`, but at `bar` it's a new symbol.
        node_matches_name(store, property_name, reference_location)
    } else {
        // `export { foo } from "foo"` is a re-export.
        // `export { foo };` is not a re-export, it creates an alias for the local variable `foo`.
        store
            .parent(export_specifier)
            .and_then(|parent| store.parent(parent))
            .and_then(|parent| store.module_specifier(parent))
            .is_none()
    }
}

fn create_symbol_identity_table_for_services(
    checker: &mut Checker<'_, '_>,
    symbols: Vec<SymbolIdentity>,
) -> SymbolIdentityTable {
    if symbols.is_empty() {
        return SymbolIdentityTable::default();
    }
    let mut result = SymbolIdentityTable::default();
    for symbol in symbols {
        let name = checker.symbol_identity_name(symbol);
        result.insert(name, symbol);
    }
    result
}

fn get_named_member_identities_for_services(
    checker: &mut Checker<'_, '_>,
    members: &SymbolIdentityTable,
) -> Vec<SymbolIdentity> {
    if members.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    for (id, &symbol) in members {
        if is_named_member_identity_for_services(checker, symbol, id) {
            result.push(symbol);
        }
    }
    if checker.compiler_options.stable_type_ordering.is_true() {
        sort_symbol_identities_for_services(checker, &mut result);
    }
    result
}

fn is_named_member_identity_for_services(
    checker: &mut Checker<'_, '_>,
    symbol: SymbolIdentity,
    id: &str,
) -> bool {
    !is_reserved_member_name(id)
        && checker
            .missing_name_symbol_identity_flags(symbol)
            .intersects(ast::SYMBOL_FLAGS_VALUE)
}

fn sort_symbol_identities_for_services(checker: &Checker<'_, '_>, symbols: &mut [SymbolIdentity]) {
    symbols.sort_by(|&left, &right| compare_symbol_identities_for_services(checker, left, right));
}

fn compare_symbol_identities_for_services(
    checker: &Checker<'_, '_>,
    left: SymbolIdentity,
    right: SymbolIdentity,
) -> std::cmp::Ordering {
    if left == right {
        return std::cmp::Ordering::Equal;
    }
    let declaration_order = compare_nodes_for_services(
        checker,
        checker.first_symbol_identity_declaration(left),
        checker.first_symbol_identity_declaration(right),
    );
    if declaration_order != std::cmp::Ordering::Equal {
        return declaration_order;
    }
    checker
        .missing_name_symbol_identity_name_ref(left)
        .cmp(checker.missing_name_symbol_identity_name_ref(right))
        .then_with(|| checker.compare_symbol_identity_tiebreaker(left, right))
}

fn compare_nodes_for_services(
    checker: &Checker<'_, '_>,
    left: Option<ast::Node>,
    right: Option<ast::Node>,
) -> std::cmp::Ordering {
    if left == right {
        return std::cmp::Ordering::Equal;
    }
    let (Some(left), Some(right)) = (left, right) else {
        return if left.is_some() {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        };
    };
    let left_source_file = checker.try_source_file_for_node_with_order(left);
    let right_source_file = checker.try_source_file_for_node_with_order(right);
    let same_file = match (left_source_file, right_source_file) {
        (Some((_, left_index)), Some((_, right_index))) => left_index == right_index,
        (None, None) => true,
        _ => false,
    };
    if !same_file {
        let left_index = left_source_file
            .map(|(_, index)| index)
            .unwrap_or(usize::MAX);
        let right_index = right_source_file
            .map(|(_, index)| index)
            .unwrap_or(usize::MAX);
        return left_index.cmp(&right_index);
    }
    let left_store = left_source_file
        .map(|(file, _)| file.store())
        .unwrap_or_else(|| checker.store_for_node(left));
    let right_store = right_source_file
        .map(|(file, _)| file.store())
        .unwrap_or_else(|| checker.store_for_node(right));
    left_store
        .loc(left)
        .pos()
        .cmp(&right_store.loc(right).pos())
}

fn get_possible_symbol_reference_nodes<'a>(
    source_file: &'a ast::SourceFile,
    symbol_name: &str,
    container: ast::Node,
) -> Vec<ast::Node> {
    let source_file_node = source_file.root();
    get_possible_symbol_reference_positions(source_file, symbol_name, Some(container))
        .into_iter()
        .filter_map(|pos| {
            let reference_location = astnav::get_touching_property_name(source_file, pos as i32);
            if let Some(reference_location) = reference_location {
                if reference_location != source_file_node {
                    return Some(reference_location);
                }
            }
            None
        })
        .collect()
}

fn get_possible_symbol_reference_positions(
    source_file: &ast::SourceFile,
    symbol_name: &str,
    container: Option<ast::Node>,
) -> Vec<usize> {
    let mut positions = Vec::new();
    // TODO: Cache symbol existence for files to save text search
    // Also, need to make this work for unicode escapes.
    if symbol_name.is_empty() {
        return positions;
    }
    let store = source_file.store();
    let text = source_file.text();
    let source_length = text.len();
    let symbol_name_length = symbol_name.len();
    let source_file_node = source_file.root();
    let container = container.unwrap_or(source_file_node);
    let container_pos = store.loc(container).pos() as usize;
    let mut position = text[container_pos..]
        .find(symbol_name)
        .map(|p| p + container_pos);
    let end_pos = store.loc(container).end() as usize;
    while let Some(pos) = position {
        if pos >= end_pos {
            break;
        }
        // We found a match.  Make sure it's not part of a larger word (i.e. the char
        // before and after it have to be a non-identifier char).
        let end_position = pos + symbol_name_length;
        if (pos == 0 || !scanner::is_identifier_part(text.as_bytes()[pos - 1] as char))
            && (end_position == source_length
                || !scanner::is_identifier_part(text.as_bytes()[end_position] as char))
        {
            // Found a real match.  Keep searching.
            positions.push(pos);
        }
        let start_index = pos + symbol_name_length + 1;
        if start_index > text.len() {
            break;
        }
        position = text[start_index..]
            .find(symbol_name)
            .map(|found| start_index + found);
    }
    positions
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn get_candidate_signatures_for_string_literal_completions(
        &mut self,
        call: &ast::CallLikeExpression,
        editing_argument: ast::Node,
    ) -> Vec<SignatureHandle> {
        let call = *call;
        let editing_argument = editing_argument;
        // first, get candidates when inference is blocked from the source node.
        let mut candidates =
            run_with_inference_blocked_from_source_node(self, editing_argument, |c| {
                let (_, blocked_inference_candidates) =
                    c.get_resolved_signature_worker(call, CHECK_MODE_NORMAL, 0);
                blocked_inference_candidates
            });
        let candidates_set = candidates.clone();

        // next, get candidates where the source node is considered for inference.
        let other_candidates =
            run_without_resolved_signature_caching(self, editing_argument, |c| {
                let (_, inference_candidates) =
                    c.get_resolved_signature_worker(call, CHECK_MODE_NORMAL, 0);
                inference_candidates
            });
        for candidate in other_candidates {
            if candidates_set.iter().any(|existing| *existing == candidate) {
                continue;
            }
            candidates.push(candidate);
        }
        candidates
    }

    // GetTypeAtPosition returns the type of a parameter at a given index in a signature.
    pub fn get_type_at_position_public(&mut self, s: SignatureHandle, pos: usize) -> TypeHandle {
        self.get_type_at_position(s, pos)
    }

    pub fn get_type_parameter_at_position(&mut self, s: SignatureHandle, pos: usize) -> TypeHandle {
        let t = self.get_type_at_position(s, pos);
        if self.type_flags(t) & TYPE_FLAGS_INDEX != 0 {
            let target = self.type_record(t).as_index_type().target.unwrap();
            let is_this_type_parameter = self.type_flags(target) & TYPE_FLAGS_TYPE_PARAMETER != 0
                && self.type_record(target).as_type_parameter().is_this_type;
            if is_this_type_parameter {
                let constraint = self.get_base_constraint_of_type(target);
                if let Some(constraint) = constraint {
                    return self.get_index_type(constraint);
                }
            }
        }
        t
    }

    // GetContextualTypeForArrayLiteralAtPosition returns the contextual type for an element at the given position
    // in an array with the given contextual type.
    pub fn get_contextual_type_for_array_literal_at_position(
        &mut self,
        contextual_array_type: Option<TypeHandle>,
        array_literal: ast::Node,
        position: usize,
    ) -> Option<TypeHandle> {
        let contextual_array_type = contextual_array_type?;
        let mut first_spread_index = -1;
        let mut last_spread_index = -1;
        let mut element_index = 0;
        let store = self.store_for_node(array_literal);
        let elements = store
            .elements(array_literal)
            .expect("array literal should have elements");
        for (i, elem) in elements.iter().enumerate() {
            if store.loc(elem).pos() < position as i32 {
                element_index += 1;
            }
            if ast::is_spread_element(store, elem) {
                if first_spread_index == -1 {
                    first_spread_index = i as isize;
                }
                last_spread_index = i as isize;
            }
        }
        // The array may be incomplete, so we don't know its final length.
        self.get_contextual_type_for_element_expression(
            Some(contextual_array_type),
            element_index,
            -1, /*length*/
            first_spread_index,
            last_spread_index,
        )
    }

    pub fn get_first_type_argument_from_known_type(&mut self, t: TypeHandle) -> Option<TypeHandle> {
        if self.object_flags(t) & OBJECT_FLAGS_REFERENCE != 0
            && let Some(type_symbol) = self.type_symbol_identity(t)
            && is_known_generic_type_name(&self.missing_name_symbol_identity_name(type_symbol))
        {
            let symbol_name = self.missing_name_symbol_identity_name(type_symbol);
            let symbol =
                self.get_global_symbol_identity(&symbol_name, ast::SYMBOL_FLAGS_TYPE, None);
            if symbol.is_some()
                && self.same_optional_symbol_identity(
                    symbol,
                    self.type_symbol_identity(self.type_target(t)),
                )
            {
                return if self.type_arguments_len(t) > 0 {
                    Some(self.type_argument_at(t, 0))
                } else {
                    None
                };
            }
        }
        if let Some(alias) = self.type_alias_record(t).cloned() {
            let alias_symbol = alias.symbol.expect("alias type must keep alias symbol");
            if is_known_generic_type_name(&self.missing_name_symbol_identity_name(alias_symbol)) {
                let symbol_name = self.missing_name_symbol_identity_name(alias_symbol);
                let symbol =
                    self.get_global_symbol_identity(&symbol_name, ast::SYMBOL_FLAGS_TYPE, None);
                if self.same_optional_symbol_identity(symbol, Some(alias_symbol)) {
                    return alias.type_arguments.first().copied();
                }
            }
        }
        None
    }

    // Gets all symbols for one property. Does not get symbols for every property.
    pub fn get_property_symbols_from_contextual_type(
        &mut self,
        node: ast::Node,
        contextual_type: TypeHandle,
        union_symbol_ok: bool,
    ) -> Vec<ast::SymbolIdentity> {
        to_ast_symbols(self.get_property_symbols_from_contextual_type_worker(
            node,
            contextual_type,
            union_symbol_ok,
        ))
    }

    fn get_property_symbols_from_contextual_type_worker(
        &mut self,
        node: ast::Node,
        contextual_type: TypeHandle,
        union_symbol_ok: bool,
    ) -> Vec<SymbolIdentity> {
        let store = self.store_for_node(node);
        let Some(name_node) = store.name(node) else {
            return Vec::new();
        };
        let name = ast::get_text_of_property_name(store, name_node);
        if name.is_empty() {
            return Vec::new();
        }
        if self.type_flags(contextual_type) & TYPE_FLAGS_UNION == 0 {
            if let Some(symbol) = self.get_property_of_type(contextual_type, &name) {
                return vec![symbol];
            }
            return Vec::new();
        }
        let mut filtered_types = self.type_types(contextual_type);
        let parent = store.parent(node).unwrap();
        if ast::is_object_literal_expression(store, parent) || ast::is_jsx_attributes(store, parent)
        {
            filtered_types = filtered_types
                .into_iter()
                .filter(|t| !self.is_type_invalid_due_to_union_discriminant(*t, parent))
                .collect();
        }
        let mut discriminated_property_symbols = Vec::new();
        for t in filtered_types.iter().copied() {
            if let Some(symbol) = self.get_property_of_type(t, &name) {
                discriminated_property_symbols.push(symbol);
            }
        }
        if union_symbol_ok
            && (discriminated_property_symbols.is_empty()
                || discriminated_property_symbols.len() == self.type_types_len(contextual_type))
        {
            if let Some(symbol) = self.get_property_of_type(contextual_type, &name) {
                return vec![symbol];
            }
        }
        if filtered_types.is_empty() && discriminated_property_symbols.is_empty() {
            // Bad discriminant -- do again without discriminating
            let mut symbols = Vec::new();
            for t in self.type_types(contextual_type) {
                if let Some(symbol) = self.get_property_of_type(t, &name) {
                    symbols.push(symbol);
                }
            }
            return symbols;
        }
        // by eliminating duplicates we might even end up with a single symbol
        // that helps with displaying better quick infos on properties of union types
        core::deduplicate(&discriminated_property_symbols)
    }

    // Gets the property symbol corresponding to the property in destructuring assignment
    // 'property1' from
    //
    //	for ( { property1: a } of elems) {
    //	}
    //
    // 'property1' at location 'a' from:
    //
    //	[a] = [ property1, property2 ]
    pub fn get_property_symbol_of_destructuring_assignment(
        &mut self,
        location: ast::Node,
    ) -> Option<SymbolIdentity> {
        self.get_property_symbol_of_destructuring_assignment_worker(location)
    }

    fn get_property_symbol_of_destructuring_assignment_worker(
        &mut self,
        location: ast::Node,
    ) -> Option<SymbolIdentity> {
        let store = self.store_for_node(location);
        let parent = store.parent(location).unwrap();
        let grandparent = store.parent(parent).unwrap();
        if ast::is_array_literal_or_object_literal_destructuring_pattern(store, Some(grandparent)) {
            // Get the type of the object or array literal and then look for property of given name in the type
            if let Some(type_of_object_literal) = self.get_type_of_assignment_pattern(grandparent) {
                return self
                    .get_property_of_type(type_of_object_literal, &store.text(location))
                    .map(|symbol| symbol);
            }
        }
        None
    }

    // Gets the type of object literal or array literal of destructuring assignment.
    // { a } from
    //
    //	for ( { a } of elems) {
    //	}
    //
    // [ a ] from
    //
    //	[a] = [ some array ...]
    fn get_type_of_assignment_pattern(&mut self, expr: ast::Node) -> Option<TypeHandle> {
        let store = self.store_for_node(expr);
        let expr_parent = store.parent(expr).unwrap();
        // If this is from "for of"
        //     for ( { a } of elems) {
        //     }
        if ast::is_for_of_statement(store, &expr_parent) {
            let iterated_type = self.check_right_hand_side_of_for_of(expr_parent);
            let iterated_type =
                iterated_type.unwrap_or(self.semantic_state.semantic_handles().error_type);
            return Some(self.check_destructuring_assignment(
                expr,
                iterated_type,
                CHECK_MODE_NORMAL,
                false,
            ));
        }
        // If this is from "for" initializer
        //     for ({a } = elems[0];.....) { }
        if ast::is_binary_expression(store, expr_parent) {
            let parent = expr_parent;
            let iterated_type = self.get_type_of_expression(store.right(parent).unwrap());
            return Some(self.check_destructuring_assignment(
                expr,
                iterated_type,
                CHECK_MODE_NORMAL,
                false,
            ));
        }
        // If this is from nested object binding pattern
        //     for ({ skills: { primary, secondary } } = multiRobot, i = 0; i < 1; i++) {
        if ast::is_property_assignment(store, expr_parent) {
            let property = expr_parent;
            let node = store.parent(property).unwrap();
            let type_of_parent_object_literal = self
                .get_type_of_assignment_pattern(node)
                .unwrap_or(self.semantic_state.semantic_handles().error_type);
            let properties = store
                .properties(node)
                .expect("object literal should have properties");
            let property_index = properties
                .iter()
                .position(|p| node_matches_name(store, p, property))
                .unwrap_or(0);
            return self.check_object_literal_destructuring_property_assignment(
                node,
                type_of_parent_object_literal,
                property_index,
                None,
                false,
            );
        }
        // Array literal assignment - array destructuring pattern
        let node = expr_parent;
        //    [{ property1: p1, property2 }] = elems;
        let type_of_array_literal = self
            .get_type_of_assignment_pattern(node)
            .unwrap_or(self.semantic_state.semantic_handles().error_type);
        let element_type = self.check_iterated_type_or_element_type(
            ITERATION_USE_DESTRUCTURING,
            type_of_array_literal,
            self.semantic_state.semantic_handles().undefined_type,
            Some(node),
        );
        self.check_array_literal_destructuring_element_assignment(
            node,
            type_of_array_literal,
            store
                .elements(node)
                .expect("array literal should have elements")
                .iter()
                .position(|e| node_matches_name(store, e, expr))
                .unwrap_or(0),
            element_type,
            CHECK_MODE_NORMAL,
        )
    }

    pub fn get_signature_from_declaration_public(&mut self, node: ast::Node) -> SignatureHandle {
        self.get_signature_from_declaration(node)
    }

    // IsLibSymbolForHoverVerbosity returns true if a symbol is declared in a lib file.
    pub(crate) fn is_lib_symbol_for_hover_verbosity(&self, symbol: Option<SymbolIdentity>) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        self.with_symbol_identity_declarations(symbol, |declarations| {
            for &decl in declarations {
                let store = self.store_for_node(decl);
                let sf = ast::get_source_file_of_node(store, Some(decl));
                if sf.is_some()
                    && self
                        .program
                        .is_source_file_default_library(store.as_source_file(sf.unwrap()).path())
                {
                    return true;
                }
            }
            false
        })
    }

    // IsLibTypeForHoverVerbosity returns true if a type is declared in a lib file.
    // Don't expand types like Array or Promise, instead treating them as opaque.
    pub fn is_lib_type_for_hover_verbosity(&self, t: TypeHandle) -> bool {
        let symbol = if self.object_flags(t) & OBJECT_FLAGS_REFERENCE != 0 {
            self.type_symbol_identity(self.type_target(t))
        } else {
            self.type_symbol_identity(t)
        };
        if self.is_lib_symbol_for_hover_verbosity(symbol) {
            return true;
        }
        self.is_tuple_type(t)
    }
}

static KNOWN_GENERIC_TYPE_NAMES: &[&str] = &[
    "Array",
    "ArrayLike",
    "ReadonlyArray",
    "Promise",
    "PromiseLike",
    "Iterable",
    "IterableIterator",
    "AsyncIterable",
    "Set",
    "WeakSet",
    "ReadonlySet",
    "Map",
    "WeakMap",
    "ReadonlyMap",
    "Partial",
    "Required",
    "Readonly",
    "Pick",
    "Omit",
    "NonNullable",
];

fn is_known_generic_type_name(name: &str) -> bool {
    KNOWN_GENERIC_TYPE_NAMES.contains(&name)
}
