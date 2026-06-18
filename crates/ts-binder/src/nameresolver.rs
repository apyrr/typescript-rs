use ts_ast as ast;
use ts_ast::SymbolFlagsExt;
use ts_core as core;
use ts_diagnostics as diagnostics;

use crate::ProgramBindingState;

fn optional_node_eq(left: Option<&ast::Node>, right: Option<ast::Node>) -> bool {
    left.copied() == right
}

fn optional_node_ref_eq(left: Option<&ast::Node>, right: Option<&ast::Node>) -> bool {
    left.copied() == right.copied()
}

fn is_within_node(
    store: &ast::AstStore,
    location: Option<&ast::Node>,
    ancestor: Option<ast::Node>,
) -> bool {
    let Some(ancestor) = ancestor else {
        return false;
    };
    let mut location = location.cloned();
    while let Some(current) = location {
        if current == ancestor {
            return true;
        }
        location = store.parent(current);
    }
    false
}

pub struct NameResolver<'a> {
    pub store: &'a ast::AstStore,
    pub binding_state: Option<&'a ProgramBindingState>,
    pub hooks: Option<Box<dyn NameResolverHooks<'a> + 'a>>,
    pub store_for_node: Option<Box<dyn FnMut(ast::Node) -> &'a ast::AstStore + 'a>>,
    pub compiler_options: Option<core::CompilerOptions>,
    pub get_symbol_of_declaration:
        Option<Box<dyn FnMut(ast::Node) -> Option<ast::SymbolHandle> + 'a>>,
    pub error: Option<
        Box<dyn FnMut(ast::Node, &'static diagnostics::Message, &[String]) -> ast::Diagnostic + 'a>,
    >,
    pub lookup_global:
        Option<Box<dyn FnMut(&str, ast::SymbolFlags) -> Option<ast::SymbolHandle> + 'a>>,
    pub arguments_symbol: Option<ast::SymbolHandle>,
    pub require_symbol: Option<ast::SymbolHandle>,
    pub lookup: Option<
        Box<
            dyn FnMut(&ast::SymbolHandleTable, &str, ast::SymbolFlags) -> Option<ast::SymbolHandle>
                + 'a,
        >,
    >,
    pub symbol_referenced: Option<Box<dyn FnMut(ast::SymbolHandle, ast::SymbolFlags) + 'a>>,
    pub set_requires_scope_change_cache: Option<Box<dyn FnMut(ast::Node, core::Tristate) + 'a>>,
    pub get_requires_scope_change_cache: Option<Box<dyn FnMut(ast::Node) -> core::Tristate + 'a>>,
    pub on_property_with_invalid_initializer: Option<
        Box<dyn FnMut(Option<ast::Node>, &str, ast::Node, Option<ast::SymbolHandle>) -> bool + 'a>,
    >,
    pub on_failed_to_resolve_symbol: Option<
        Box<
            dyn FnMut(Option<ast::Node>, &str, ast::SymbolFlags, &'static diagnostics::Message)
                + 'a,
        >,
    >,
    pub on_successfully_resolved_symbol: Option<
        Box<
            dyn FnMut(
                    Option<ast::Node>,
                    ast::SymbolHandle,
                    ast::SymbolFlags,
                    Option<ast::Node>,
                    Option<ast::Node>,
                    bool,
                ) + 'a,
        >,
    >,
}

pub trait NameResolverHooks<'a> {
    fn store_for_node(&mut self, node: ast::Node) -> &'a ast::AstStore;
    fn get_symbol_of_declaration(&mut self, node: ast::Node) -> Option<ast::SymbolHandle>;
    fn get_local_symbol_of_declaration(&mut self, node: ast::Node) -> Option<ast::SymbolHandle>;
    fn collect_symbol_declarations(&mut self, symbol: ast::SymbolHandle) -> Vec<ast::Node>;
    fn get_declaration_of_kind(
        &mut self,
        symbol: ast::SymbolHandle,
        kind: ast::Kind,
    ) -> Option<ast::Node>;
    fn get_local_symbol_for_export_default(
        &mut self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolHandle>;
    fn container_has_locals(&mut self, container: ast::Node) -> bool;
    fn lookup_locals_of_container(
        &mut self,
        container: ast::Node,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle>;
    fn source_file_common_js_module_indicator(&mut self, node: ast::Node) -> Option<ast::Node>;
    fn error(
        &mut self,
        location: ast::Node,
        message: &'static diagnostics::Message,
        args: &[String],
    );
    fn lookup(
        &mut self,
        symbols: &ast::SymbolHandleTable,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle>;
    fn symbol_flags(&mut self, symbol: ast::SymbolHandle) -> ast::SymbolFlags;
    fn symbol_name(&mut self, symbol: ast::SymbolHandle) -> ast::SymbolName;
    fn symbol_value_declaration(&mut self, symbol: ast::SymbolHandle) -> Option<ast::Node>;
    fn get_symbol_export(
        &mut self,
        symbol: ast::SymbolHandle,
        name: &str,
    ) -> Option<ast::SymbolHandle>;
    fn lookup_symbol_exports(
        &mut self,
        symbol: ast::SymbolHandle,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle>;
    fn lookup_symbol_members(
        &mut self,
        symbol: ast::SymbolHandle,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle>;
    fn lookup_global(&mut self, name: &str, meaning: ast::SymbolFlags)
    -> Option<ast::SymbolHandle>;
    fn symbol_referenced(&mut self, symbol: ast::SymbolHandle, meaning: ast::SymbolFlags);
    fn set_requires_scope_change_cache(&mut self, node: ast::Node, value: core::Tristate);
    fn get_requires_scope_change_cache(&mut self, node: ast::Node) -> core::Tristate;
    fn on_property_with_invalid_initializer(
        &mut self,
        location: Option<ast::Node>,
        name: &str,
        declaration: ast::Node,
        result: Option<ast::SymbolHandle>,
    ) -> bool;
    fn on_failed_to_resolve_symbol(
        &mut self,
        location: Option<ast::Node>,
        name: &str,
        meaning: ast::SymbolFlags,
        name_not_found_message: &'static diagnostics::Message,
    );
    fn on_successfully_resolved_symbol(
        &mut self,
        location: Option<ast::Node>,
        result: ast::SymbolHandle,
        meaning: ast::SymbolFlags,
        last_location: Option<ast::Node>,
        associated_declaration_for_containing_initializer_or_binding_name: Option<ast::Node>,
        within_deferred_context: bool,
    );
}

impl<'a> NameResolver<'a> {
    fn binding_state(&self) -> &ProgramBindingState {
        self.binding_state
            .expect("name resolver requires binding_state for symbol handles")
    }

    fn symbol_flags(&mut self, symbol: ast::SymbolHandle) -> ast::SymbolFlags {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.symbol_flags(symbol);
        }
        self.binding_state().symbol_flags(symbol)
    }

    fn symbol_name(&mut self, symbol: ast::SymbolHandle) -> ast::SymbolName {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.symbol_name(symbol);
        }
        self.binding_state().symbol_name(symbol).clone()
    }

    fn symbol_value_declaration(&mut self, symbol: ast::SymbolHandle) -> Option<ast::Node> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.symbol_value_declaration(symbol);
        }
        self.binding_state().symbol_value_declaration(symbol)
    }

    fn with_symbol_declarations<R>(
        &mut self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(&mut Self, &[ast::Node]) -> R,
    ) -> R {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            let declarations = hooks.collect_symbol_declarations(symbol);
            return f(self, &declarations);
        }
        let binding_state = self
            .binding_state
            .expect("name resolver requires binding_state for symbol handles");
        binding_state.with_symbol_declarations(symbol, |declarations| f(self, declarations))
    }

    fn get_declaration_of_kind(
        &mut self,
        symbol: ast::SymbolHandle,
        kind: ast::Kind,
    ) -> Option<ast::Node> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.get_declaration_of_kind(symbol, kind);
        }
        self.with_symbol_declarations(symbol, |resolver, declarations| {
            declarations.iter().copied().find(|declaration| {
                let store = resolver.store_for_node(*declaration);
                store.kind(*declaration) == kind
            })
        })
    }

    fn store_for_node(&mut self, node: ast::Node) -> &'a ast::AstStore {
        if node.store_id() == self.store.store_id() {
            return self.store;
        }
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.store_for_node(node);
        }
        let store_for_node = self
            .store_for_node
            .as_mut()
            .expect("name resolver requires store_for_node for cross-file declarations");
        store_for_node(node)
    }

    fn source_file_common_js_module_indicator(&mut self, node: ast::Node) -> Option<ast::Node> {
        if !ast::is_source_file(self.store, node) {
            return None;
        }
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks
                .source_file_common_js_module_indicator(node)
                .or_else(|| self.store.as_source_file(node).common_js_module_indicator());
        }
        self.binding_state
            .filter(|state| state.root() == node)
            .and_then(|state| state.common_js_module_indicator())
            .or_else(|| self.store.as_source_file(node).common_js_module_indicator())
    }

    fn get_local_symbol_of_declaration(&mut self, node: ast::Node) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.get_local_symbol_of_declaration(node);
        }
        self.binding_state
            .and_then(|state| state.exportable_local_symbol(node))
    }

    fn get_local_symbol_for_export_default(
        &mut self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.get_local_symbol_for_export_default(symbol);
        }
        self.with_symbol_declarations(symbol, |resolver, declarations| {
            let first_declaration = declarations.first().copied()?;
            let declaration_store = resolver.store_for_node(first_declaration);
            if !ast::has_syntactic_modifier(
                declaration_store,
                first_declaration,
                ast::ModifierFlags::Default,
            ) {
                return None;
            }
            for &decl in declarations {
                let local_symbol = resolver.get_local_symbol_of_declaration(decl);
                if local_symbol.is_some() {
                    return local_symbol;
                }
            }
            None
        })
    }

    fn container_has_locals(&mut self, container: ast::Node) -> bool {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.container_has_locals(container);
        }
        self.binding_state
            .is_some_and(|state| state.has_locals(container))
    }

    fn lookup_locals_of_container(
        &mut self,
        container: ast::Node,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.lookup_locals_of_container(container, name, meaning);
        }
        let binding_state = self.binding_state?;
        binding_state.with_locals(container, |locals| {
            locals.and_then(|locals| self.lookup(locals, name, meaning))
        })
    }

    fn get_symbol_export(
        &mut self,
        symbol: ast::SymbolHandle,
        name: &str,
    ) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.get_symbol_export(symbol, name);
        }
        self.binding_state().lookup_symbol_export(symbol, name)
    }

    fn lookup_symbol_exports(
        &mut self,
        symbol: ast::SymbolHandle,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.lookup_symbol_exports(symbol, name, meaning);
        }
        if self.lookup.is_some() {
            let binding_state = self
                .binding_state
                .expect("name resolver requires binding_state for symbol handles");
            let lookup = self.lookup.as_mut().unwrap();
            return binding_state.with_symbol_exports(symbol, |exports| {
                exports.and_then(|exports| lookup(exports, name, meaning))
            });
        }
        let binding_state = self.binding_state();
        binding_state.with_symbol_exports(symbol, |exports| {
            exports.and_then(|exports| lookup_symbol_table(binding_state, exports, name, meaning))
        })
    }

    fn lookup_symbol_members(
        &mut self,
        symbol: ast::SymbolHandle,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.lookup_symbol_members(symbol, name, meaning);
        }
        if self.lookup.is_some() {
            let binding_state = self
                .binding_state
                .expect("name resolver requires binding_state for symbol handles");
            let lookup = self.lookup.as_mut().unwrap();
            return binding_state.with_symbol_members(symbol, |members| {
                members.and_then(|members| lookup(members, name, meaning))
            });
        }
        let binding_state = self.binding_state();
        binding_state.with_symbol_members(symbol, |members| {
            members.and_then(|members| lookup_symbol_table(binding_state, members, name, meaning))
        })
    }

    pub fn resolve(
        &mut self,
        location: Option<ast::Node>,
        name: &str,
        meaning: ast::SymbolFlags,
        name_not_found_message: Option<&'static diagnostics::Message>,
        is_use: bool,
        exclude_globals: bool,
    ) -> Option<ast::SymbolHandle> {
        let mut location = location;
        let mut result: Option<ast::SymbolHandle> = None;
        let mut last_location: Option<ast::Node> = None;
        let mut last_self_reference_location: Option<ast::Node> = None;
        let mut property_with_invalid_initializer: Option<ast::Node> = None;
        let mut associated_declaration_for_containing_initializer_or_binding_name: Option<
            ast::Node,
        > = None;
        let mut within_deferred_context = false;
        let original_location = location.clone(); // needed for did-you-mean error reporting, which gathers candidates starting from the original location
        let name_is_const = name == "const";
        while let Some(mut current_location_node) = location.take() {
            if name_is_const && ast::is_const_assertion(self.store, current_location_node) {
                // `const` in an `as const` has no symbol, but issues no error because there is no *actual* lookup of the type
                // (it refers to the constant type of the expression instead)
                return None;
            }
            if (ast::is_module_declaration(self.store, current_location_node)
                || ast::is_enum_declaration(self.store, current_location_node))
                && last_location.is_some()
                && optional_node_eq(
                    last_location.as_ref(),
                    self.store.name(current_location_node),
                )
            {
                // If lastLocation is the name of a namespace or enum, skip the parent since it will have is own locals that could
                // conflict.
                last_location = Some(current_location_node);
                current_location_node = self.store.parent(current_location_node).unwrap();
            }
            // Locals of a source file are not in scope (because they get merged into the global symbol table)
            let stop_lookup = if self.container_has_locals(current_location_node) {
                let is_global_source_file = ast::is_source_file(self.store, current_location_node)
                    && self
                        .store
                        .as_source_file(current_location_node)
                        .external_module_indicator()
                        .is_none()
                    && self
                        .source_file_common_js_module_indicator(current_location_node)
                        .is_none();
                if self.store.kind(current_location_node) != ast::Kind::SourceFile
                    || !is_global_source_file
                {
                    result = self.lookup_locals_of_container(current_location_node, name, meaning);
                    let mut stop_lookup = false;
                    if let Some(&candidate) = result.as_ref() {
                        let candidate_flags = self.symbol_flags(candidate);
                        let mut use_result = true;
                        if ast::is_function_like(self.store, Some(current_location_node))
                            && last_location.is_some()
                            && !optional_node_eq(
                                last_location.as_ref(),
                                self.store.body(current_location_node),
                            )
                            && !is_within_node(
                                self.store,
                                last_location.as_ref(),
                                self.store.body(current_location_node),
                            )
                        {
                            // symbol lookup restrictions for function-like declarations
                            // - Type parameters of a function are in scope in the entire function declaration, including the parameter
                            //   list and return type. However, local types are only in scope in the function body.
                            // - parameters are only in the scope of function body
                            let last_kind = last_location
                                .as_ref()
                                .map(|last| self.store.kind(*last))
                                .unwrap_or(ast::Kind::Unknown);
                            if meaning.intersects(candidate_flags & ast::SYMBOL_FLAGS_TYPE) {
                                // type parameters are visible in parameter list, return type and type parameter list.
                                // Synthetic fake scopes are added for signatures so type parameters are accessible from them.
                                use_result = candidate_flags
                                    .intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER)
                                    && (last_location.as_ref().is_some_and(|last| {
                                        self.store
                                            .flags(*last)
                                            .intersects(ast::NodeFlags::Synthesized)
                                    }) || optional_node_eq(
                                        last_location.as_ref(),
                                        self.store.type_node(current_location_node),
                                    ) || matches!(
                                        last_kind,
                                        ast::Kind::Parameter | ast::Kind::TypeParameter
                                    ));
                            }
                            if meaning.intersects(candidate_flags & ast::SYMBOL_FLAGS_VARIABLE) {
                                // expression inside parameter will lookup as normal variable scope when targeting es2015+
                                if self.use_outer_variable_scope_in_parameter(
                                    candidate,
                                    &current_location_node,
                                    last_location.as_ref().unwrap(),
                                ) {
                                    use_result = false;
                                } else if candidate_flags
                                    .intersects(ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE)
                                {
                                    // parameters are visible only inside function body, parameter list and return type
                                    // technically for parameter list case here we might mix parameters and variables declared in function,
                                    // however it is detected separately when checking initializers of parameters
                                    // to make sure that they reference no variables declared after them.
                                    let last_kind = last_location
                                        .as_ref()
                                        .map(|last| self.store.kind(*last))
                                        .unwrap_or(ast::Kind::Unknown);
                                    use_result = last_kind == ast::Kind::Parameter
                                        || last_location.as_ref().is_some_and(|last| {
                                            self.store
                                                .flags(*last)
                                                .intersects(ast::NodeFlags::Synthesized)
                                        })
                                        || optional_node_eq(
                                            last_location.as_ref(),
                                            self.store.type_node(current_location_node),
                                        ) && ast::find_ancestor(
                                            self.store,
                                            self.symbol_value_declaration(candidate),
                                            |store, node| {
                                                ast::is_parameter_declaration(store, node)
                                            },
                                        )
                                        .is_some();
                                }
                            }
                        } else if self.store.kind(current_location_node)
                            == ast::Kind::ConditionalType
                        {
                            // A type parameter declared using 'infer T' in a conditional type is visible only in
                            // the true branch of the conditional type.
                            use_result = optional_node_ref_eq(
                                last_location.as_ref(),
                                self.store.true_type(current_location_node).as_ref(),
                            );
                        }
                        if use_result {
                            stop_lookup = true;
                        } else {
                            result = None;
                        }
                    }
                    stop_lookup
                } else {
                    false
                }
            } else {
                false
            };
            if stop_lookup {
                break;
            }
            within_deferred_context = within_deferred_context
                || get_is_deferred_context(
                    self.store,
                    &current_location_node,
                    last_location.as_ref(),
                );
            match self.store.kind(current_location_node) {
                ast::Kind::SourceFile | ast::Kind::ModuleDeclaration => {
                    if self.store.kind(current_location_node) == ast::Kind::SourceFile
                        && self
                            .store
                            .as_source_file(current_location_node)
                            .external_module_indicator()
                            .is_none()
                        && self
                            .source_file_common_js_module_indicator(current_location_node)
                            .is_none()
                    {
                        // Go breaks the switch, not the outer loop.
                    } else {
                        let module_symbol = self.get_symbol_of_declaration(current_location_node);
                        if let Some(module_symbol) = module_symbol {
                            let mut skip_module_member_lookup = false;
                            if ast::is_source_file(self.store, current_location_node)
                                || (ast::is_module_declaration(self.store, current_location_node)
                                    && self
                                        .store
                                        .flags(current_location_node)
                                        .intersects(ast::NodeFlags::Ambient)
                                    && !ast::is_global_scope_augmentation(
                                        self.store,
                                        current_location_node,
                                    ))
                            {
                                // It's an external module. First see if the module has an export default and if the local
                                // name of that export default matches.
                                result = self.get_symbol_export(
                                    module_symbol,
                                    ast::INTERNAL_SYMBOL_NAME_DEFAULT,
                                );
                                if let Some(&default_result) = result.as_ref() {
                                    let mut default_matches = false;
                                    let local_symbol =
                                        self.get_local_symbol_for_export_default(default_result);
                                    if let Some(local_symbol) = local_symbol {
                                        if self.symbol_flags(default_result).intersects(meaning)
                                            && self.symbol_name(local_symbol) == name
                                        {
                                            default_matches = true;
                                        }
                                    }
                                    if default_matches {
                                        break;
                                    }
                                    result = None;
                                }
                                // Because of module/namespace merging, a module's exports are in scope,
                                // yet we never want to treat an export specifier as putting a member in scope.
                                // Therefore, if the name we find is purely an export specifier, it is not actually considered in scope.
                                // Two things to note about this:
                                //     1. We have to check this without calling getSymbol. The problem with calling getSymbol
                                //        on an export specifier is that it might find the export specifier itself, and try to
                                //        resolve it as an alias. This will cause the checker to consider the export specifier
                                //        a circular alias reference when it might not be.
                                //     2. We check === SymbolFlags.Alias in order to check that the symbol is *purely*
                                //        an alias. If we used &, we'd be throwing out symbols that have non alias aspects,
                                //        which is not the desired behavior.
                                if let Some(module_export) =
                                    self.get_symbol_export(module_symbol, name)
                                {
                                    if self.symbol_flags(module_export) == ast::SYMBOL_FLAGS_ALIAS
                                        && (self
                                            .get_declaration_of_kind(
                                                module_export,
                                                ast::Kind::ExportSpecifier,
                                            )
                                            .is_some()
                                            || self
                                                .get_declaration_of_kind(
                                                    module_export,
                                                    ast::Kind::NamespaceExport,
                                                )
                                                .is_some())
                                    {
                                        // Go breaks the switch, not the outer loop.
                                        skip_module_member_lookup = true;
                                    }
                                }
                            }
                            if !skip_module_member_lookup
                                && name != ast::INTERNAL_SYMBOL_NAME_DEFAULT
                            {
                                result = self.lookup_symbol_exports(
                                    module_symbol,
                                    name,
                                    meaning & ast::SYMBOL_FLAGS_MODULE_MEMBER,
                                );
                                if let Some(&found) = result.as_ref() {
                                    let reject_found =
                                        ast::is_source_file(self.store, current_location_node)
                                            && self
                                                .source_file_common_js_module_indicator(
                                                    current_location_node,
                                                )
                                                .is_some()
                                            && !self
                                                .symbol_flags(found)
                                                .intersects(ast::SYMBOL_FLAGS_TYPE);
                                    if reject_found {
                                        result = None;
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                ast::Kind::EnumDeclaration => {
                    if let Some(enum_symbol) = self.get_symbol_of_declaration(current_location_node)
                    {
                        result = self.lookup_symbol_exports(
                            enum_symbol,
                            name,
                            meaning & ast::SYMBOL_FLAGS_ENUM_MEMBER,
                        );
                        if let Some(&found) = result.as_ref() {
                            let location_source_file = ast::get_source_file_node_of_node(
                                self.store_for_node(current_location_node),
                                Some(current_location_node),
                            );
                            let value_declaration = self.symbol_value_declaration(found);
                            let value_declaration_source_file =
                                value_declaration.and_then(|value_declaration| {
                                    ast::get_source_file_node_of_node(
                                        self.store_for_node(value_declaration),
                                        Some(value_declaration),
                                    )
                                });
                            if name_not_found_message.is_some()
                                && self
                                    .compiler_options
                                    .as_ref()
                                    .unwrap()
                                    .get_isolated_modules()
                                && !self
                                    .store
                                    .flags(current_location_node)
                                    .intersects(ast::NodeFlags::Ambient)
                                && location_source_file != value_declaration_source_file
                            {
                                let isolated_modules_like_flag_name = core::if_else(
                                    self.compiler_options
                                        .as_ref()
                                        .unwrap()
                                        .verbatim_module_syntax
                                        == core::TSTrue,
                                    "verbatimModuleSyntax",
                                    "isolatedModules",
                                );
                                let enum_symbol_name = self.symbol_name(enum_symbol);
                                self.error(original_location.unwrap(), &diagnostics::CANNOT_ACCESS_0_FROM_ANOTHER_FILE_WITHOUT_QUALIFICATION_WHEN_1_IS_ENABLED_USE_2_INSTEAD, &[
                                    name.to_owned(),
                                    isolated_modules_like_flag_name.to_owned(),
                                    format!("{}.{}", enum_symbol_name, name),
                                ]);
                            }
                            break;
                        }
                    }
                }
                ast::Kind::PropertyDeclaration => {
                    if !ast::is_static(self.store, current_location_node) {
                        let parent = self.store.parent(current_location_node).unwrap();
                        let ctor = ast::find_constructor_declaration(self.store, parent);
                        if let Some(ctor) = ctor {
                            if self
                                .lookup_locals_of_container(
                                    ctor,
                                    name,
                                    meaning & ast::SYMBOL_FLAGS_VALUE,
                                )
                                .is_some()
                            {
                                // Remember the property node, it will be used later to report appropriate error
                                property_with_invalid_initializer = Some(current_location_node);
                            }
                        }
                    }
                }
                ast::Kind::ClassDeclaration
                | ast::Kind::ClassExpression
                | ast::Kind::InterfaceDeclaration => {
                    let container_symbol = self
                        .get_symbol_of_declaration(current_location_node)
                        .unwrap();
                    result = self.lookup_symbol_members(
                        container_symbol,
                        name,
                        meaning & ast::SYMBOL_FLAGS_TYPE,
                    );
                    if let Some(&found) = result.as_ref() {
                        let is_declared = self.is_type_parameter_symbol_declared_in_container(
                            found,
                            &current_location_node,
                        );
                        if !is_declared {
                            // ignore type parameters not declared in this container
                            result = None;
                        } else if last_location
                            .as_ref()
                            .is_some_and(|node| ast::is_static(self.store, *node))
                        {
                            // TypeScript 1.0 spec (April 2014): 3.4.1
                            // The scope of a type parameter extends over the entire declaration with which the type
                            // parameter list is associated, with the exception of static member declarations in classes.
                            if name_not_found_message.is_some() {
                                self.error(original_location.unwrap(), &diagnostics::STATIC_MEMBERS_CANNOT_REFERENCE_CLASS_TYPE_PARAMETERS, &[]);
                            }
                            return None;
                        } else {
                            break;
                        }
                    }
                    if ast::is_class_expression(self.store, current_location_node)
                        && meaning.intersects(ast::SYMBOL_FLAGS_CLASS)
                    {
                        let class_name = self.store.name(current_location_node);
                        if class_name.is_some_and(|class_name| self.store.text(class_name) == name)
                        {
                            result = self.get_symbol_of_declaration(current_location_node);
                            break;
                        }
                    }
                }
                ast::Kind::ExpressionWithTypeArguments => {
                    if optional_node_eq(
                        last_location.as_ref(),
                        self.store.expression(current_location_node),
                    ) && self
                        .store
                        .parent(current_location_node)
                        .as_ref()
                        .is_some_and(|parent| ast::is_heritage_clause(self.store, *parent))
                        && self
                            .store
                            .token(self.store.parent(current_location_node).unwrap())
                            == Some(ast::Kind::ExtendsKeyword)
                    {
                        let container = self
                            .store
                            .parent(self.store.parent(current_location_node).unwrap())
                            .unwrap();
                        if ast::is_class_like(self.store, container) {
                            let container_symbol =
                                self.get_symbol_of_declaration(container).unwrap();
                            result = self.lookup_symbol_members(
                                container_symbol,
                                name,
                                meaning & ast::SYMBOL_FLAGS_TYPE,
                            );
                            if result.is_some() {
                                if name_not_found_message.is_some() {
                                    self.error(original_location.unwrap(), &diagnostics::BASE_CLASS_EXPRESSIONS_CANNOT_REFERENCE_CLASS_TYPE_PARAMETERS, &[]);
                                }
                                return None;
                            }
                        }
                    }
                }
                // It is not legal to reference a class's own type parameters from a computed property name that
                // belongs to the class. For example:
                //
                //   function foo<T>() { return '' }
                //   class C<T> { // <-- Class's own type parameter T
                //       [foo<T>()]() { } // <-- Reference to T from class's own computed property
                //   }
                ast::Kind::ComputedPropertyName => {
                    let grandparent = self
                        .store
                        .parent(current_location_node)
                        .and_then(|parent| self.store.parent(parent));
                    if grandparent.as_ref().is_some_and(|grandparent| {
                        ast::is_class_like(self.store, *grandparent)
                            || ast::is_interface_declaration(self.store, *grandparent)
                    }) {
                        // A reference to this grandparent's type parameters would be an error
                        let grandparent = grandparent.as_ref().unwrap();
                        let grandparent_symbol =
                            self.get_symbol_of_declaration(*grandparent).unwrap();
                        result = self.lookup_symbol_members(
                            grandparent_symbol,
                            name,
                            meaning & ast::SYMBOL_FLAGS_TYPE,
                        );
                        if result.is_some() {
                            if name_not_found_message.is_some() {
                                self.error(original_location.unwrap(), &diagnostics::A_COMPUTED_PROPERTY_NAME_CANNOT_REFERENCE_A_TYPE_PARAMETER_FROM_ITS_CONTAINING_TYPE, &[]);
                            }
                            return None;
                        }
                    }
                }
                ast::Kind::MethodDeclaration
                | ast::Kind::Constructor
                | ast::Kind::GetAccessor
                | ast::Kind::SetAccessor
                | ast::Kind::FunctionDeclaration => {
                    if meaning.intersects(ast::SYMBOL_FLAGS_VARIABLE) && name == "arguments" {
                        result = Some(self.arguments_symbol());
                        break;
                    }
                }
                ast::Kind::FunctionExpression => {
                    if meaning.intersects(ast::SYMBOL_FLAGS_VARIABLE) && name == "arguments" {
                        result = Some(self.arguments_symbol());
                        break;
                    }
                    if meaning.intersects(ast::SYMBOL_FLAGS_FUNCTION) {
                        let function_name = self.store.name(current_location_node);
                        if function_name
                            .is_some_and(|function_name| self.store.text(function_name) == name)
                        {
                            result = self.get_symbol_of_declaration(current_location_node);
                            break;
                        }
                    }
                }
                ast::Kind::Decorator => {
                    // Decorators are resolved at the class declaration. Resolving at the parameter
                    // or member would result in looking up locals in the method.
                    //
                    //   function y() {}
                    //   class C {
                    //       method(@y x, y) {} // <-- decorator y should be resolved at the class declaration, not the parameter.
                    //   }
                    //
                    if self
                        .store
                        .parent(current_location_node)
                        .is_some_and(|parent| self.store.kind(parent) == ast::Kind::Parameter)
                    {
                        current_location_node = self.store.parent(current_location_node).unwrap();
                    }
                    //   function y() {}
                    //   class C {
                    //       @y method(x, y) {} // <-- decorator y should be resolved at the class declaration, not the method.
                    //   }
                    //
                    // class Decorators are resolved outside of the class to avoid referencing type parameters of that class.
                    //
                    //   type T = number;
                    //   declare function y(x: T): any;
                    //   @param(1 as T) // <-- T should resolve to the type alias outside of class C
                    //   class C<T> {}
                    if self
                        .store
                        .parent(current_location_node)
                        .is_some_and(|parent| {
                            ast::is_class_element(self.store, parent)
                                || self.store.kind(parent) == ast::Kind::ClassDeclaration
                        })
                    {
                        current_location_node = self.store.parent(current_location_node).unwrap();
                    }
                }
                ast::Kind::Parameter => {
                    let parameter_initializer = self.store.initializer(current_location_node);
                    let parameter_name = self.store.name(current_location_node);
                    if last_location.is_some()
                        && (optional_node_ref_eq(
                            last_location.as_ref(),
                            parameter_initializer.as_ref(),
                        ) || optional_node_eq(last_location.as_ref(), parameter_name)
                            && ast::is_binding_pattern(
                                self.store,
                                *last_location.as_ref().unwrap(),
                            ))
                        && associated_declaration_for_containing_initializer_or_binding_name
                            .is_none()
                    {
                        associated_declaration_for_containing_initializer_or_binding_name =
                            Some(current_location_node);
                    }
                }
                ast::Kind::BindingElement => {
                    let binding_initializer = self.store.initializer(current_location_node);
                    let binding_name = self.store.name(current_location_node);
                    if last_location.is_some()
                        && (optional_node_ref_eq(
                            last_location.as_ref(),
                            binding_initializer.as_ref(),
                        ) || optional_node_eq(last_location.as_ref(), binding_name)
                            && ast::is_binding_pattern(
                                self.store,
                                *last_location.as_ref().unwrap(),
                            ))
                        && ast::is_part_of_parameter_declaration(self.store, current_location_node)
                        && associated_declaration_for_containing_initializer_or_binding_name
                            .is_none()
                    {
                        associated_declaration_for_containing_initializer_or_binding_name =
                            Some(current_location_node);
                    }
                }
                ast::Kind::InferType => {
                    if meaning.intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER) {
                        let type_parameter =
                            self.store.type_parameter(current_location_node).unwrap();
                        let parameter_name = self.store.name(type_parameter);
                        if parameter_name
                            .is_some_and(|parameter_name| self.store.text(parameter_name) == name)
                        {
                            result = self.get_symbol_of_declaration(type_parameter);
                            break;
                        }
                    }
                }
                ast::Kind::ExportSpecifier => {
                    let property_name = self.store.property_name(current_location_node);
                    if last_location.is_some()
                        && optional_node_ref_eq(last_location.as_ref(), property_name.as_ref())
                        && self
                            .store
                            .module_specifier(
                                self.store
                                    .parent(self.store.parent(current_location_node).unwrap())
                                    .unwrap(),
                            )
                            .is_some()
                    {
                        location = self.store.parent(
                            self.store
                                .parent(self.store.parent(current_location_node).unwrap())
                                .unwrap(),
                        );
                        current_location_node = location.unwrap();
                    }
                }
                _ => {}
            }
            if is_self_reference_location(
                self.store,
                &current_location_node,
                last_location.as_ref(),
            ) {
                last_self_reference_location = Some(current_location_node);
            }
            let next_parent = self.store.parent(current_location_node);
            last_location = Some(current_location_node);
            location = next_parent;
        }
        // We just climbed up parents looking for the name, meaning that we started in a descendant node of `lastLocation`.
        // If `result === lastSelfReferenceLocation.symbol`, that means that we are somewhere inside `lastSelfReferenceLocation` looking up a name, and resolving to `lastLocation` itself.
        // That means that this is a self-reference of `lastLocation`, and shouldn't count this when considering whether `lastLocation` is used.
        if is_use
            && result.is_some()
            && (last_self_reference_location.is_none()
                || result
                    != self
                        .get_symbol_of_declaration(*last_self_reference_location.as_ref().unwrap()))
        {
            if let Some(symbol_referenced) = &mut self.symbol_referenced {
                symbol_referenced(result.unwrap(), meaning);
            } else if self.hooks.is_some() {
                self.symbol_referenced(result.unwrap(), meaning);
            }
        }
        if result.is_none() {
            if let Some(last_location) = last_location {
                debug_assert!(ast::is_source_file(self.store, last_location));
                if self
                    .source_file_common_js_module_indicator(last_location)
                    .is_some()
                    && name == "exports"
                    && self
                        .get_symbol_of_declaration(last_location)
                        .is_some_and(|symbol| self.symbol_flags(symbol).intersects(meaning))
                {
                    return self.get_symbol_of_declaration(last_location);
                }
            }
            if !exclude_globals {
                let meaning = meaning | ast::SYMBOL_FLAGS_GLOBAL_LOOKUP;
                result = self.lookup_global(name, meaning);
            }
        }
        if result.is_none() {
            if original_location.as_ref().is_some_and(|node| {
                self.store
                    .flags(*node)
                    .intersects(ast::NodeFlags::JAVA_SCRIPT_FILE)
            }) && original_location
                .as_ref()
                .and_then(|node| self.store.parent(*node))
                .is_some()
            {
                let parent = self
                    .store
                    .parent(*original_location.as_ref().unwrap())
                    .unwrap();
                if ast::is_require_call(self.store, parent, false) {
                    return self.require_symbol;
                }
            }
        }
        if let Some(name_not_found_message) = name_not_found_message {
            if let Some(property_with_invalid_initializer) = property_with_invalid_initializer {
                if self.on_property_with_invalid_initializer(
                    original_location,
                    name,
                    property_with_invalid_initializer,
                    result,
                ) {
                    return None;
                }
            }
            if result.is_none() {
                self.on_failed_to_resolve_symbol(
                    original_location,
                    name,
                    meaning,
                    name_not_found_message,
                );
            } else {
                self.on_successfully_resolved_symbol(
                    original_location,
                    result.unwrap(),
                    meaning,
                    last_location,
                    associated_declaration_for_containing_initializer_or_binding_name,
                    within_deferred_context,
                );
            }
        }
        result
    }

    fn use_outer_variable_scope_in_parameter(
        &mut self,
        result: ast::SymbolHandle,
        location: &ast::Node,
        last_location: &ast::Node,
    ) -> bool {
        if ast::is_parameter_declaration(self.store, *last_location) {
            let body = self.store.body(*location);
            if let Some(body) = body {
                if let Some(value_declaration) = self.symbol_value_declaration(result) {
                    let value_declaration_loc = self.store.loc(value_declaration);
                    let body_loc = self.store.loc(body);
                    if value_declaration_loc.pos() >= body_loc.pos()
                        && value_declaration_loc.end() <= body_loc.end()
                    {
                        // check for several cases where we introduce temporaries that require moving the name/initializer of the parameter to the body
                        // - static field in a class expression
                        // - optional chaining pre-es2020
                        // - nullish coalesce pre-es2020
                        // - spread assignment in binding pattern pre-es2017
                        let function_location = location;
                        let mut declaration_requires_scope_change =
                            self.get_requires_scope_change_cache(*function_location);
                        if declaration_requires_scope_change == core::TSUnknown {
                            let mut requires_scope_change = false;
                            for p in self
                                .store
                                .parameters(*function_location)
                                .into_iter()
                                .flatten()
                            {
                                if self.requires_scope_change(&p) {
                                    requires_scope_change = true;
                                    break;
                                }
                            }
                            declaration_requires_scope_change =
                                core::if_else(requires_scope_change, core::TSTrue, core::TSFalse);
                            self.set_requires_scope_change_cache(
                                *function_location,
                                declaration_requires_scope_change,
                            );
                        }
                        return declaration_requires_scope_change != core::TSTrue;
                    }
                }
            }
        }
        false
    }

    fn requires_scope_change(&mut self, node: &ast::Node) -> bool {
        self.requires_scope_change_worker(&self.store.name(*node).unwrap())
            || self
                .store
                .initializer(*node)
                .is_some_and(|initializer| self.requires_scope_change_worker(&initializer))
    }

    fn requires_scope_change_worker(&mut self, node: &ast::Node) -> bool {
        match self.store.kind(*node) {
            ast::Kind::ArrowFunction
            | ast::Kind::FunctionExpression
            | ast::Kind::FunctionDeclaration
            | ast::Kind::Constructor => false,
            ast::Kind::MethodDeclaration
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::PropertyAssignment => {
                self.requires_scope_change_worker(&self.store.name(*node).unwrap())
            }
            ast::Kind::PropertyDeclaration => {
                if ast::has_static_modifier(self.store, *node) {
                    return !self
                        .compiler_options
                        .as_ref()
                        .unwrap()
                        .get_emit_standard_class_fields();
                }
                self.requires_scope_change_worker(&self.store.name(*node).unwrap())
            }
            _ => {
                if ast::is_nullish_coalesce(self.store, *node)
                    || ast::is_optional_chain(self.store, *node)
                {
                    return self
                        .compiler_options
                        .as_ref()
                        .unwrap()
                        .get_emit_script_target()
                        < core::ScriptTarget::ES2020;
                }
                if ast::is_binding_element(self.store, *node)
                    && self.store.dot_dot_dot_token(*node).is_some()
                    && self
                        .store
                        .parent(*node)
                        .as_ref()
                        .is_some_and(|parent| ast::is_object_binding_pattern(self.store, *parent))
                {
                    return self
                        .compiler_options
                        .as_ref()
                        .unwrap()
                        .get_emit_script_target()
                        < core::ScriptTarget::ES2017;
                }
                if ast::is_type_node(self.store, *node) {
                    return false;
                }
                let mut children = Vec::new();
                let _ = self.store.for_each_present_child(*node, |child| {
                    children.push(child);
                    std::ops::ControlFlow::Continue(())
                });
                children
                    .iter()
                    .any(|child| self.requires_scope_change_worker(child))
            }
        }
    }

    fn error(
        &mut self,
        location: ast::Node,
        message: &'static diagnostics::Message,
        args: &[String],
    ) {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            hooks.error(location, message, args);
            return;
        }
        if let Some(error) = &mut self.error {
            error(location, message, args);
        }
        // Default implementation does not report errors
    }

    fn get_symbol_of_declaration(&mut self, node: ast::Node) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.get_symbol_of_declaration(node);
        }
        if let Some(get_symbol_of_declaration) = &mut self.get_symbol_of_declaration {
            return get_symbol_of_declaration(node);
        }

        // Default implementation does not support merged symbols.
        self.binding_state.and_then(|state| state.symbol(node))
    }

    fn lookup(
        &mut self,
        symbols: &ast::SymbolHandleTable,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.lookup(symbols, name, meaning);
        }
        if let Some(lookup) = &mut self.lookup {
            return lookup(symbols, name, meaning);
        }
        lookup_symbol_table(self.binding_state(), symbols, name, meaning)
    }

    fn lookup_global(
        &mut self,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<ast::SymbolHandle> {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.lookup_global(name, meaning);
        }
        if let Some(lookup_global) = &mut self.lookup_global {
            return lookup_global(name, meaning);
        }
        None
    }

    fn symbol_referenced(&mut self, symbol: ast::SymbolHandle, meaning: ast::SymbolFlags) {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            hooks.symbol_referenced(symbol, meaning);
        }
    }

    fn set_requires_scope_change_cache(&mut self, node: ast::Node, value: core::Tristate) {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            hooks.set_requires_scope_change_cache(node, value);
            return;
        }
        if let Some(set_cache) = &mut self.set_requires_scope_change_cache {
            set_cache(node, value);
        }
    }

    fn get_requires_scope_change_cache(&mut self, node: ast::Node) -> core::Tristate {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.get_requires_scope_change_cache(node);
        }
        if let Some(get_cache) = &mut self.get_requires_scope_change_cache {
            return get_cache(node);
        }
        core::TSUnknown
    }

    fn on_property_with_invalid_initializer(
        &mut self,
        location: Option<ast::Node>,
        name: &str,
        declaration: ast::Node,
        result: Option<ast::SymbolHandle>,
    ) -> bool {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            return hooks.on_property_with_invalid_initializer(location, name, declaration, result);
        }
        if let Some(callback) = &mut self.on_property_with_invalid_initializer {
            return callback(location, name, declaration, result);
        }
        false
    }

    fn on_failed_to_resolve_symbol(
        &mut self,
        location: Option<ast::Node>,
        name: &str,
        meaning: ast::SymbolFlags,
        name_not_found_message: &'static diagnostics::Message,
    ) {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            hooks.on_failed_to_resolve_symbol(location, name, meaning, name_not_found_message);
            return;
        }
        if let Some(callback) = &mut self.on_failed_to_resolve_symbol {
            callback(location, name, meaning, name_not_found_message);
        }
    }

    fn on_successfully_resolved_symbol(
        &mut self,
        location: Option<ast::Node>,
        result: ast::SymbolHandle,
        meaning: ast::SymbolFlags,
        last_location: Option<ast::Node>,
        associated_declaration_for_containing_initializer_or_binding_name: Option<ast::Node>,
        within_deferred_context: bool,
    ) {
        if let Some(hooks) = self.hooks.as_deref_mut() {
            hooks.on_successfully_resolved_symbol(
                location,
                result,
                meaning,
                last_location,
                associated_declaration_for_containing_initializer_or_binding_name,
                within_deferred_context,
            );
            return;
        }
        if let Some(callback) = &mut self.on_successfully_resolved_symbol {
            callback(
                location,
                result,
                meaning,
                last_location,
                associated_declaration_for_containing_initializer_or_binding_name,
                within_deferred_context,
            );
        }
    }

    fn arguments_symbol(&mut self) -> ast::SymbolHandle {
        self.arguments_symbol
            .expect("name resolver requires arguments_symbol for arguments lookup")
    }

    fn is_type_parameter_symbol_declared_in_container(
        &mut self,
        symbol: ast::SymbolHandle,
        container: &ast::Node,
    ) -> bool {
        self.with_symbol_declarations(symbol, |resolver, declarations| {
            for &decl in declarations {
                let decl_store = resolver.store_for_node(decl);
                if decl_store.kind(decl) == ast::Kind::TypeParameter {
                    let parent = decl_store.parent(decl);
                    if parent.as_ref().is_some_and(|parent| *parent == *container) {
                        return true;
                    }
                }
            }
            false
        })
    }
}

fn lookup_symbol_table(
    binding_state: &ProgramBindingState,
    symbols: &ast::SymbolHandleTable,
    name: &str,
    meaning: ast::SymbolFlags,
) -> Option<ast::SymbolHandle> {
    // Default implementation does not support following aliases or merged symbols.
    if !meaning.is_empty() {
        if let Some(&symbol) = symbols.get(name) {
            if binding_state.symbol_flags(symbol).intersects(meaning) {
                return Some(symbol);
            }
        }
    }
    None
}

pub fn get_local_symbol_for_export_default(
    store: &ast::AstStore,
    binding_state: &ProgramBindingState,
    symbol: ast::SymbolHandle,
) -> Option<ast::SymbolHandle> {
    if !is_export_default_symbol(store, binding_state, Some(symbol))
        || binding_state.symbol_declarations_are_empty(symbol)
    {
        return None;
    }
    binding_state.with_symbol_declarations(symbol, |declarations| {
        for &decl in declarations {
            let local_symbol = binding_state.exportable_local_symbol(decl);
            if local_symbol.is_some() {
                return local_symbol;
            }
        }
        None
    })
}

fn is_export_default_symbol(
    store: &ast::AstStore,
    binding_state: &ProgramBindingState,
    symbol: Option<ast::SymbolHandle>,
) -> bool {
    symbol.is_some_and(|symbol| {
        binding_state.with_symbol_declarations(symbol, |declarations| {
            declarations.first().is_some_and(|declaration| {
                ast::has_syntactic_modifier(store, *declaration, ast::ModifierFlags::Default)
            })
        })
    })
}

fn get_is_deferred_context(
    store: &ast::AstStore,
    location: &ast::Node,
    last_location: Option<&ast::Node>,
) -> bool {
    let location_kind = store.kind(*location);
    if location_kind != ast::Kind::ArrowFunction && location_kind != ast::Kind::FunctionExpression {
        // initializers in instance property declaration of class like entities are executed in constructor and thus deferred
        // A name is evaluated within the enclosing scope - so it shouldn't count as deferred
        return ast::is_type_query_node(store, *location)
            || (ast::is_function_like_declaration(store, Some(*location))
                || location_kind == ast::Kind::PropertyDeclaration
                    && !ast::is_static(store, *location))
                && (last_location.is_none()
                    || !optional_node_eq(last_location, store.name(*location)));
    }
    if last_location.is_some() && optional_node_eq(last_location, store.name(*location)) {
        return false;
    }
    // generator functions and async functions are not inlined in control flow when immediately invoked
    if store.asterisk_token(*location).is_some()
        || ast::has_syntactic_modifier(store, *location, ast::ModifierFlags::Async)
    {
        return true;
    }
    ast::get_immediately_invoked_function_expression(store, *location).is_none()
}

fn is_self_reference_location(
    store: &ast::AstStore,
    node: &ast::Node,
    last_location: Option<&ast::Node>,
) -> bool {
    match store.kind(*node) {
        ast::Kind::Parameter => {
            last_location.is_some() && optional_node_eq(last_location, store.name(*node))
        }
        ast::Kind::FunctionDeclaration
        | ast::Kind::ClassDeclaration
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::JSTypeAliasDeclaration
        | ast::Kind::ModuleDeclaration => true, // For `namespace N { N; }`
        _ => false,
    }
}
