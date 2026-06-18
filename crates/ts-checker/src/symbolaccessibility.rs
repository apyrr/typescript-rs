use std::collections::{HashMap, HashSet};

use ts_ast as ast;
use ts_printer as printer;

use crate::checker::*;
use crate::semantic::ContainingSymbolLinksStoreExt;
use crate::types::SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND;

fn node_matches_name(store: &ast::AstStore, left: ast::Node, right: ast::Node) -> bool {
    left == right || store.kind(left) == store.kind(right) && store.text(left) == store.text(right)
}

impl<'a, 'state> Checker<'a, 'state> {
    pub fn is_type_symbol_accessible(
        &mut self,
        type_symbol: SymbolIdentity,
        enclosing_declaration: ast::Node,
    ) -> bool {
        let access = self.is_symbol_accessible_identity_worker(
            Some(type_symbol),
            Some(enclosing_declaration),
            ast::SYMBOL_FLAGS_TYPE, /*shouldComputeAliasesToMakeVisible*/
            false,                  /*allowModules*/
            true,
        );
        access.accessibility == printer::SymbolAccessibility::Accessible
    }

    pub fn is_value_symbol_accessible(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: ast::Node,
    ) -> bool {
        let access = self.is_symbol_accessible_identity_worker(
            Some(symbol),
            Some(enclosing_declaration),
            ast::SYMBOL_FLAGS_VALUE, /*shouldComputeAliasesToMakeVisible*/
            false,                   /*allowModules*/
            true,
        );
        access.accessibility == printer::SymbolAccessibility::Accessible
    }

    pub fn is_symbol_accessible_by_flags(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: ast::Node,
        flags: ast::SymbolFlags,
    ) -> bool {
        let access = self.is_symbol_accessible_identity_worker(
            Some(symbol),
            Some(enclosing_declaration),
            flags, /*shouldComputeAliasesToMakeVisible*/
            false, /*allowModules*/
            false,
        ); // TODO: Strada bug? Why is this allowModules: false?
        access.accessibility == printer::SymbolAccessibility::Accessible
    }

    pub fn is_any_symbol_accessible(
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

        let mut had_accessible_chain: Option<SymbolIdentity> = None;
        let mut early_module_bail = false;
        for &symbol in symbols.iter() {
            // Symbol is accessible if it by itself is accessible
            let accessible_symbol_chain = self.get_accessible_symbol_chain_identity(
                symbol,
                Some(enclosing_declaration),
                meaning, /*useOnlyExternalAliasing*/
                false,
            );
            if !accessible_symbol_chain.is_empty() {
                had_accessible_chain = Some(symbol);
                let has_accessible_declarations = self.has_visible_declarations_by_identity(
                    accessible_symbol_chain[0],
                    should_compute_aliases_to_make_visible,
                );
                if has_accessible_declarations.is_some() {
                    return has_accessible_declarations;
                }
            }
            if allow_modules {
                if self.any_symbol_handle_declaration(
                    symbol.symbol_handle(),
                    |checker, declaration| {
                        let store = checker.store_for_node(declaration);
                        has_non_global_augmentation_external_module_symbol(
                            checker,
                            store,
                            declaration,
                        )
                    },
                ) {
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
            }

            // If we haven't got the accessible symbol, it doesn't mean the symbol is actually inaccessible.
            // It could be a qualified symbol and hence verify the path
            // e.g.:
            // module m {
            //     export class c {
            //     }
            // }
            // const x: typeof m.c
            // In the above example when we start with checking if typeof m.c symbol is accessible,
            // we are going to see if c can be accessed in scope directly.
            // But it can't, hence the accessible is going to be undefined, but that doesn't mean m.c is inaccessible
            // It is accessible if the parent m is accessible because then m.c can be accessed through qualification

            let containers = self.get_containers_of_symbol_identity(
                symbol,
                Some(enclosing_declaration),
                meaning,
            );
            let mut next_meaning = meaning;
            if self.same_symbol_identity(initial_symbol, symbol) {
                next_meaning = get_qualified_left_meaning(meaning);
            }
            let parent_result = self.is_any_symbol_accessible(
                &containers,
                enclosing_declaration,
                initial_symbol,
                next_meaning,
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
            let mut module_name = String::new();
            if !self.same_symbol_identity(had_accessible_chain, initial_symbol) {
                module_name = self.symbol_identity_to_string_ex(
                    had_accessible_chain,
                    Some(enclosing_declaration),
                    ast::SYMBOL_FLAGS_NAMESPACE,
                    SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
                );
            }
            let error_symbol_name = self.symbol_identity_to_string_ex(
                initial_symbol,
                Some(enclosing_declaration),
                meaning,
                SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
            );
            return Some(printer::SymbolAccessibilityResult {
                accessibility: printer::SymbolAccessibility::NotAccessible,
                error_symbol_name,
                error_module_name: module_name,
                ..Default::default()
            });
        }
        None
    }

    fn get_with_alternative_containers(
        &mut self,
        container: SymbolIdentity,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> Vec<SymbolIdentity> {
        let mut additional_containers = Vec::new();
        self.for_each_symbol_handle_declaration(container.symbol_handle(), |checker, d| {
            if let Some(file_symbol) = checker
                .get_file_symbol_if_file_symbol_export_equals_container_identity(d, container)
            {
                additional_containers.push(file_symbol);
            }
        });
        let mut reexport_containers = Vec::new();
        if let Some(enclosing_declaration) = enclosing_declaration {
            reexport_containers =
                self.get_alternative_containing_modules(symbol, enclosing_declaration);
        }
        let object_literal_container =
            self.get_variable_declaration_of_object_literal(container, meaning);
        let left_meaning = get_qualified_left_meaning(meaning);
        if enclosing_declaration.is_some()
            && self
                .symbol_identity_flags(container)
                .intersects(left_meaning)
            && !self
                .get_accessible_symbol_chain_identity(
                    container,
                    enclosing_declaration,
                    ast::SYMBOL_FLAGS_NAMESPACE, /*useOnlyExternalAliasing*/
                    false,
                )
                .is_empty()
        {
            // This order expresses a preference for the real container if it is in scope
            let mut res = vec![container];
            res.extend(additional_containers);
            res.extend(reexport_containers);
            if let Some(object_literal_container) = object_literal_container {
                res.push(object_literal_container);
            }
            return res;
        }
        // we potentially have a symbol which is a member of the instance side of something - look for a variable in scope with the container's type
        // which may be acting like a namespace (eg, `Symbol` acts like a namespace when looking up `Symbol.toStringTag`)
        let mut first_variable_match = None;
        let container_declared_type = self.declared_type_of_symbol_identity(container);
        if meaning == ast::SYMBOL_FLAGS_VALUE
            && !self
                .symbol_identity_flags(container)
                .intersects(left_meaning)
            && self
                .symbol_identity_flags(container)
                .intersects(ast::SYMBOL_FLAGS_TYPE)
            && container_declared_type.is_some_and(|container_declared_type| {
                self.type_flags(container_declared_type) & TYPE_FLAGS_OBJECT != 0
            })
        {
            self.some_symbol_table_in_scope(enclosing_declaration, |checker, t, _, _, _, _| {
                let mut found = None;
                t.for_each_value(checker, |checker, s| {
                    if found.is_some() {
                        return;
                    }
                    if checker.symbol_identity_flags(s).intersects(left_meaning)
                        && container_declared_type.is_some_and(|container_declared_type| {
                            checker.get_type_of_symbol_identity(s) == container_declared_type
                        })
                    {
                        found = Some(s);
                    }
                });
                first_variable_match = found;
                first_variable_match.is_some()
            });
        }

        let mut res = Vec::new();
        if let Some(first_variable_match) = first_variable_match {
            res.push(first_variable_match);
        }
        res.extend(additional_containers);
        res.push(container);
        if let Some(object_literal_container) = object_literal_container {
            res.push(object_literal_container);
        }
        res.extend(reexport_containers);
        res
    }

    fn get_alternative_containing_modules(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: ast::Node,
    ) -> Vec<SymbolIdentity> {
        let Some(source_file) = self.try_source_file_for_node(enclosing_declaration) else {
            return Vec::new();
        };
        let store = source_file.store();
        let containing_file =
            ast::get_source_file_of_node(store, Some(enclosing_declaration)).unwrap();
        let id = ast::get_node_id(store, containing_file);
        if let Some(existing) = self
            .semantic_state
            .alternative_containing_modules_for_file(symbol, id)
        {
            if !existing.is_empty() {
                return existing;
            }
        }
        let mut results: Vec<SymbolIdentity> = Vec::new();
        let imports = store.as_source_file(containing_file).imports().to_vec();
        if !imports.is_empty() {
            // Try to make an import using an import already in the enclosing file, if possible
            for import_ref in imports {
                let import_ref = import_ref;
                if ast::node_is_synthesized(store, import_ref) {
                    // Synthetic names can't be resolved by `resolveExternalModuleName` - they'll cause a debug assert if they error
                    continue;
                }
                let resolved_module = self.resolve_external_module_name(
                    enclosing_declaration,
                    import_ref,
                    true, /*ignoreErrors*/
                );
                let Some(resolved_module) = resolved_module else {
                    continue;
                };
                let r#ref =
                    self.get_alias_for_symbol_in_container_identity(resolved_module, symbol);
                if r#ref.is_none() {
                    continue;
                }
                results.push(resolved_module);
            }
            if !results.is_empty() {
                self.semantic_state
                    .record_alternative_containing_modules_for_file(symbol, id, results.clone());
                return results;
            }
        }

        if let Some(existing) = self.semantic_state.extended_containers(symbol) {
            return existing;
        }
        // No results from files already being imported by this file - expand search (expensive, but not location-specific, so cached)
        let other_files = self.program.source_files();
        for file in other_files {
            if !ast::is_external_module(file) {
                continue;
            }
            let file_node = file.root();
            let Some(sym) = self.get_symbol_of_declaration(file_node) else {
                continue;
            };
            let sym = SymbolIdentity::from_symbol_handle(sym);
            let r#ref = self.get_alias_for_symbol_in_container_identity(sym, symbol);
            if r#ref.is_none() {
                continue;
            }
            results.push(sym);
        }
        self.semantic_state
            .set_extended_containers(symbol, results.clone());
        results
    }

    fn get_variable_declaration_of_object_literal(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
    ) -> Option<SymbolIdentity> {
        // If we're trying to reference some object literal in, eg `var a = { x: 1 }`, the symbol for the literal, `__object`, is distinct
        // from the symbol of the declaration it is being assigned to. Since we can use the declaration to refer to the literal, however,
        // we'd like to make that connection here - potentially causing us to paint the declaration's visibility, and therefore the literal.
        if !meaning.intersects(ast::SYMBOL_FLAGS_VALUE) {
            return None;
        }
        let first_decl = self.first_symbol_identity_declaration(symbol)?;
        let store = self.store_for_node(first_decl);
        let Some(parent) = store.parent(first_decl) else {
            return None;
        };
        if !ast::is_variable_declaration(store, parent) {
            return None;
        }
        if ast::is_object_literal_expression(store, first_decl)
            && store
                .initializer(parent)
                .is_some_and(|initializer| node_matches_name(store, initializer, first_decl))
            || ast::is_type_literal_node(store, first_decl)
                && store
                    .type_node(parent)
                    .is_some_and(|type_node| node_matches_name(store, type_node, first_decl))
        {
            return self
                .get_symbol_of_declaration(parent)
                .map(SymbolIdentity::from_symbol_handle);
        }
        None
    }

    pub(crate) fn get_external_module_container_identity(
        &mut self,
        declaration: ast::Node,
    ) -> Option<SymbolIdentity> {
        let store = if declaration.store_id() == self.factory().store().store_id() {
            self.factory().store()
        } else {
            self.try_source_file_for_node(declaration)?.store()
        };
        let node = ast::find_ancestor(store, Some(declaration), |store, node| {
            has_external_module_symbol(self, store, node)
        });
        let node = node?;
        self.get_symbol_of_declaration(node)
            .map(SymbolIdentity::from_symbol_handle)
    }

    pub(crate) fn get_file_symbol_if_file_symbol_export_equals_container_identity(
        &mut self,
        d: ast::Node,
        container: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        let file_symbol = self.get_external_module_container_identity(d)?;
        let exported = self
            .lookup_symbol_identity_export(file_symbol, ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS)?;
        if self
            .get_symbol_if_same_reference(exported, container)
            .is_some()
        {
            return Some(file_symbol);
        }
        None
    }

    /**
     * Attempts to find the symbol corresponding to the container a symbol is in - usually this
     * is just its' `.parent`, but for locals, this value is `undefined`
     */
    pub(crate) fn get_containers_of_symbol_identity(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> Vec<SymbolIdentity> {
        let container = self.get_parent_of_symbol_identity(symbol);
        // Type parameters end up in the `members` lists but are not externally visible
        if container.is_some()
            && !self
                .symbol_identity_flags(symbol)
                .intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER)
        {
            return self.get_with_alternative_containers(
                container.unwrap(),
                symbol,
                enclosing_declaration,
                meaning,
            );
        }
        let mut candidates = Vec::new();
        self.for_each_symbol_handle_declaration(symbol.symbol_handle(), |checker, d| {
            let store = checker.store_for_node(d);
            let d_parent = store.parent(d);
            if !ast::is_ambient_module(store, d) && d_parent.is_some() {
                // direct children of a module
                let parent = d_parent.unwrap();
                if has_non_global_augmentation_external_module_symbol(checker, store, parent) {
                    let sym = checker
                        .get_symbol_of_declaration(parent)
                        .map(SymbolIdentity::from_symbol_handle);
                    if let Some(sym) = sym {
                        if !candidates.contains(&sym) {
                            candidates.push(sym);
                        }
                    }
                    return;
                }
                // export ='d member of an ambient module
                if ast::is_module_block(store, parent) && store.parent(parent).is_some() {
                    let grandparent = store.parent(parent).unwrap();
                    let module_symbol = checker.get_symbol_of_declaration(grandparent).unwrap();
                    let resolved_module_symbol = checker
                        .resolve_external_module_symbol_identity(
                            SymbolIdentity::from_symbol_handle(module_symbol),
                            false,
                        )
                        .unwrap_or(SymbolIdentity::from_symbol_handle(module_symbol));
                    if checker
                        .same_optional_symbol_identity(Some(resolved_module_symbol), Some(symbol))
                    {
                        let sym = checker
                            .get_symbol_of_declaration(grandparent)
                            .map(SymbolIdentity::from_symbol_handle);
                        if let Some(sym) = sym {
                            if !candidates.contains(&sym) {
                                candidates.push(sym);
                            }
                        }
                        return;
                    }
                }
            }
            if ast::is_class_expression(store, d) {
                let binary_parent =
                    d_parent.filter(|parent| ast::is_binary_expression(store, *parent));
                if let Some(binary_parent) = binary_parent {
                    let left = store.left(binary_parent).unwrap();
                    let is_equals_assignment = store
                        .operator_token(binary_parent)
                        .is_some_and(|operator| store.kind(operator) == ast::Kind::EqualsToken);
                    let left_expression = store.expression(left);
                    if is_equals_assignment
                        && ast::is_access_expression(store, left)
                        && left_expression
                            .as_ref()
                            .is_some_and(|expr| ast::is_entity_name_expression(store, *expr))
                    {
                        let left = left;
                        let left_expression = left_expression.unwrap();
                        if ast::is_module_exports_access_expression(store, left)
                            || ast::is_exports_identifier(store, left_expression)
                        {
                            let source_file = ast::get_source_file_of_node(store, Some(d)).unwrap();
                            let source_file_node = source_file;
                            let sym = checker
                                .get_symbol_of_declaration(source_file_node)
                                .map(SymbolIdentity::from_symbol_handle);
                            if let Some(sym) = sym {
                                if !candidates.contains(&sym) {
                                    candidates.push(sym);
                                }
                            }
                            return;
                        }
                        checker.check_expression_cached(left_expression);
                        let sym = checker.node_resolved_symbol_identity(left_expression);
                        if let Some(sym) = sym {
                            if !candidates.contains(&sym) {
                                candidates.push(sym);
                            }
                        }
                        return;
                    }
                }
            }
        });
        if candidates.is_empty() {
            return Vec::new();
        }

        let mut best_containers = Vec::new();
        let mut alternative_containers = Vec::new();
        for container in candidates {
            if self
                .get_alias_for_symbol_in_container_identity(container, symbol)
                .is_none()
            {
                continue;
            }
            let all_alts = self.get_with_alternative_containers(
                container,
                symbol,
                enclosing_declaration,
                meaning,
            );
            if all_alts.is_empty() {
                continue;
            }
            best_containers.push(all_alts[0].clone());
            alternative_containers.extend(all_alts.into_iter().skip(1));
        }
        best_containers.extend(alternative_containers);
        best_containers
    }

    pub(crate) fn get_alias_for_symbol_in_container_identity(
        &mut self,
        container: SymbolIdentity,
        symbol: SymbolIdentity,
    ) -> Option<SymbolIdentity> {
        if Some(container) == self.get_parent_of_symbol_identity(symbol) {
            // fast path, `symbol` is either already the alias or isn't aliased
            return Some(symbol);
        }
        // Check if container is a thing with an `export=` which points directly at `symbol`, and if so, return
        // the container itself as the alias for the symbol
        if let Some(export_equals) =
            self.lookup_symbol_identity_export(container, ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS)
        {
            if self
                .get_symbol_if_same_reference(export_equals, symbol)
                .is_some()
            {
                return Some(container);
            }
        }
        let exports = self.collect_exports_of_symbol_identities(container);
        let symbol_name = self.symbol_identity_name(symbol);
        let quick = exports.get(symbol_name.as_str()).copied();
        if let Some(quick) = quick {
            if self.get_symbol_if_same_reference(quick, symbol).is_some() {
                return Some(quick);
            }
        }
        let mut candidates = Vec::new();
        for exported in exports.into_values() {
            if self
                .get_symbol_if_same_reference(exported, symbol)
                .is_some()
            {
                candidates.push(exported);
            }
        }
        if !candidates.is_empty() {
            candidates.sort_by(|&left, &right| self.compare_symbol_identities(left, right)); // _must_ sort exports for stable results - symbol table is randomly iterated
            return Some(candidates[0]);
        }
        None
    }

    pub(crate) fn get_accessible_symbol_chain_identity(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        use_only_external_aliasing: bool,
    ) -> Vec<SymbolIdentity> {
        let mut ctx = AccessibleSymbolChainContext {
            symbol: Some(symbol),
            enclosing_declaration,
            meaning,
            use_only_external_aliasing,
            visited_symbol_tables_map: HashMap::new(),
        };
        self.get_accessible_symbol_chain_ex(&mut ctx)
    }

    pub fn get_accessible_symbol_chain_for_symbol_identity_public(
        &mut self,
        symbol: ast::SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        use_only_external_aliasing: bool,
    ) -> Vec<ast::SymbolIdentity> {
        self.get_accessible_symbol_chain_identity(
            SymbolIdentity::from(symbol),
            enclosing_declaration,
            meaning,
            use_only_external_aliasing,
        )
        .into_iter()
        .map(SymbolIdentity::ast_identity)
        .collect()
    }

    fn get_accessible_symbol_chain_ex(
        &mut self,
        ctx: &mut AccessibleSymbolChainContext,
    ) -> Vec<SymbolIdentity> {
        let Some(symbol) = ctx.symbol else {
            return Vec::new();
        };
        if is_property_or_method_declaration_symbol(self, symbol) {
            return Vec::new();
        }
        // Go from enclosingDeclaration to the first scope we check, so the cache is keyed off the scope and thus shared more
        let mut first_relevant_location = None;
        self.some_symbol_table_in_scope(ctx.enclosing_declaration, |_, _, _, _, _, node| {
            first_relevant_location = node;
            true
        });
        let link_key = AccessibleChainCacheKey {
            use_only_external_aliasing: ctx.use_only_external_aliasing,
            first_relevant_location: first_relevant_location,
            meaning: ctx.meaning,
        };
        if let Some(existing) = self
            .semantic_state
            .accessible_chain_cache_entry(symbol, &link_key)
        {
            return existing;
        }

        let mut result = Vec::new();

        self.some_symbol_table_in_scope(
            ctx.enclosing_declaration,
            |checker, t, table_id, ignore_qualification, is_local_name_lookup, _| {
                let res = checker.get_accessible_symbol_chain_from_symbol_table(
                    ctx,
                    t,
                    table_id,
                    ignore_qualification,
                    is_local_name_lookup,
                );
                if !res.is_empty() {
                    result = res;
                    return true;
                }
                false
            },
        );
        self.semantic_state
            .record_accessible_chain_cache_entry(symbol, link_key, result.clone());
        result
    }

    /**
     * @param {ignoreQualification} boolean Set when a symbol is being looked for through the exports of another symbol (meaning we have a route to qualify it already)
     */
    fn get_accessible_symbol_chain_from_symbol_table(
        &mut self,
        ctx: &mut AccessibleSymbolChainContext,
        t: SymbolTableInScope<'_>,
        table_id: SymbolTableId,
        ignore_qualification: bool,
        is_local_name_lookup: bool,
    ) -> Vec<SymbolIdentity> {
        let sym_id = ctx.symbol.unwrap();
        {
            let visited_symbol_tables = ctx
                .visited_symbol_tables_map
                .entry(sym_id)
                .or_insert_with(HashSet::new);

            if !visited_symbol_tables.insert(table_id) {
                return Vec::new();
            }
        }

        let res = self.try_symbol_table(
            ctx,
            t,
            table_id == SymbolTableId::Globals,
            ignore_qualification,
            is_local_name_lookup,
        );

        ctx.visited_symbol_tables_map
            .get_mut(&sym_id)
            .unwrap()
            .remove(&table_id);
        res
    }

    fn try_symbol_table(
        &mut self,
        ctx: &mut AccessibleSymbolChainContext,
        symbols: SymbolTableInScope<'_>,
        is_globals: bool,
        ignore_qualification: bool,
        is_local_name_lookup: bool,
    ) -> Vec<SymbolIdentity> {
        // If symbol is directly available by its name in the symbol table
        let ctx_symbol = ctx.symbol.unwrap();
        let ctx_symbol_name = self.symbol_identity_name(ctx_symbol);
        let enclosing_declaration = ctx.enclosing_declaration;
        let use_only_external_aliasing = ctx.use_only_external_aliasing;
        let enclosing_declaration_is_external_module =
            enclosing_declaration.is_some_and(|declaration| {
                let Some(store) = self.try_store_for_node(declaration) else {
                    return false;
                };
                ast::get_source_file_of_node(store, Some(declaration))
                    .is_some_and(|sf| is_external_or_common_js_module_node(self, sf))
            });
        let res = symbols.get(self, ctx_symbol_name.as_str());
        if let Some(res) = res {
            if self.is_accessible(
                ctx,
                res,
                /*resolvedAliasSymbol*/ None,
                ignore_qualification,
            ) {
                return vec![ctx_symbol];
            }
        }

        let mut best_candidate_chain: Option<Vec<SymbolIdentity>> = None;
        if let Some(res) = res
            && let Some(export_symbol) = self.symbol_identity_export_symbol(res)
            && self.is_accessible(
                ctx,
                export_symbol, /*resolvedAliasSymbol*/
                None,
                ignore_qualification,
            )
        {
            best_candidate_chain = Some(vec![ctx_symbol]);
        }

        // Keep the same first-shortest/best result as a stable sort, without collecting every chain.
        symbols.for_each_accessibility_entry(self, |checker, entry| {
            let symbol_from_symbol_table = entry.symbol;
            // for every non-default, non-export= alias symbol in scope, check if it refers to or can chain to the target symbol
            let is_alias_candidate = if !entry.name_is_default_or_export_equals
                && checker
                    .symbol_identity_flags(symbol_from_symbol_table)
                    .intersects(ast::SYMBOL_FLAGS_ALIAS)
            {
                let alias_facts = checker.alias_declaration_facts(symbol_from_symbol_table);
                !(alias_facts.is_umd_export && enclosing_declaration_is_external_module)
                    // If `!useOnlyExternalAliasing`, we can use any type of alias to get the name
                    && (!use_only_external_aliasing
                        || alias_facts.has_external_module_import_equals)
                    // If we're looking up a local name to reference directly, omit namespace reexports, otherwise when we're trawling through an export list to make a dotted name, we can keep it
                    && (!is_local_name_lookup || !alias_facts.has_namespace_reexport)
                    // While exports are generally considered to be in scope, export-specifier declared symbols are _not_
                    // See similar comment in `resolveName` for details
                    && (ignore_qualification || !alias_facts.has_export_specifier)
            } else {
                false
            };
            if is_alias_candidate {
                let resolved_imported_symbol =
                    checker.resolve_alias_identity(symbol_from_symbol_table);
                let candidate = checker.get_candidate_list_for_symbol(
                    ctx,
                    symbol_from_symbol_table,
                    resolved_imported_symbol,
                    ignore_qualification,
                );
                if !candidate.is_empty() {
                    if best_candidate_chain
                        .as_ref()
                        .is_none_or(|best| checker.compare_symbol_chains(&candidate, best).is_lt())
                    {
                        best_candidate_chain = Some(candidate);
                    }
                }
            }
        });

        if let Some(best_candidate_chain) = best_candidate_chain {
            return best_candidate_chain;
        }

        // If there's no result and we're looking at the global symbol table, treat `globalThis` like an alias and try to lookup thru that
        if is_globals {
            let global_this_symbol = self.global_this_symbol_identity();
            return self.get_candidate_list_for_symbol(
                ctx,
                global_this_symbol,
                global_this_symbol,
                ignore_qualification,
            );
        }
        Vec::new()
    }

    fn compare_symbol_chains(
        &self,
        left: &[SymbolIdentity],
        right: &[SymbolIdentity],
    ) -> std::cmp::Ordering {
        let chain_len = left.len().cmp(&right.len());
        if chain_len != std::cmp::Ordering::Equal {
            return chain_len;
        }

        for (&left_symbol, &right_symbol) in left.iter().zip(right.iter()) {
            let comparison = self.compare_symbol_identities(left_symbol, right_symbol);
            if comparison != std::cmp::Ordering::Equal {
                return comparison;
            }
        }
        std::cmp::Ordering::Equal
    }

    fn get_candidate_list_for_symbol(
        &mut self,
        ctx: &mut AccessibleSymbolChainContext,
        symbol_from_symbol_table: SymbolIdentity,
        resolved_imported_symbol: SymbolIdentity,
        ignore_qualification: bool,
    ) -> Vec<SymbolIdentity> {
        if self.is_accessible(
            ctx,
            symbol_from_symbol_table,
            Some(resolved_imported_symbol),
            ignore_qualification,
        ) {
            return vec![symbol_from_symbol_table];
        }

        // Look in the exported members, if we can find accessibleSymbolChain, symbol is accessible using this chain
        // but only if the symbolFromSymbolTable can be qualified
        if self.symbol_identity_exports_are_empty(resolved_imported_symbol) {
            return Vec::new();
        }
        let candidate_table_id = symbol_table_id_from_identity_exports(resolved_imported_symbol);
        let accessible_symbols_from_exports = self.get_accessible_symbol_chain_from_symbol_table(
            ctx,
            SymbolTableInScope::Exports(resolved_imported_symbol),
            candidate_table_id, /*ignoreQualification*/
            true,
            false,
        );
        if accessible_symbols_from_exports.is_empty() {
            return Vec::new();
        }
        let can_qualify = self.can_qualify_symbol(
            ctx,
            symbol_from_symbol_table,
            get_qualified_left_meaning(ctx.meaning),
        );
        if !can_qualify {
            return Vec::new();
        }
        let mut result = vec![symbol_from_symbol_table];
        result.extend(accessible_symbols_from_exports);
        result
    }

    fn is_accessible(
        &mut self,
        ctx: &mut AccessibleSymbolChainContext,
        symbol_from_symbol_table: SymbolIdentity,
        resolved_alias_symbol: Option<SymbolIdentity>,
        ignore_qualification: bool,
    ) -> bool {
        let mut like_symbols = false;
        if ctx.symbol == resolved_alias_symbol {
            like_symbols = true;
        }
        if ctx.symbol == Some(symbol_from_symbol_table) {
            like_symbols = true;
        }
        let symbol = self.get_merged_symbol_identity(ctx.symbol);
        if symbol == self.get_merged_symbol_identity(resolved_alias_symbol) {
            like_symbols = true;
        }
        if symbol == self.get_merged_symbol_identity(Some(symbol_from_symbol_table)) {
            like_symbols = true;
        }
        if !like_symbols {
            return false;
        }
        // if the symbolFromSymbolTable is not external module (it could be if it was determined as ambient external module and would be in globals table)
        // and if symbolFromSymbolTable or alias resolution matches the symbol,
        // check the symbol can be qualified, it is only then this symbol is accessible
        !self.any_symbol_handle_declaration(
            symbol_from_symbol_table.symbol_handle(),
            |checker, declaration| {
                let store = checker.store_for_node(declaration);
                has_non_global_augmentation_external_module_symbol(checker, store, declaration)
            },
        ) && (ignore_qualification
            || self
                .get_merged_symbol_identity(Some(symbol_from_symbol_table))
                .is_some_and(|symbol| self.can_qualify_symbol(ctx, symbol, ctx.meaning)))
    }

    fn can_qualify_symbol(
        &mut self,
        ctx: &mut AccessibleSymbolChainContext,
        symbol_from_symbol_table: SymbolIdentity,
        meaning: ast::SymbolFlags,
    ) -> bool {
        // If the symbol is equivalent and doesn't need further qualification, this symbol is accessible
        !self.needs_qualification_identity(symbol_from_symbol_table, ctx.enclosing_declaration, meaning)
            ||
            // If symbol needs qualification, make sure that parent is accessible, if it is then this symbol is accessible too
            {
                let saved_symbol = ctx.symbol;
                let saved_meaning = ctx.meaning;
                ctx.symbol = self.symbol_identity_parent(symbol_from_symbol_table);
                ctx.meaning = get_qualified_left_meaning(meaning);
                let is_accessible = !self.get_accessible_symbol_chain_ex(ctx).is_empty();
                ctx.symbol = saved_symbol;
                ctx.meaning = saved_meaning;
                is_accessible
            }
    }

    pub(crate) fn needs_qualification_identity(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> bool {
        let mut qualify = false;
        self.some_symbol_table_in_scope(
            enclosing_declaration,
            |checker, symbol_table, _, _, _, _| {
                // If symbol of this name is not available in the symbol table we are ok
                let symbol_name = checker.symbol_identity_name(symbol);
                let res = symbol_table.get(checker, symbol_name.as_str());
                let Some(res) = res else {
                    return false;
                };
                let mut symbol_from_symbol_table_identity = res;
                let mut symbol_from_symbol_table =
                    checker.get_merged_symbol_identity(Some(symbol_from_symbol_table_identity));
                // If the symbol with this name is present it should refer to the symbol
                if symbol_from_symbol_table == Some(symbol) {
                    // No need to qualify
                    return true;
                }

                // Qualify if the symbol from symbol table has same meaning as expected
                let should_resolve_alias = {
                    let flags = checker.symbol_identity_flags(symbol_from_symbol_table_identity);
                    flags.intersects(ast::SYMBOL_FLAGS_ALIAS)
                        && !checker.any_symbol_handle_declaration(
                            symbol_from_symbol_table_identity.symbol_handle(),
                            |checker, declaration| {
                                let Some(store) = checker.try_store_for_node(declaration) else {
                                    return false;
                                };
                                store.kind(declaration) == ast::Kind::ExportSpecifier
                            },
                        )
                };
                if should_resolve_alias {
                    let resolved =
                        checker.resolve_alias_identity(symbol_from_symbol_table_identity);
                    symbol_from_symbol_table_identity = resolved;
                    symbol_from_symbol_table = Some(resolved);
                }
                let mut flags = checker.symbol_identity_flags(symbol_from_symbol_table_identity);
                if should_resolve_alias {
                    let Some(symbol_from_symbol_table) = symbol_from_symbol_table else {
                        return false;
                    };
                    flags = checker.missing_name_symbol_identity_flags(symbol_from_symbol_table);
                }
                if flags.intersects(meaning) {
                    qualify = true;
                    return true;
                }

                // Continue to the next symbol table
                false
            },
        );

        qualify
    }

    fn some_symbol_table_in_scope<F>(
        &mut self,
        enclosing_declaration: Option<ast::Node>,
        mut callback: F,
    ) -> bool
    where
        F: FnMut(
            &mut Checker<'a, '_>,
            SymbolTableInScope<'_>,
            SymbolTableId,
            bool,
            bool,
            Option<ast::Node>,
        ) -> bool,
    {
        if enclosing_declaration.is_some_and(|node| self.try_source_file_for_node(node).is_none()) {
            return callback(
                self,
                SymbolTableInScope::Globals,
                symbol_table_id_from_globals(),
                false,
                true,
                None,
            );
        }

        let mut location = enclosing_declaration;
        while let Some(current) = location {
            let store = self.store_for_node(current);
            // Locals of a source file are not in scope (because they get merged into the global symbol table)
            let has_locals = if can_have_locals(store, current)
                && !is_global_source_file_node(self, store, current)
            {
                self.node_has_locals(current)
            } else {
                false
            };
            if has_locals {
                if callback(
                    self,
                    SymbolTableInScope::Locals(current),
                    symbol_table_id_from_locals(store, current),
                    false,
                    true,
                    Some(current),
                ) {
                    return true;
                }
            }
            match store.kind(current) {
                ast::Kind::SourceFile | ast::Kind::ModuleDeclaration => {
                    if ast::is_source_file(store, current)
                        && !is_external_or_common_js_module_node(self, current)
                    {
                        // break
                    } else {
                        let (exports, table_id) =
                            if let Some(sym) = self.get_symbol_of_declaration(current) {
                                (
                                    SymbolTableInScope::RawExports(
                                        SymbolIdentity::from_symbol_handle(sym),
                                    ),
                                    symbol_table_id_from_handle_exports(sym),
                                )
                            } else {
                                (
                                    SymbolTableInScope::Empty,
                                    symbol_table_id_from_empty_exports(store, current),
                                )
                            };
                        if callback(self, exports, table_id, false, true, Some(current)) {
                            return true;
                        }
                    }
                }
                ast::Kind::ClassDeclaration
                | ast::Kind::ClassExpression
                | ast::Kind::InterfaceDeclaration => {
                    // Type parameters are bound into `members` lists so they can merge across declarations
                    // This is troublesome, since in all other respects, they behave like locals :cries:
                    // TODO: the below is shared with similar code in `resolveName` - in fact, rephrasing all this symbol
                    // lookup logic in terms of `resolveName` would be nice
                    // The below is used to lookup type parameters within a class or interface, as they are added to the class/interface locals
                    // These can never be latebound, so the symbol's raw members are sufficient. `getMembersOfNode` cannot be used, as it would
                    // trigger resolving late-bound names, which we may already be in the process of doing while we're here!
                    let mut table: Option<SymbolIdentityTable> = None;
                    let sym = self.get_symbol_of_declaration(current).unwrap();
                    // TODO: Should this filtered table be cached in some way?
                    self.with_symbol_handle_members(sym, |members| {
                        if let Some(members) = members {
                            for (key, member_symbol) in members {
                                if self.symbol_handle_flags(*member_symbol).intersects(
                                    ast::SYMBOL_FLAGS_TYPE & !ast::SYMBOL_FLAGS_ASSIGNMENT,
                                ) {
                                    if table.is_none() {
                                        table = Some(SymbolIdentityTable::default());
                                    }
                                    table.as_mut().unwrap().insert(
                                        key.clone(),
                                        SymbolIdentity::from_symbol_handle(*member_symbol),
                                    );
                                }
                            }
                        }
                    });
                    if let Some(table) = table {
                        if callback(
                            self,
                            SymbolTableInScope::Borrowed(&table),
                            symbol_table_id_from_handle_members(sym),
                            false,
                            false,
                            Some(current),
                        ) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
            location = store.parent(current);
        }

        callback(
            self,
            SymbolTableInScope::Globals,
            symbol_table_id_from_globals(),
            false,
            true,
            None,
        )
    }

    /**
     * Check if the given symbol in given enclosing declaration is accessible and mark all associated alias to be visible if requested
     *
     * @param symbol a Symbol to check if accessible
     * @param enclosingDeclaration a Node containing reference to the symbol
     * @param meaning a SymbolFlags to check if such meaning of the symbol is accessible
     * @param shouldComputeAliasToMakeVisible a boolean value to indicate whether to return aliases to be mark visible in case the symbol is accessible
     */

    pub fn is_symbol_accessible_by_identity(
        &mut self,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        should_compute_aliases_to_make_visible: bool,
    ) -> printer::SymbolAccessibilityResult {
        self.is_symbol_accessible_identity_worker(
            symbol,
            enclosing_declaration,
            meaning,
            should_compute_aliases_to_make_visible,
            true,
        )
    }

    fn is_symbol_accessible_identity_worker(
        &mut self,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        should_compute_aliases_to_make_visible: bool,
        allow_modules: bool,
    ) -> printer::SymbolAccessibilityResult {
        if symbol.is_some() && enclosing_declaration.is_some() {
            let symbol = symbol.unwrap();
            let enclosing_declaration = enclosing_declaration.unwrap();
            let result = self.is_any_symbol_accessible(
                &[symbol],
                enclosing_declaration,
                symbol,
                meaning,
                should_compute_aliases_to_make_visible,
                allow_modules,
            );
            if let Some(result) = result {
                return result;
            }

            // This could be a symbol that is not exported in the external module
            // or it could be a symbol from different external module that is not aliased and hence cannot be named
            let mut symbol_external_module = None;
            self.find_symbol_handle_declaration(symbol.symbol_handle(), |checker, d| {
                symbol_external_module = checker.get_external_module_container_identity(d);
                symbol_external_module.is_some()
            });
            if let Some(symbol_external_module) = symbol_external_module {
                let enclosing_external_module =
                    self.get_external_module_container_identity(enclosing_declaration);
                if !self.same_optional_symbol_identity(
                    Some(symbol_external_module),
                    enclosing_external_module,
                ) {
                    let symbol_name = self.symbol_identity_to_string_ex(
                        symbol,
                        Some(enclosing_declaration),
                        meaning,
                        SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
                    );
                    let module_name = self.symbol_identity_to_string(symbol_external_module);
                    // name from different external module that is not visible
                    return printer::SymbolAccessibilityResult {
                        accessibility: printer::SymbolAccessibility::CannotBeNamed,
                        error_symbol_name: symbol_name,
                        error_module_name: module_name,
                        error_node: if self
                            .try_source_file_for_node(enclosing_declaration)
                            .is_some_and(|source_file| {
                                ast::is_in_js_file(source_file.store(), enclosing_declaration)
                            }) {
                            Some(enclosing_declaration)
                        } else {
                            None
                        },
                        ..Default::default()
                    };
                }
            }

            // Just a local name that is not accessible
            let symbol_name = self.symbol_identity_to_string_ex(
                symbol,
                Some(enclosing_declaration),
                meaning,
                SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
            );
            return printer::SymbolAccessibilityResult {
                accessibility: printer::SymbolAccessibility::NotAccessible,
                error_symbol_name: symbol_name,
                ..Default::default()
            };
        }

        printer::SymbolAccessibilityResult {
            accessibility: printer::SymbolAccessibility::Accessible,
            ..Default::default()
        }
    }
}

pub(crate) fn has_non_global_augmentation_external_module_symbol(
    checker: &Checker<'_, '_>,
    store: &ast::AstStore,
    declaration: ast::Node,
) -> bool {
    ast::is_module_with_string_literal_name(store, declaration)
        || (store.kind(declaration) == ast::Kind::SourceFile
            && is_external_or_common_js_module_node(checker, declaration))
}

pub(crate) fn get_qualified_left_meaning(right_meaning: ast::SymbolFlags) -> ast::SymbolFlags {
    // If we are looking in value space, the parent meaning is value, other wise it is namespace
    if right_meaning == ast::SYMBOL_FLAGS_VALUE {
        return ast::SYMBOL_FLAGS_VALUE;
    }
    ast::SYMBOL_FLAGS_NAMESPACE
}

fn has_external_module_symbol(
    checker: &Checker<'_, '_>,
    store: &ast::AstStore,
    declaration: ast::Node,
) -> bool {
    ast::is_ambient_module(store, declaration)
        || (store.kind(declaration) == ast::Kind::SourceFile
            && is_external_or_common_js_module_node(checker, declaration))
}

struct AccessibleSymbolChainContext {
    symbol: Option<SymbolIdentity>,
    enclosing_declaration: Option<ast::Node>,
    meaning: ast::SymbolFlags,
    use_only_external_aliasing: bool,
    visited_symbol_tables_map: HashMap<SymbolIdentity, HashSet<SymbolTableId>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SymbolTableId {
    Locals(ast::NodeId),
    Exports(ast::SymbolHandle),
    EmptyExports(ast::NodeId),
    Members(ast::SymbolHandle),
    Globals,
}

#[derive(Clone, Copy)]
enum SymbolTableInScope<'table> {
    Borrowed(&'table SymbolIdentityTable),
    Locals(ast::Node),
    RawExports(SymbolIdentity),
    Exports(SymbolIdentity),
    Globals,
    Empty,
}

#[derive(Clone, Copy)]
struct SymbolTableAccessibilityEntry {
    symbol: SymbolIdentity,
    name_is_default_or_export_equals: bool,
}

impl SymbolTableAccessibilityEntry {
    fn new(name: &ast::SymbolName, symbol: SymbolIdentity) -> Self {
        let name = name.as_str();
        Self {
            symbol,
            name_is_default_or_export_equals: name == ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
                || name == ast::INTERNAL_SYMBOL_NAME_DEFAULT,
        }
    }
}

fn materialize_accessibility_entries(
    table: SymbolIdentityTableView<'_>,
) -> Vec<SymbolTableAccessibilityEntry> {
    let mut entries = Vec::with_capacity(table.len());
    table.for_each(|name, symbol| {
        entries.push(SymbolTableAccessibilityEntry::new(name, symbol));
    });
    entries
}

impl SymbolTableInScope<'_> {
    fn get(self, checker: &mut Checker<'_, '_>, name: &str) -> Option<SymbolIdentity> {
        match self {
            Self::Borrowed(table) => table.get(name).copied(),
            Self::Locals(node) => checker.with_node_locals(node, |locals| {
                locals
                    .get(name)
                    .copied()
                    .map(SymbolIdentity::from_symbol_handle)
            })?,
            Self::RawExports(symbol) => checker
                .with_symbol_identity_export_table(symbol, |exports| {
                    exports.and_then(|exports| exports.get(name))
                }),
            Self::Exports(symbol) => checker.with_exports_of_symbol_identities(symbol, |exports| {
                exports.and_then(|exports| exports.get(name))
            }),
            Self::Globals => checker.semantic_state.global_symbol_identity(name),
            Self::Empty => None,
        }
    }

    fn for_each_value(
        self,
        checker: &mut Checker<'_, '_>,
        mut f: impl FnMut(&mut Checker<'_, '_>, SymbolIdentity),
    ) {
        match self {
            Self::Borrowed(table) => {
                for &symbol in table.values() {
                    f(checker, symbol);
                }
            }
            Self::Locals(node) => {
                let local_len = checker.node_local_len(node);
                for index in 0..local_len {
                    if let Some(symbol) = checker
                        .node_local_handle_at(node, index)
                        .map(SymbolIdentity::from_symbol_handle)
                    {
                        f(checker, symbol);
                    }
                }
            }
            Self::RawExports(symbol) => {
                if let Some(exports) = checker.collect_symbol_identity_export_table(symbol) {
                    for symbol in exports.into_values() {
                        f(checker, symbol);
                    }
                }
            }
            Self::Exports(symbol) => {
                let exports = checker.collect_exports_of_symbol_identities(symbol);
                for symbol in exports.into_values() {
                    f(checker, symbol);
                }
            }
            Self::Globals => {
                let global_len = checker.global_symbol_identity_len();
                for index in 0..global_len {
                    if let Some(symbol) = checker.global_symbol_identity_at(index) {
                        f(checker, symbol);
                    }
                }
            }
            Self::Empty => {}
        }
    }

    fn for_each_accessibility_entry(
        self,
        checker: &mut Checker<'_, '_>,
        mut f: impl FnMut(&mut Checker<'_, '_>, SymbolTableAccessibilityEntry),
    ) {
        match self {
            Self::Borrowed(table) => {
                for (name, &symbol) in table {
                    f(checker, SymbolTableAccessibilityEntry::new(name, symbol));
                }
            }
            Self::Locals(node) => {
                let entries = checker
                    .with_node_locals(node, |locals| {
                        let mut entries = Vec::with_capacity(locals.len());
                        for (name, &symbol) in locals {
                            entries.push(SymbolTableAccessibilityEntry::new(
                                name,
                                SymbolIdentity::from_symbol_handle(symbol),
                            ));
                        }
                        entries
                    })
                    .unwrap_or_default();
                for entry in entries {
                    f(checker, entry);
                }
            }
            Self::RawExports(symbol) => {
                let entries = checker.with_symbol_identity_export_table(symbol, |exports| {
                    exports
                        .map(materialize_accessibility_entries)
                        .unwrap_or_default()
                });
                for entry in entries {
                    f(checker, entry);
                }
            }
            Self::Exports(symbol) => {
                let entries = checker.with_exports_of_symbol_identities(symbol, |exports| {
                    exports
                        .map(materialize_accessibility_entries)
                        .unwrap_or_default()
                });
                for entry in entries {
                    f(checker, entry);
                }
            }
            Self::Globals => {
                let entries = checker.semantic_state.with_global_symbols(|globals| {
                    materialize_accessibility_entries(SymbolIdentityTableView::Globals(globals))
                });
                for entry in entries {
                    f(checker, entry);
                }
            }
            Self::Empty => {}
        }
    }
}

fn symbol_table_id_from_locals(store: &ast::AstStore, node: ast::Node) -> SymbolTableId {
    SymbolTableId::Locals(ast::get_node_id(store, node))
}

fn symbol_table_id_from_identity_exports(symbol: SymbolIdentity) -> SymbolTableId {
    let handle = symbol.symbol_handle();
    symbol_table_id_from_handle_exports(handle)
}

fn symbol_table_id_from_handle_exports(sym: ast::SymbolHandle) -> SymbolTableId {
    SymbolTableId::Exports(sym)
}

fn symbol_table_id_from_empty_exports(store: &ast::AstStore, node: ast::Node) -> SymbolTableId {
    SymbolTableId::EmptyExports(ast::get_node_id(store, node))
}

fn symbol_table_id_from_handle_members(sym: ast::SymbolHandle) -> SymbolTableId {
    SymbolTableId::Members(sym)
}

fn symbol_table_id_from_globals() -> SymbolTableId {
    SymbolTableId::Globals
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

#[derive(Default)]
struct AliasDeclarationFacts {
    is_umd_export: bool,
    has_external_module_import_equals: bool,
    has_namespace_reexport: bool,
    has_export_specifier: bool,
}

impl Checker<'_, '_> {
    fn alias_declaration_facts(&self, symbol: SymbolIdentity) -> AliasDeclarationFacts {
        let mut facts = AliasDeclarationFacts::default();
        self.with_symbol_identity_declarations(symbol, |declarations| {
            let mut declarations = declarations.iter().copied();
            let Some(first_declaration) = declarations.next() else {
                return;
            };
            if let Some(store) = self.try_store_for_node(first_declaration) {
                facts.is_umd_export =
                    ast::is_namespace_export_declaration(store, first_declaration);
                update_alias_declaration_facts(&mut facts, store, first_declaration);
            }
            for declaration in declarations {
                let Some(store) = self.try_store_for_node(declaration) else {
                    continue;
                };
                update_alias_declaration_facts(&mut facts, store, declaration);
                if facts.has_external_module_import_equals
                    && facts.has_namespace_reexport
                    && facts.has_export_specifier
                {
                    break;
                }
            }
        });
        facts
    }
}

fn update_alias_declaration_facts(
    facts: &mut AliasDeclarationFacts,
    store: &ast::AstStore,
    declaration: ast::Node,
) {
    facts.has_external_module_import_equals |=
        ast::is_external_module_import_equals_declaration(store, declaration);
    facts.has_namespace_reexport |= is_namespace_reexport_declaration(store, declaration);
    facts.has_export_specifier |= store.kind(declaration) == ast::Kind::ExportSpecifier;
}

fn is_namespace_reexport_declaration(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_namespace_export(store, node)
        && store
            .parent(node)
            .is_some_and(|parent| store.module_specifier(parent).is_some())
}

fn is_property_or_method_declaration_symbol(
    checker: &Checker<'_, '_>,
    symbol: SymbolIdentity,
) -> bool {
    checker.with_symbol_identity_declarations(symbol, |declarations| {
        if !declarations.is_empty() {
            for &declaration in declarations {
                let store = checker.store_for_node(declaration);
                match store.kind(declaration) {
                    ast::Kind::PropertyDeclaration
                    | ast::Kind::MethodDeclaration
                    | ast::Kind::GetAccessor
                    | ast::Kind::SetAccessor => {
                        continue;
                    }
                    _ => {
                        return false;
                    }
                }
            }
            return true;
        }
        false
    })
}
