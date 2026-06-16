use crate::checker::*;
use crate::{ast, nodebuilder};
use ts_collections as collections;

struct NodeBuilderContextScope {
    restore_names: collections::CopyOnWriteMapScope<TypeId, ast::Node>,
    restore_names_by_text: collections::CopyOnWriteSetScope<String>,
    restore_names_by_text_next_name_count: collections::CopyOnWriteMapScope<String, i32>,
    restore_symbol_list: collections::CopyOnWriteSetScope<SymbolIdentity>,
}

impl NodeBuilderContextScope {
    fn restore(self, context: &mut NodeBuilderContext<'_>) {
        self.restore_names
            .restore(&mut context.type_parameter_names);
        self.restore_names_by_text
            .restore(&mut context.type_parameter_names_by_text);
        self.restore_names_by_text_next_name_count
            .restore(&mut context.type_parameter_names_by_text_next_name_count);
        self.restore_symbol_list
            .restore(&mut context.type_parameter_symbol_list);
    }
}

fn clone_node_builder_context(context: &mut NodeBuilderContext<'_>) -> NodeBuilderContextScope {
    // Make type parameters created within this context not consume the name outside this context
    // The symbol serializer ends up creating many sibling scopes that all need "separate" contexts when
    // it comes to naming things - within a normal `typeToTypeNode` call, the node builder only ever descends
    // through the type tree, so the only cases where we could have used distinct sibling scopes was when there
    // were multiple generic overloads with similar generated type parameter names
    // The effect:
    // When we write out
    // export const x: <T>(x: T) => T
    // export const y: <T>(x: T) => T
    // we write it out like that, rather than as
    // export const x: <T>(x: T) => T
    // export const y: <T_1>(x: T_1) => T_1
    let restore_names = context.type_parameter_names.enter_scope();
    let restore_names_by_text = context.type_parameter_names_by_text.enter_scope();
    let restore_names_by_text_next_name_count = context
        .type_parameter_names_by_text_next_name_count
        .enter_scope();
    let restore_symbol_list = context.type_parameter_symbol_list.enter_scope();
    NodeBuilderContextScope {
        restore_names,
        restore_names_by_text,
        restore_names_by_text_next_name_count,
        restore_symbol_list,
    }
}

pub(crate) struct NodeBuilderScopeCleanup {
    cleanup_context: NodeBuilderContextScope,
    fake_scope_cleanups: Vec<FakeScopeCleanup>,
    old_enclosing_decl: Option<ast::Node>,
    replacing_mapper: bool,
    old_mapper: Option<TypeMapperHandle>,
}

struct FakeScopeCleanup {
    scope: ast::Node,
    new_locals: Vec<String>,
    old_locals: Vec<(String, SymbolIdentity)>,
}

impl<'a, 'state, 'c, 'e> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    pub(crate) fn add_symbol_handle_type_to_context(
        &mut self,
        symbol: ast::SymbolHandle,
        t: TypeHandle,
    ) -> Box<dyn FnOnce(&mut NodeBuilderImpl<'a, 'state, 'c, 'e>) + 'e> {
        self.add_symbol_identity_type_to_context(SymbolIdentity::from_symbol_handle(symbol), t)
    }

    pub(crate) fn add_symbol_identity_type_to_context(
        &mut self,
        symbol: SymbolIdentity,
        t: TypeHandle,
    ) -> Box<dyn FnOnce(&mut NodeBuilderImpl<'a, 'state, 'c, 'e>) + 'e> {
        let old_type = self.ctx.enclosing_symbol_types.get(&symbol).copied();
        self.ctx.enclosing_symbol_types.insert(symbol, t);
        Box::new(move |b: &mut NodeBuilderImpl<'a, 'state, 'c, 'e>| {
            if let Some(old_type) = old_type {
                b.ctx.enclosing_symbol_types.insert(symbol, old_type);
            } else {
                b.ctx.enclosing_symbol_types.remove(&symbol);
            }
        })
    }

    pub(crate) fn enter_signature_scope(
        &mut self,
        signature: SignatureHandle,
    ) -> (Vec<SymbolIdentity>, NodeBuilderScopeCleanup) {
        let expanded_params = self
            .ch
            .get_expanded_parameters(signature, true /*skipUnionExpanding*/)[0]
            .clone();
        let signature_record = self.ch.signature_record(signature).clone();
        let mapper = signature_record.mapper;
        let cleanup = self.enter_new_scope(
            signature_record.declaration,
            Some(expanded_params.clone()),
            signature_record.type_parameters,
            Some(signature_record.parameters.to_vec()),
            mapper,
        );
        (expanded_params, cleanup)
    }

    pub(crate) fn enter_new_scope(
        &mut self,
        declaration: Option<ast::Node>,
        expanded_params: Option<Vec<SymbolIdentity>>,
        type_parameters: Vec<TypeHandle>,
        original_parameters: Option<Vec<SymbolIdentity>>,
        mapper: Option<TypeMapperHandle>,
    ) -> NodeBuilderScopeCleanup {
        let cleanup_context = clone_node_builder_context(&mut self.ctx);
        // For regular function/method declarations, the enclosing declaration will already be signature.declaration,
        // so this is a no-op, but for arrow functions and function expressions, the enclosing declaration will be
        // the declaration that the arrow function / function expression is assigned to.
        //
        // If the parameters or return type include "typeof globalThis.paramName", using the wrong scope will lead
        // us to believe that we can emit "typeof paramName" instead, even though that would refer to the parameter,
        // not the global. Make sure we are in the right scope by changing the enclosingDeclaration to the function.
        //
        // We can't use the declaration directly; it may be in another file and so we may lose access to symbols
        // accessible to the current enclosing declaration, or gain access to symbols not accessible to the current
        // enclosing declaration. To keep this chain accurate, insert a fake scope into the chain which makes the
        // function's parameters visible.
        let old_enclosing_decl = self.ctx.enclosing_declaration;
        let replacing_mapper = mapper.is_some();
        let old_mapper = self.ctx.mapper.clone();
        if let Some(mapper) = mapper {
            self.ctx.mapper = Some(mapper);
        }
        if self.ctx.enclosing_declaration.is_some() && declaration.is_some() {
            let mut fake_scope_cleanups = Vec::new();

            if expanded_params
                .as_ref()
                .is_some_and(|params| !params.is_empty())
            {
                let params = expanded_params.clone().unwrap();
                let originals = original_parameters.clone();
                if let Some(cleanup) = self.push_fake_scope("params", |b, add| {
                    for (p_index, param) in params.iter().enumerate() {
                        let original_param = originals.as_ref().and_then(|o| o.get(p_index));
                        if originals.is_some()
                            && !original_param.is_some_and(|original| original == param)
                        {
                            // Can't reference parameters that come from an expansion
                            add(
                                b.ch.symbol_identity_name(*param).to_string(),
                                b.ch.unknown_symbol_identity(),
                            );
                            // Can't reference the original expanded parameter either
                            if let Some(original_param) = original_param {
                                add(
                                    b.ch.symbol_identity_name(*original_param).to_string(),
                                    b.ch.unknown_symbol_identity(),
                                );
                            }
                        } else {
                            let added_binding_pattern = b
                                .ch
                                .collect_symbol_identity_declarations(*param)
                                .iter()
                                .any(|d| {
                                    let store = b.ch.store_for_node(*d);
                                    let name = store.name(*d);
                                    if store.kind(*d) == ast::KIND_PARAMETER
                                        && name.as_ref().is_some_and(|name| {
                                            ast::is_binding_pattern(store, *name)
                                        })
                                    {
                                        b.add_binding_pattern_symbols(*name.as_ref().unwrap(), add);
                                        return true;
                                    }
                                    false
                                });
                            if !added_binding_pattern {
                                add(b.ch.symbol_identity_name(*param).to_string(), *param);
                            }
                        }
                    }
                }) {
                    fake_scope_cleanups.push(cleanup);
                }
            }

            if self.ctx.flags & nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS != 0
                && !type_parameters.is_empty()
            {
                if let Some(cleanup) = self.push_fake_scope("typeParams", |b, add| {
                    for type_param in &type_parameters {
                        let type_param_name = b.type_parameter_to_name(*type_param);
                        let type_param_name =
                            b.e.factory.node_factory.store().text(type_param_name);
                        add(
                            type_param_name,
                            b.ch.type_symbol_identity(*type_param).unwrap(),
                        );
                    }
                }) {
                    fake_scope_cleanups.push(cleanup);
                }
            }

            return NodeBuilderScopeCleanup {
                cleanup_context,
                fake_scope_cleanups,
                old_enclosing_decl,
                replacing_mapper,
                old_mapper,
            };
        }

        NodeBuilderScopeCleanup {
            cleanup_context,
            fake_scope_cleanups: Vec::new(),
            old_enclosing_decl,
            replacing_mapper,
            old_mapper,
        }
    }

    pub(crate) fn exit_scope(&mut self, cleanup: NodeBuilderScopeCleanup) {
        for fake_scope_cleanup in cleanup.fake_scope_cleanups.into_iter().rev() {
            self.restore_fake_scope(fake_scope_cleanup);
        }
        cleanup.cleanup_context.restore(&mut self.ctx);
        self.ctx.enclosing_declaration = cleanup.old_enclosing_decl;
        if cleanup.replacing_mapper {
            self.ctx.mapper = cleanup.old_mapper;
        }
    }

    fn push_fake_scope(
        &mut self,
        kind: &'static str,
        add_all: impl FnOnce(
            &mut NodeBuilderImpl<'a, 'state, 'c, 'e>,
            &mut dyn FnMut(String, SymbolIdentity),
        ),
    ) -> Option<FakeScopeCleanup> {
        // As a performance optimization, reuse the same fake scope within this chain.
        // This is especially needed when we are working on an excessively deep type;
        // if we don't do this, then we spend all of our time adding more and more
        // scopes that need to be searched in isSymbolAccessible later. Since all we
        // really want to do is to mark certain names as unavailable, we can just keep
        // all of the names we're introducing in one large table and push/pop from it as
        // needed; isSymbolAccessible will walk upward and find the closest "fake" scope,
        // which will conveniently report on any and all faked scopes in the chain.
        //
        // It'd likely be better to store this somewhere else for isSymbolAccessible, but
        // since that API _only_ uses the enclosing declaration (and its parents), this is
        // seems like the best way to inject names into that search process.
        //
        // Note that we only check the most immediate enclosingDeclaration; the only place we
        // could potentially add another fake scope into the chain is right here, so we don't
        // traverse all ancestors.
        let enclosing_declaration = self
            .ctx
            .enclosing_declaration
            .expect("push_fake_scope requires an enclosing declaration");
        let existing_fake_scope = if self
            .fake_scope_for_signature_declaration(enclosing_declaration)
            .as_deref()
            == Some(kind)
        {
            Some(enclosing_declaration)
        } else {
            self.store_for_node(enclosing_declaration)
                .parent(enclosing_declaration)
                .filter(|parent| {
                    self.fake_scope_for_signature_declaration(*parent)
                        .as_deref()
                        == Some(kind)
                })
        };

        let mut locals = existing_fake_scope
            .and_then(|scope| self.ch.semantic_state.collect_synthetic_node_locals(scope))
            .unwrap_or_default();
        let mut new_locals = Vec::new();
        let mut old_locals = Vec::new();
        {
            let mut add = |name: String, symbol: SymbolIdentity| {
                if existing_fake_scope.is_some() {
                    if let Some(old_symbol) = locals.get(name.as_str()).copied() {
                        old_locals.push((name.clone(), old_symbol));
                    } else {
                        new_locals.push(name.clone());
                    }
                }
                locals.insert(name.into(), symbol);
            };
            add_all(self, &mut add);
        }

        if let Some(scope) = existing_fake_scope {
            self.ch
                .semantic_state
                .set_synthetic_node_locals(scope, locals);
            Some(FakeScopeCleanup {
                scope,
                new_locals,
                old_locals,
            })
        } else {
            // Use a Block for this; the type of the node doesn't matter so long as it
            // has locals, and this is cheaper/easier than using a function-ish Node.
            let list = self.new_factory_node_list([]);
            let fake_scope = self.e.factory.node_factory.new_block(list, false);
            self.ch
                .semantic_state
                .set_synthetic_node_locals(fake_scope, locals);
            self.e
                .factory
                .node_factory
                .link_checker_synthetic_parent(fake_scope, Some(enclosing_declaration));
            self.with_node_builder_links_mut(fake_scope, |links| {
                links.fake_scope_for_signature_declaration = Some(kind.to_string());
            });
            self.ctx.enclosing_declaration = Some(fake_scope);
            None
        }
    }

    fn restore_fake_scope(&mut self, cleanup: FakeScopeCleanup) {
        let mut locals = self
            .ch
            .semantic_state
            .collect_synthetic_node_locals(cleanup.scope)
            .unwrap_or_default();
        for name in cleanup.new_locals {
            locals.shift_remove(name.as_str());
        }
        for (name, old_symbol) in cleanup.old_locals {
            locals.insert(name.into(), old_symbol);
        }
        self.ch
            .semantic_state
            .set_synthetic_node_locals(cleanup.scope, locals);
    }

    fn add_binding_pattern_symbols(
        &mut self,
        pattern: ast::Node,
        add: &mut dyn FnMut(String, SymbolIdentity),
    ) {
        let store = self.ch.store_for_node(pattern);
        let Some(elements) = store.elements(pattern) else {
            return;
        };
        for element in elements.iter() {
            let element = element;
            match store.kind(element) {
                ast::KIND_OMITTED_EXPRESSION => return,
                ast::KIND_BINDING_ELEMENT => {
                    let element_store = self.ch.store_for_node(element);
                    let name = element_store.name(element);
                    if let Some(name) = name.as_ref() {
                        if ast::is_binding_pattern(element_store, *name) {
                            self.add_binding_pattern_symbols(*name, add);
                            return;
                        }
                    }
                    if let Some(symbol) = self.ch.get_symbol_of_declaration(element) {
                        let name = self.ch.symbol_handle_name(symbol).to_string();
                        let symbol = SymbolIdentity::from_symbol_handle(symbol);
                        add(name, symbol);
                    }
                }
                _ => panic!("Unhandled binding element kind"),
            }
        }
    }
}
