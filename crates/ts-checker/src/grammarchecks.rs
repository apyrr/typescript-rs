// package checker

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ts_ast as ast;
use ts_binder as binder;
use ts_collections as collections;
use ts_core as core;
use ts_debug as debug;
use ts_diagnostics as diagnostics;
use ts_jsnum as jsnum;
use ts_scanner as scanner;
use ts_tspath as tspath;

use crate::checker::*;
use crate::utilities::{
    has_async_modifier, has_readonly_modifier, is_variable_declaration_in_variable_statement,
    new_diagnostic_for_node,
};
fn to_diagnostic_args(args: Vec<Any>) -> Vec<diagnostics::Argument> {
    args.into_iter()
        .map(|arg| Box::new(arg) as diagnostics::Argument)
        .collect()
}

fn scanner_args_to_diagnostic_args(args: &[String]) -> Vec<diagnostics::Argument> {
    args.iter()
        .map(|arg| Box::new(arg.clone()) as diagnostics::Argument)
        .collect()
}

fn source_file_of_node<'a>(store: &'a ast::AstStore, node: ast::Node) -> ast::SourceFileView<'a> {
    let source_file_node = ast::get_source_file_of_node(store, Some(node)).unwrap();
    store.source_file_view(source_file_node)
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn invalid_jsdoc_type_token(&mut self, node: ast::Node) -> Option<(char, bool)> {
        let store = self.store_for_node(node);
        let source_file = source_file_of_node(store, node);
        let source_text = source_file.shared_text();
        let loc = core::TextRange::new(
            scanner::get_token_pos_of_node(&node, &source_file, false) as i32,
            store.loc(node).end(),
        );
        if loc.len() <= 1 {
            return None;
        }

        let text = &source_text[loc.pos() as usize..loc.end() as usize];
        let first = text.as_bytes().first().copied();
        let last = text.as_bytes().last().copied();
        if last == Some(b'!') {
            return Some(('!', true));
        }
        if first == Some(b'!') {
            return Some(('!', false));
        }
        if last == Some(b'?') {
            return Some(('?', true));
        }
        if first == Some(b'?') {
            return Some(('?', false));
        }
        None
    }

    pub(crate) fn check_invalid_non_nullable_type(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if ast::is_in_js_file(store, node) {
            return false;
        }

        let Some((token, postfix)) = self.invalid_jsdoc_type_token(node) else {
            return false;
        };

        let message = if postfix {
            &diagnostics::X_0_at_the_end_of_a_type_is_not_valid_TypeScript_syntax_Did_you_mean_to_write_1
        } else {
            &diagnostics::X_0_at_the_start_of_a_type_is_not_valid_TypeScript_syntax_Did_you_mean_to_write_1
        };
        let mut t = self.get_type_from_type_node_worker(node);
        t = self.get_conditional_flow_type_of_type(t, node);
        if token == '?'
            && t != self.semantic_state.semantic_handles().never_type
            && t != self.semantic_state.semantic_handles().void_type
        {
            t = self.get_nullable_type(
                t,
                if postfix {
                    TYPE_FLAGS_UNDEFINED
                } else {
                    TYPE_FLAGS_NULLABLE
                },
            );
        }
        let type_text = self.type_to_string(t, None);
        self.grammar_error_on_node(
            node,
            message,
            vec![token.to_string().into(), type_text.into()],
        )
    }

    pub(crate) fn grammar_error_on_first_token(
        &mut self,
        node: ast::Node,
        message: &'static diagnostics::Message,
        args: Vec<Any>,
    ) -> bool {
        let store = self.store_for_node(node);
        let source_file = source_file_of_node(store, node);
        if !self.has_parse_diagnostics(&source_file) {
            let span = scanner::get_range_of_token_at_position(
                &source_file,
                store.loc(node).pos() as usize,
            );
            let args = to_diagnostic_args(args);
            self.diagnostics().add(ast::new_diagnostic_with_file(
                Some(source_file.diagnostic_file()),
                span,
                message,
                &args,
            ));
            return true;
        }
        false
    }

    pub(crate) fn grammar_error_at_pos(
        &mut self,
        node_for_source_file: ast::Node,
        start: usize,
        length: usize,
        message: &'static diagnostics::Message,
        args: Vec<Any>,
    ) -> bool {
        let store = self.store_for_node(node_for_source_file);
        let source_file = source_file_of_node(store, node_for_source_file);
        if !self.has_parse_diagnostics(&source_file) {
            let args = to_diagnostic_args(args);
            self.diagnostics().add(ast::new_diagnostic_with_file(
                Some(source_file.diagnostic_file()),
                core::new_text_range(start as i32, (start + length) as i32),
                message,
                &args,
            ));
            return true;
        }
        false
    }

    pub(crate) fn grammar_error_on_node(
        &mut self,
        node: ast::Node,
        message: &'static diagnostics::Message,
        args: Vec<Any>,
    ) -> bool {
        let store = self.store_for_node(node);
        let source_file = source_file_of_node(store, node);
        if !self.has_parse_diagnostics(&source_file) {
            let diagnostic = new_diagnostic_for_node(store, Some(node), message, args);
            self.diagnostics().add(diagnostic);
            return true;
        }
        false
    }

    pub(crate) fn grammar_error_on_node_skipped_on_no_emit(
        &mut self,
        node: ast::Node,
        message: &'static diagnostics::Message,
        args: Vec<Any>,
    ) -> bool {
        let store = self.store_for_node(node);
        let source_file = source_file_of_node(store, node);
        if !self.has_parse_diagnostics(&source_file) {
            let mut d = new_diagnostic_for_node(store, Some(node), message, args);
            d.set_skipped_on_no_emit();
            self.diagnostics().add(d);
            return true;
        }
        false
    }
}

fn get_identifier_from_entity_name_expression(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    match store.kind(node) {
        ast::Kind::Identifier => Some(node),
        ast::Kind::PropertyAccessExpression => store.name(node),
        _ => None,
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn check_grammar_regular_expression_literal(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let source_file = source_file_of_node(store, node);
        if !self.has_parse_diagnostics(&source_file) {
            let last_error: Arc<Mutex<Option<ast::Diagnostic>>> = Arc::new(Mutex::new(None));
            let diagnostics_to_add: Arc<Mutex<Vec<ast::Diagnostic>>> =
                Arc::new(Mutex::new(Vec::new()));
            let diagnostic_file_for_error = source_file.diagnostic_file();
            let last_error_for_callback = last_error.clone();
            let diagnostics_for_callback = diagnostics_to_add.clone();
            let language_version = self.language_version();
            let language_variant = source_file.language_variant();
            let source_text = source_file.shared_text();
            let token_pos = store.loc(node).pos();
            let token_is_regular_expression_literal = {
                let scanner = self.reg_exp_scanner_mut();
                scanner.set_script_target(language_version);
                scanner.set_language_variant(language_variant);
                scanner.set_on_error(Some(Arc::new(Mutex::new(
                    move |message: &diagnostics::Message,
                          start: usize,
                          length: usize,
                          args: &[String]| {
                        let mut last_error = last_error_for_callback
                            .lock()
                            .unwrap_or_else(|err| err.into_inner());
                        let diagnostic_args = scanner_args_to_diagnostic_args(args);
                        if message.category() == diagnostics::Category::Message
                            && last_error.is_some()
                            && start as i32 == last_error.as_ref().unwrap().pos()
                            && length as i32 == last_error.as_ref().unwrap().len()
                        {
                            // For providing spelling suggestions.
                            let err = ast::new_diagnostic(
                                None,
                                core::new_text_range(start as i32, (start + length) as i32),
                                message,
                                &diagnostic_args,
                            );
                            last_error.as_mut().unwrap().add_related_info(err.clone());
                            let mut diagnostics_for_callback = diagnostics_for_callback
                                .lock()
                                .unwrap_or_else(|err| err.into_inner());
                            if let Some(diagnostic) = diagnostics_for_callback.last_mut() {
                                diagnostic.add_related_info(err);
                            }
                        } else if last_error.is_none()
                            || start as i32 != last_error.as_ref().unwrap().pos()
                        {
                            *last_error = Some(ast::new_diagnostic_with_file(
                                Some(diagnostic_file_for_error.clone()),
                                core::new_text_range(start as i32, (start + length) as i32),
                                message,
                                &diagnostic_args,
                            ));
                            diagnostics_for_callback
                                .lock()
                                .unwrap_or_else(|err| err.into_inner())
                                .push(last_error.as_ref().unwrap().clone());
                        }
                    },
                ))));
                scanner.set_text(source_text);
                scanner.reset_token_state(token_pos);
                scanner.scan();
                let token_is_regular_expression_literal =
                    scanner.re_scan_slash_token() == ast::Kind::RegularExpressionLiteral;
                scanner.set_text(Arc::<str>::from(""));
                scanner.set_on_error(None);
                token_is_regular_expression_literal
            };
            debug::assert(token_is_regular_expression_literal, None);
            let has_error = last_error
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .is_some();
            for diagnostic in diagnostics_to_add
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .drain(..)
            {
                self.diagnostics().add(diagnostic);
            }
            return has_error;
        }
        false
    }

    pub(crate) fn check_grammar_private_identifier_expression(
        &mut self,
        priv_id_as_node: ast::Node,
    ) -> bool {
        let store = self.store_for_node(priv_id_as_node);
        if ast::get_containing_class(store, priv_id_as_node).is_none() {
            return self.grammar_error_on_node(
                priv_id_as_node,
                &diagnostics::Private_identifiers_are_not_allowed_outside_class_bodies,
                vec![],
            );
        }

        let parent = store.parent(priv_id_as_node);
        if !parent
            .as_ref()
            .is_some_and(|parent| ast::is_for_in_statement(store, *parent))
        {
            if !ast::is_expression_node(store, priv_id_as_node) {
                return self.grammar_error_on_node(
                    priv_id_as_node,
                    &diagnostics::Private_identifiers_are_only_allowed_in_class_bodies_and_may_only_be_used_as_part_of_a_class_member_declaration_property_access_or_on_the_left_hand_side_of_an_in_expression,
                    vec![],
                );
            }

            let is_in_operation = parent
                .as_ref()
                .is_some_and(|parent| ast::is_binary_expression(store, *parent))
                && store.kind(store.operator_token(parent.unwrap()).unwrap())
                    == ast::Kind::InKeyword;
            if self
                .get_symbol_for_private_identifier_expression(priv_id_as_node)
                .is_none()
                && !is_in_operation
            {
                return self.grammar_error_on_node(
                    priv_id_as_node,
                    &diagnostics::Cannot_find_name_0,
                    vec![
                        self.store_for_node(priv_id_as_node)
                            .text(priv_id_as_node)
                            .into(),
                    ],
                );
            }
        }

        false
    }

    pub(crate) fn check_grammar_mapped_type(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let members = store.members(node);
        if members.is_some_and(|members| !members.is_empty()) {
            return self.grammar_error_on_node(
                members.unwrap().first().unwrap(),
                &diagnostics::A_mapped_type_may_not_declare_properties_or_methods,
                vec![],
            );
        }
        false
    }

    pub(crate) fn check_grammar_decorator(&mut self, decorator: ast::Node) -> bool {
        let store = self.store_for_node(decorator);
        let source_file = source_file_of_node(store, decorator);
        if !self.has_parse_diagnostics(&source_file) {
            let decorator_expression = store.expression(decorator).unwrap();
            let mut node = decorator_expression;

            // DecoratorParenthesizedExpression :
            //   `(` Expression `)`

            if ast::is_parenthesized_expression(store, node) {
                return false;
            }

            let mut can_have_call_expression = true;
            let mut error_node: Option<ast::Node> = None;
            loop {
                // Allow TS syntax such as non-null assertions and instantiation expressions
                if ast::is_expression_with_type_arguments(store, node)
                    || ast::is_non_null_expression(store, node)
                {
                    node = store.expression(node).unwrap();
                    continue;
                }

                // DecoratorCallExpression :
                //   DecoratorMemberExpression Arguments

                if ast::is_call_expression(store, node) {
                    if !can_have_call_expression {
                        error_node = Some(node);
                    }
                    if let Some(question_dot_token) = store.question_dot_token(node) {
                        // Even if we already have an error node, error at the `?.` token since it appears earlier.
                        error_node = Some(question_dot_token);
                    }
                    node = store.expression(node).unwrap();
                    can_have_call_expression = false;
                    continue;
                }

                // DecoratorMemberExpression :
                //   IdentifierReference
                //   DecoratorMemberExpression `.` IdentifierName
                //   DecoratorMemberExpression `.` PrivateIdentifier

                if ast::is_property_access_expression(store, node) {
                    if let Some(question_dot_token) = store.question_dot_token(node) {
                        // Even if we already have an error node, error at the `?.` token since it appears earlier.
                        error_node = Some(question_dot_token);
                    }
                    node = store.expression(node).unwrap();
                    can_have_call_expression = false;
                    continue;
                }

                if !ast::is_identifier(store, node) {
                    // Even if we already have an error node, error at this node since it appears earlier.
                    error_node = Some(node);
                }

                break;
            }

            if let Some(error_node) = error_node {
                let mut err = self.create_error_diagnostic(
                    decorator_expression,
                    &diagnostics::Expression_must_be_enclosed_in_parentheses_to_be_used_as_a_decorator,
                    Vec::<DiagnosticArg>::new(),
                );
                err.add_related_info(new_diagnostic_for_node(
                    self.store_for_node(error_node),
                    Some(error_node),
                    &diagnostics::Invalid_syntax_in_decorator,
                    Vec::<DiagnosticArg>::new(),
                ));
                self.add_error_diagnostic(err);
                return true;
            }
        }

        false
    }

    pub(crate) fn check_grammar_export_declaration(&mut self, node: ast::Node) -> bool {
        let store = self
            .current_source_file()
            .expect("grammar check requires current source file")
            .store();
        let export_clause = store.export_clause(node);
        if store.is_type_only(node).unwrap_or(false)
            && export_clause.is_some()
            && store.kind(export_clause.unwrap()) == ast::Kind::NamedExports
        {
            return self.check_grammar_type_only_named_imports_or_exports(export_clause.unwrap());
        }
        false
    }

    pub(crate) fn check_grammar_module_element_context(
        &mut self,
        node: ast::Statement,
        error_message: &'static diagnostics::Message,
    ) -> bool {
        let store = self.store_for_node(node);
        let parent = store.parent(node).unwrap();
        let is_in_appropriate_context = store.kind(parent) == ast::Kind::SourceFile
            || store.kind(parent) == ast::Kind::ModuleBlock
            || store.kind(parent) == ast::Kind::ModuleDeclaration;
        if !is_in_appropriate_context {
            self.grammar_error_on_first_token(node, error_message, vec![]);
        }
        !is_in_appropriate_context
    }

    pub(crate) fn check_grammar_modifiers(
        &mut self,
        node: ast::Node, /*Union[HasModifiers, HasDecorators, HasIllegalModifiers, HasIllegalDecorators]*/
    ) -> bool {
        let store = self.store_for_node(node);
        if store.modifiers(node).is_none() {
            return false;
        }
        if self.report_obvious_decorator_errors(node) || self.report_obvious_modifier_errors(node) {
            return true;
        }
        if ast::is_this_parameter(store, node) {
            return self.grammar_error_on_first_token(
                node,
                &diagnostics::Neither_decorators_nor_modifiers_may_be_applied_to_this_parameters,
                vec![],
            );
        }
        let mut block_scope_kind = ast::NodeFlags::None;
        if ast::is_variable_statement(store, node) {
            let declaration_list = store.declaration_list(node).unwrap();
            block_scope_kind = store.flags(declaration_list) & ast::NodeFlags::BlockScoped;
        }
        let mut last_static: Option<ast::Node> = None;
        let mut last_declare: Option<ast::Node> = None;
        let mut last_async: Option<ast::Node> = None;
        let mut last_override: Option<ast::Node> = None;
        let mut first_decorator: Option<ast::Node> = None;
        let mut flags = ast::ModifierFlags::None;
        let mut saw_export_before_decorators = false;
        // We parse decorators and modifiers in four contiguous chunks:
        // [...leadingDecorators...leadingModifiers, ...trailingDecorators, ...trailingModifiers]. It is an error to
        // have both leading and trailing decorators.
        let mut has_leading_decorators = false;
        let modifiers = store.modifier_nodes(node);
        for modifier in modifiers {
            if ast::is_decorator(store, modifier) {
                let parent = store.parent(node);
                let grandparent = parent.as_ref().and_then(|parent| store.parent(*parent));
                if !ast::node_can_be_decorated(
                    store,
                    self.legacy_decorators(),
                    node,
                    parent,
                    grandparent,
                ) {
                    if store.kind(node) == ast::Kind::MethodDeclaration
                        && !ast::node_is_present(store, store.body(node))
                    {
                        return self.grammar_error_on_first_token(
                            node,
                            &diagnostics::A_decorator_can_only_decorate_a_method_implementation_not_an_overload,
                            vec![],
                        );
                    } else {
                        return self.grammar_error_on_first_token(
                            node,
                            &diagnostics::Decorators_are_not_valid_here,
                            vec![],
                        );
                    }
                } else if self.legacy_decorators()
                    && (store.kind(node) == ast::Kind::GetAccessor
                        || store.kind(node) == ast::Kind::SetAccessor)
                {
                    let symbol = self.get_symbol_of_declaration(node).unwrap();
                    let accessors = self.with_symbol_handle_declarations(symbol, |declarations| {
                        ast::get_all_accessor_declarations_for_declaration(
                            self.store_for_node(node),
                            node,
                            declarations,
                        )
                    });
                    if ast::has_decorators(
                        self.store_for_node(accessors.first_accessor),
                        accessors.first_accessor,
                    ) && accessors
                        .second_accessor
                        .as_ref()
                        .is_some_and(|second| node == *second)
                    {
                        return self.grammar_error_on_first_token(
                            node,
                            &diagnostics::Decorators_cannot_be_applied_to_multiple_get_Slashset_accessors_of_the_same_name,
                            vec![],
                        );
                    }
                }

                // if we've seen any modifiers aside from `export`, `default`, or another decorator, then this is an invalid position
                if flags & !(ast::ModifierFlags::ExportDefault | ast::ModifierFlags::Decorator) != 0
                {
                    return self.grammar_error_on_node(
                        modifier,
                        &diagnostics::Decorators_are_not_valid_here,
                        vec![],
                    );
                }

                // if we've already seen leading decorators and leading modifiers, then trailing decorators are an invalid position
                if has_leading_decorators && flags & ast::ModifierFlags::Modifier != 0 {
                    if first_decorator.is_none() {
                        panic!("Expected firstDecorator to be set");
                    }
                    let source_file = source_file_of_node(self.store_for_node(modifier), modifier);
                    if !self.has_parse_diagnostics(&source_file) {
                        let mut err = self.create_error_diagnostic(
                            modifier,
                            &diagnostics::Decorators_may_not_appear_after_export_or_export_default_if_they_also_appear_before_export,
                            Vec::<DiagnosticArg>::new(),
                        );
                        err.add_related_info(new_diagnostic_for_node(
                            self.store_for_node(first_decorator.unwrap()),
                            Some(first_decorator.unwrap()),
                            &diagnostics::Decorator_used_before_export_here,
                            Vec::<DiagnosticArg>::new(),
                        ));
                        self.add_error_diagnostic(err);
                        return true;
                    }
                    return false;
                }

                flags |= ast::ModifierFlags::Decorator;

                // if we have not yet seen a modifier, then these are leading decorators
                if flags & ast::ModifierFlags::Modifier == 0 {
                    has_leading_decorators = true;
                } else if flags & ast::ModifierFlags::Export != 0 {
                    saw_export_before_decorators = true;
                }

                if first_decorator.is_none() {
                    first_decorator = Some(modifier);
                }
            } else {
                let modifier_kind = store.kind(modifier);
                if modifier_kind != ast::Kind::ReadonlyKeyword {
                    if store.kind(node) == ast::Kind::PropertySignature
                        || store.kind(node) == ast::Kind::MethodSignature
                    {
                        return self.grammar_error_on_node(
                            modifier,
                            &diagnostics::X_0_modifier_cannot_appear_on_a_type_member,
                            vec![scanner::token_to_string(modifier_kind).into()],
                        );
                    }
                    if store.kind(node) == ast::Kind::IndexSignature
                        && (modifier_kind != ast::Kind::StaticKeyword
                            || !store
                                .parent(node)
                                .is_some_and(|parent| ast::is_class_like(store, parent)))
                    {
                        return self.grammar_error_on_node(
                            modifier,
                            &diagnostics::X_0_modifier_cannot_appear_on_an_index_signature,
                            vec![scanner::token_to_string(modifier_kind).into()],
                        );
                    }
                }
                if modifier_kind != ast::Kind::InKeyword
                    && modifier_kind != ast::Kind::OutKeyword
                    && modifier_kind != ast::Kind::ConstKeyword
                    && store.kind(node) == ast::Kind::TypeParameter
                {
                    return self.grammar_error_on_node(
                        modifier,
                        &diagnostics::X_0_modifier_cannot_appear_on_a_type_parameter,
                        vec![scanner::token_to_string(modifier_kind).into()],
                    );
                }
                match modifier_kind {
                    ast::Kind::ConstKeyword => {
                        if store.kind(node) != ast::Kind::EnumDeclaration
                            && store.kind(node) != ast::Kind::TypeParameter
                        {
                            return self.grammar_error_on_node(
                                node,
                                &diagnostics::A_class_member_cannot_have_the_0_keyword,
                                vec![scanner::token_to_string(ast::Kind::ConstKeyword).into()],
                            );
                        }

                        let parent = store.parent(node).unwrap();

                        if store.kind(node) == ast::Kind::TypeParameter {
                            if !(ast::is_function_like_declaration(store, Some(parent))
                                || ast::is_class_like(store, parent)
                                || ast::is_function_type_node(store, parent)
                                || ast::is_constructor_type_node(store, parent)
                                || ast::is_call_signature_declaration(store, parent)
                                || ast::is_construct_signature_declaration(store, parent)
                                || ast::is_method_signature_declaration(store, parent))
                            {
                                return self.grammar_error_on_node(
                                    modifier,
                                    &diagnostics::X_0_modifier_can_only_appear_on_a_type_parameter_of_a_function_method_or_class,
                                    vec![scanner::token_to_string(modifier_kind).into()],
                                );
                            }
                        }
                    }
                    ast::Kind::OverrideKeyword => {
                        // If node.kind === SyntaxKind.Parameter, checkParameter reports an error if it's not a parameter property.
                        if flags & ast::ModifierFlags::Override != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec!["override".into()],
                            );
                        } else if flags & ast::ModifierFlags::Ambient != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                vec!["override".into(), "declare".into()],
                            );
                        } else if flags & ast::ModifierFlags::Readonly != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["override".into(), "readonly".into()],
                            );
                        } else if flags & ast::ModifierFlags::Accessor != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["override".into(), "accessor".into()],
                            );
                        } else if flags & ast::ModifierFlags::Async != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["override".into(), "async".into()],
                            );
                        }
                        flags |= ast::ModifierFlags::Override;
                        last_override = Some(modifier);
                    }
                    ast::Kind::PublicKeyword
                    | ast::Kind::ProtectedKeyword
                    | ast::Kind::PrivateKeyword => {
                        let text = visibility_to_string(ast::modifier_to_flag(modifier_kind));

                        if flags & ast::ModifierFlags::AccessibilityModifier != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::Accessibility_modifier_already_seen,
                                vec![],
                            );
                        } else if flags & ast::ModifierFlags::Override != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec![text.into(), "override".into()],
                            );
                        } else if flags & ast::ModifierFlags::Static != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec![text.into(), "static".into()],
                            );
                        } else if flags & ast::ModifierFlags::Accessor != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec![text.into(), "accessor".into()],
                            );
                        } else if flags & ast::ModifierFlags::Readonly != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec![text.into(), "readonly".into()],
                            );
                        } else if flags & ast::ModifierFlags::Async != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec![text.into(), "async".into()],
                            );
                        } else if store.kind(store.parent(node).unwrap()) == ast::Kind::ModuleBlock
                            || store.kind(store.parent(node).unwrap()) == ast::Kind::SourceFile
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_module_or_namespace_element,
                                vec![text.into()],
                            );
                        } else if flags & ast::ModifierFlags::Abstract != 0 {
                            if modifier_kind == ast::Kind::PrivateKeyword {
                                return self.grammar_error_on_node(
                                    modifier,
                                    &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                    vec![text.into(), "abstract".into()],
                                );
                            } else if store.flags(modifier) & ast::NodeFlags::Reparsed == 0 {
                                return self.grammar_error_on_node(
                                    modifier,
                                    &diagnostics::X_0_modifier_must_precede_1_modifier,
                                    vec![text.into(), "abstract".into()],
                                );
                            }
                        } else if ast::is_private_identifier_class_element_declaration(store, node)
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::An_accessibility_modifier_cannot_be_used_with_a_private_identifier,
                                vec![],
                            );
                        }
                        flags |= ast::modifier_to_flag(modifier_kind);
                    }
                    ast::Kind::StaticKeyword => {
                        if flags & ast::ModifierFlags::Static != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec!["static".into()],
                            );
                        } else if flags & ast::ModifierFlags::Readonly != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["static".into(), "readonly".into()],
                            );
                        } else if flags & ast::ModifierFlags::Async != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["static".into(), "async".into()],
                            );
                        } else if flags & ast::ModifierFlags::Accessor != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["static".into(), "accessor".into()],
                            );
                        } else if store.kind(store.parent(node).unwrap()) == ast::Kind::ModuleBlock
                            || store.kind(store.parent(node).unwrap()) == ast::Kind::SourceFile
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_module_or_namespace_element,
                                vec!["static".into()],
                            );
                        } else if store.kind(node) == ast::Kind::Parameter {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_parameter,
                                vec!["static".into()],
                            );
                        } else if flags & ast::ModifierFlags::Abstract != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                vec!["static".into(), "abstract".into()],
                            );
                        } else if flags & ast::ModifierFlags::Override != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["static".into(), "override".into()],
                            );
                        }
                        flags |= ast::ModifierFlags::Static;
                        last_static = Some(modifier);
                    }
                    ast::Kind::AccessorKeyword => {
                        if flags & ast::ModifierFlags::Accessor != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec!["accessor".into()],
                            );
                        } else if flags & ast::ModifierFlags::Readonly != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                vec!["accessor".into(), "readonly".into()],
                            );
                        } else if flags & ast::ModifierFlags::Ambient != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                vec!["accessor".into(), "declare".into()],
                            );
                        } else if store.kind(node) != ast::Kind::PropertyDeclaration {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_accessor_modifier_can_only_appear_on_a_property_declaration,
                                vec![],
                            );
                        }

                        flags |= ast::ModifierFlags::Accessor;
                    }
                    ast::Kind::ReadonlyKeyword => {
                        if flags & ast::ModifierFlags::Readonly != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec!["readonly".into()],
                            );
                        } else if store.kind(node) != ast::Kind::PropertyDeclaration
                            && store.kind(node) != ast::Kind::PropertySignature
                            && store.kind(node) != ast::Kind::IndexSignature
                            && store.kind(node) != ast::Kind::Parameter
                        {
                            // If node.kind === SyntaxKind.Parameter, checkParameter reports an error if it's not a parameter property.
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_readonly_modifier_can_only_appear_on_a_property_declaration_or_index_signature,
                                vec![],
                            );
                        } else if flags & ast::ModifierFlags::Accessor != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                vec!["readonly".into(), "accessor".into()],
                            );
                        }
                        flags |= ast::ModifierFlags::Readonly;
                    }
                    ast::Kind::ExportKeyword => {
                        if self.compiler_options.verbatim_module_syntax == core::TSTrue
                            && store.flags(node) & ast::NodeFlags::Ambient == 0
                            && store.kind(node) != ast::Kind::TypeAliasDeclaration
                            && store.kind(node) != ast::Kind::InterfaceDeclaration
                            && store.kind(node) != ast::Kind::ModuleDeclaration
                            && store.kind(store.parent(node).unwrap()) == ast::Kind::SourceFile
                            && self
                                .program
                                .get_emit_module_format_of_file(&source_file_of_node(store, node))
                                == core::ModuleKind::CommonJS
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::A_top_level_export_modifier_cannot_be_used_on_value_declarations_in_a_CommonJS_module_when_verbatimModuleSyntax_is_enabled,
                                vec![],
                            );
                        }
                        if flags & ast::ModifierFlags::Export != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec!["export".into()],
                            );
                        } else if flags & ast::ModifierFlags::Ambient != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["export".into(), "declare".into()],
                            );
                        } else if flags & ast::ModifierFlags::Abstract != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["export".into(), "abstract".into()],
                            );
                        } else if flags & ast::ModifierFlags::Async != 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["export".into(), "async".into()],
                            );
                        } else if store
                            .parent(node)
                            .is_some_and(|parent| ast::is_class_like(store, parent))
                            && !ast::is_js_type_alias_declaration(store, node)
                        {
                            return self.grammar_error_on_node(modifier, &diagnostics::X_0_modifier_cannot_appear_on_class_elements_of_this_kind, vec!["export".into()]);
                        } else if store.kind(node) == ast::Kind::Parameter {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_parameter,
                                vec!["export".into()],
                            );
                        } else if block_scope_kind == ast::NodeFlags::Using {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_using_declaration,
                                vec!["export".into()],
                            );
                        } else if block_scope_kind == ast::NodeFlags::AwaitUsing {
                            return self.grammar_error_on_node(modifier, &diagnostics::X_0_modifier_cannot_appear_on_an_await_using_declaration, vec!["export".into()]);
                        }
                        flags |= ast::ModifierFlags::Export;
                    }
                    ast::Kind::DefaultKeyword => {
                        let parent = store.parent(node).unwrap();
                        let container = if store.kind(parent) == ast::Kind::SourceFile {
                            parent
                        } else {
                            store.parent(parent).unwrap()
                        };
                        if store.kind(container) == ast::Kind::ModuleDeclaration
                            && !ast::is_ambient_module(store, container)
                        {
                            return self.grammar_error_on_node(modifier, &diagnostics::A_default_export_can_only_be_used_in_an_ECMAScript_style_module, vec![]);
                        } else if block_scope_kind == ast::NodeFlags::Using {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_using_declaration,
                                vec!["default".into()],
                            );
                        } else if block_scope_kind == ast::NodeFlags::AwaitUsing {
                            return self.grammar_error_on_node(modifier, &diagnostics::X_0_modifier_cannot_appear_on_an_await_using_declaration, vec!["default".into()]);
                        } else if flags & ast::ModifierFlags::Export == 0
                            && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["export".into(), "default".into()],
                            );
                        } else if saw_export_before_decorators {
                            return self.grammar_error_on_node(
                                first_decorator.unwrap(),
                                &diagnostics::Decorators_are_not_valid_here,
                                vec![],
                            );
                        }

                        flags |= ast::ModifierFlags::Default;
                    }
                    ast::Kind::DeclareKeyword => {
                        if flags & ast::ModifierFlags::Ambient != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec!["declare".into()],
                            );
                        } else if flags & ast::ModifierFlags::Async != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_in_an_ambient_context,
                                vec!["async".into()],
                            );
                        } else if flags & ast::ModifierFlags::Override != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_in_an_ambient_context,
                                vec!["override".into()],
                            );
                        } else if self
                            .node_parent(node)
                            .is_some_and(|parent| ast::is_class_like(store, parent))
                            && !ast::is_property_declaration(store, node)
                        {
                            return self.grammar_error_on_node(modifier, &diagnostics::X_0_modifier_cannot_appear_on_class_elements_of_this_kind, vec!["declare".into()]);
                        } else if store.kind(node) == ast::Kind::Parameter {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_parameter,
                                vec!["declare".into()],
                            );
                        } else if block_scope_kind == ast::NodeFlags::Using {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_using_declaration,
                                vec!["declare".into()],
                            );
                        } else if block_scope_kind == ast::NodeFlags::AwaitUsing {
                            return self.grammar_error_on_node(modifier, &diagnostics::X_0_modifier_cannot_appear_on_an_await_using_declaration, vec!["declare".into()]);
                        } else if {
                            let parent = self.node_parent(node).unwrap();
                            store.flags(parent) & ast::NodeFlags::Ambient != 0
                                && store.kind(parent) == ast::Kind::ModuleBlock
                        } {
                            return self.grammar_error_on_node(modifier, &diagnostics::A_declare_modifier_cannot_be_used_in_an_already_ambient_context, vec![]);
                        } else if ast::is_private_identifier_class_element_declaration(
                            self.store_for_node(node),
                            node,
                        ) {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_a_private_identifier,
                                vec!["declare".into()],
                            );
                        } else if flags & ast::ModifierFlags::Accessor != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                vec!["declare".into(), "accessor".into()],
                            );
                        }
                        flags |= ast::ModifierFlags::Ambient;
                        last_declare = Some(modifier);
                    }
                    ast::Kind::AbstractKeyword => {
                        if flags & ast::ModifierFlags::Abstract != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec!["abstract".into()],
                            );
                        }
                        if store.kind(node) != ast::Kind::ClassDeclaration
                            && store.kind(node) != ast::Kind::ConstructorType
                        {
                            if store.kind(node) != ast::Kind::MethodDeclaration
                                && store.kind(node) != ast::Kind::PropertyDeclaration
                                && store.kind(node) != ast::Kind::GetAccessor
                                && store.kind(node) != ast::Kind::SetAccessor
                            {
                                return self.grammar_error_on_node(modifier, &diagnostics::X_abstract_modifier_can_only_appear_on_a_class_method_or_property_declaration, vec![]);
                            }
                            let parent = self.node_parent(node).unwrap();
                            let parent_store = self.store_for_node(parent);
                            if !(parent_store.kind(parent) == ast::Kind::ClassDeclaration
                                && ast::has_syntactic_modifier(
                                    parent_store,
                                    parent,
                                    ast::ModifierFlags::Abstract,
                                ))
                            {
                                let message = if store.kind(node) == ast::Kind::PropertyDeclaration
                                {
                                    &diagnostics::Abstract_properties_can_only_appear_within_an_abstract_class
                                } else {
                                    &diagnostics::Abstract_methods_can_only_appear_within_an_abstract_class
                                };
                                return self.grammar_error_on_node(modifier, message, vec![]);
                            }
                            if flags & ast::ModifierFlags::Static != 0 {
                                return self.grammar_error_on_node(
                                    modifier,
                                    &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                    vec!["static".into(), "abstract".into()],
                                );
                            }
                            if flags & ast::ModifierFlags::Private != 0 {
                                return self.grammar_error_on_node(
                                    modifier,
                                    &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                    vec!["private".into(), "abstract".into()],
                                );
                            }
                            if flags & ast::ModifierFlags::Async != 0 && last_async.is_some() {
                                return self.grammar_error_on_node(
                                    last_async.unwrap(),
                                    &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                    vec!["async".into(), "abstract".into()],
                                );
                            }
                            if flags & ast::ModifierFlags::Override != 0
                                && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                            {
                                return self.grammar_error_on_node(
                                    modifier,
                                    &diagnostics::X_0_modifier_must_precede_1_modifier,
                                    vec!["abstract".into(), "override".into()],
                                );
                            }
                            if flags & ast::ModifierFlags::Accessor != 0
                                && store.flags(modifier) & ast::NodeFlags::Reparsed == 0
                            {
                                return self.grammar_error_on_node(
                                    modifier,
                                    &diagnostics::X_0_modifier_must_precede_1_modifier,
                                    vec!["abstract".into(), "accessor".into()],
                                );
                            }
                        }
                        if self
                            .node_name(node)
                            .is_some_and(|name| store.kind(name) == ast::Kind::PrivateIdentifier)
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_a_private_identifier,
                                vec!["abstract".into()],
                            );
                        }

                        flags |= ast::ModifierFlags::Abstract;
                    }
                    ast::Kind::AsyncKeyword => {
                        if flags & ast::ModifierFlags::Async != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec!["async".into()],
                            );
                        } else if flags & ast::ModifierFlags::Ambient != 0
                            || store.flags(self.node_parent(node).unwrap())
                                & ast::NodeFlags::Ambient
                                != 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_in_an_ambient_context,
                                vec!["async".into()],
                            );
                        } else if store.kind(node) == ast::Kind::Parameter {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_appear_on_a_parameter,
                                vec!["async".into()],
                            );
                        }
                        if flags & ast::ModifierFlags::Abstract != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_cannot_be_used_with_1_modifier,
                                vec!["async".into(), "abstract".into()],
                            );
                        }
                        flags |= ast::ModifierFlags::Async;
                        last_async = Some(modifier);
                    }
                    ast::Kind::InKeyword | ast::Kind::OutKeyword => {
                        let in_out_flag = if modifier_kind == ast::Kind::InKeyword {
                            ast::ModifierFlags::In
                        } else {
                            ast::ModifierFlags::Out
                        };
                        let in_out_text = if modifier_kind == ast::Kind::InKeyword {
                            "in"
                        } else {
                            "out"
                        };
                        let parent = self.node_parent(node);
                        if store.kind(node) != ast::Kind::TypeParameter
                            || parent.as_ref().is_some_and(|parent| {
                                !(ast::is_interface_declaration(store, *parent)
                                    || ast::is_class_like(store, *parent)
                                    || ast::is_type_alias_declaration(store, *parent))
                            })
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_can_only_appear_on_a_type_parameter_of_a_class_interface_or_type_alias,
                                vec![in_out_text.into()],
                            );
                        }
                        if flags & in_out_flag != 0 {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_already_seen,
                                vec![in_out_text.into()],
                            );
                        }
                        if in_out_flag & ast::ModifierFlags::In != 0
                            && flags & ast::ModifierFlags::Out != 0
                        {
                            return self.grammar_error_on_node(
                                modifier,
                                &diagnostics::X_0_modifier_must_precede_1_modifier,
                                vec!["in".into(), "out".into()],
                            );
                        }
                        flags |= in_out_flag;
                    }
                    _ => {}
                }
            }
        }

        if store.kind(node) == ast::Kind::Constructor {
            if flags & ast::ModifierFlags::Static != 0 {
                return self.grammar_error_on_node(
                    last_static.unwrap(),
                    &diagnostics::X_0_modifier_cannot_appear_on_a_constructor_declaration,
                    vec!["static".into()],
                );
            }
            if flags & ast::ModifierFlags::Override != 0 {
                return self.grammar_error_on_node(
                    last_override.unwrap(),
                    &diagnostics::X_0_modifier_cannot_appear_on_a_constructor_declaration,
                    vec!["override".into()],
                );
            }
            if flags & ast::ModifierFlags::Async != 0 {
                return self.grammar_error_on_node(
                    last_async.unwrap(),
                    &diagnostics::X_0_modifier_cannot_appear_on_a_constructor_declaration,
                    vec!["async".into()],
                );
            }
            return false;
        } else if (store.kind(node) == ast::Kind::ImportDeclaration
            || store.kind(node) == ast::Kind::JSImportDeclaration
            || store.kind(node) == ast::Kind::ImportEqualsDeclaration)
            && flags & ast::ModifierFlags::Ambient != 0
        {
            return self.grammar_error_on_node(
                last_declare.unwrap(),
                &diagnostics::A_0_modifier_cannot_be_used_with_an_import_declaration,
                vec!["declare".into()],
            );
        } else if store.kind(node) == ast::Kind::Parameter
            && (flags & ast::ModifierFlags::ParameterPropertyModifier != 0)
            && self
                .node_name(node)
                .as_ref()
                .is_some_and(|name| ast::is_binding_pattern(store, *name))
        {
            return self.grammar_error_on_node(
                node,
                &diagnostics::A_parameter_property_may_not_be_declared_using_a_binding_pattern,
                vec![],
            );
        } else if store.kind(node) == ast::Kind::Parameter
            && (flags & ast::ModifierFlags::ParameterPropertyModifier != 0)
            && self.store_for_node(node).dot_dot_dot_token(node).is_some()
        {
            return self.grammar_error_on_node(
                node,
                &diagnostics::A_parameter_property_cannot_be_declared_using_a_rest_parameter,
                vec![],
            );
        }
        if flags & ast::ModifierFlags::Async != 0 {
            return self.check_grammar_async_modifier(node, last_async.unwrap());
        }
        false
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    fn report_obvious_modifier_errors(&mut self, node: ast::Node) -> bool {
        let modifier = self.find_first_illegal_modifier(node);
        if modifier.is_none() {
            return false;
        }
        let modifier = modifier.unwrap();
        self.grammar_error_on_first_token(
            modifier,
            &diagnostics::Modifiers_cannot_appear_here,
            vec![],
        )
    }

    fn find_first_modifier_except(
        &mut self,
        node: ast::Node,
        allowed_modifier: ast::Kind,
    ) -> Option<ast::Node> {
        self.store_for_node(node)
            .modifier_nodes(node)
            .iter()
            .find(|modifier| {
                let store = self.store_for_node(**modifier);
                ast::is_modifier(store, **modifier) && store.kind(**modifier) != allowed_modifier
            })
            .copied()
    }

    fn find_first_illegal_modifier(&mut self, node: ast::Node) -> Option<ast::Node> {
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::Constructor
            | ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::IndexSignature
            | ast::Kind::ModuleDeclaration
            | ast::Kind::ImportDeclaration
            | ast::Kind::JSImportDeclaration
            | ast::Kind::ImportEqualsDeclaration
            | ast::Kind::ExportDeclaration
            | ast::Kind::ExportAssignment
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::Parameter
            | ast::Kind::TypeParameter
            | ast::Kind::JSTypeAliasDeclaration => None,
            ast::Kind::ClassStaticBlockDeclaration
            | ast::Kind::PropertyAssignment
            | ast::Kind::ShorthandPropertyAssignment
            | ast::Kind::NamespaceExportDeclaration
            | ast::Kind::MissingDeclaration => self
                .store_for_node(node)
                .modifier_nodes(node)
                .iter()
                .find(|modifier| ast::is_modifier(store, **modifier))
                .copied(),
            _ => {
                let parent = self.node_parent(node).unwrap();
                if store.kind(parent) == ast::Kind::ModuleBlock
                    || store.kind(parent) == ast::Kind::SourceFile
                {
                    return None;
                }
                match store.kind(node) {
                    ast::Kind::FunctionDeclaration => {
                        self.find_first_modifier_except(node, ast::Kind::AsyncKeyword)
                    }
                    ast::Kind::ClassDeclaration | ast::Kind::ConstructorType => {
                        self.find_first_modifier_except(node, ast::Kind::AbstractKeyword)
                    }
                    ast::Kind::ClassExpression
                    | ast::Kind::InterfaceDeclaration
                    | ast::Kind::TypeAliasDeclaration => self
                        .store_for_node(node)
                        .modifier_nodes(node)
                        .iter()
                        .find(|modifier| ast::is_modifier(store, **modifier))
                        .copied(),
                    ast::Kind::VariableStatement => {
                        if store
                            .declaration_list(node)
                            .map(|declaration_list| store.flags(declaration_list))
                            .unwrap()
                            & ast::NodeFlags::Using
                            != 0
                        {
                            return self.find_first_modifier_except(node, ast::Kind::AwaitKeyword);
                        }
                        store
                            .modifier_nodes(node)
                            .iter()
                            .find(|modifier| ast::is_modifier(store, **modifier))
                            .copied()
                    }
                    ast::Kind::EnumDeclaration => {
                        self.find_first_modifier_except(node, ast::Kind::ConstKeyword)
                    }
                    _ => panic!("Unhandled case in findFirstIllegalModifier."),
                }
            }
        }
    }

    fn report_obvious_decorator_errors(&mut self, node: ast::Node) -> bool {
        let decorator = self.find_first_illegal_decorator(node);
        if decorator.is_none() {
            return false;
        }
        let decorator = decorator.unwrap();
        self.grammar_error_on_first_token(
            decorator,
            &diagnostics::Decorators_are_not_valid_here,
            vec![],
        )
    }

    fn find_first_illegal_decorator(&mut self, node: ast::Node) -> Option<ast::Node> {
        let store = self.store_for_node(node);
        if ast::can_have_illegal_decorators(store, node) {
            store
                .modifier_nodes(node)
                .iter()
                .find(|modifier| ast::is_decorator(store, **modifier))
                .copied()
        } else {
            None
        }
    }

    fn check_grammar_async_modifier(&mut self, node: ast::Node, async_modifier: ast::Node) -> bool {
        match self.store_for_node(node).kind(node) {
            ast::Kind::MethodDeclaration
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction => return false,
            _ => {}
        }

        self.grammar_error_on_node(
            async_modifier,
            &diagnostics::X_0_modifier_cannot_be_used_here,
            vec!["async".into()],
        )
    }

    pub(crate) fn check_grammar_for_disallowed_trailing_comma(
        &mut self,
        list: Option<ast::SourceNodeList<'_>>,
        diag: &'static diagnostics::Message,
    ) -> bool {
        if let Some(list) = list {
            let Some(anchor) = list.first().or_else(|| list.last()) else {
                return false;
            };
            let source_file = self.source_file_for_node(anchor);
            let text = source_file.text().as_bytes();
            let mut pos = list.end().max(0) as usize;
            while pos > 0 && pos <= text.len() && text[pos - 1].is_ascii_whitespace() {
                pos -= 1;
            }
            if pos > 0 && pos <= text.len() && text[pos - 1] == b',' {
                return self.grammar_error_at_pos(
                    anchor,
                    (list.end() as usize).saturating_sub(",".len()),
                    ",".len(),
                    diag,
                    vec![],
                );
            }
        }
        false
    }

    fn check_grammar_type_parameter_list(
        &mut self,
        type_parameters: Option<ast::SourceNodeList<'_>>,
        file: &impl ast::SourceFileStoreLike,
    ) -> bool {
        if let Some(type_parameters) = type_parameters {
            if type_parameters.is_empty() {
                let start = (type_parameters.pos() as usize).saturating_sub("<".len());
                let end = scanner::skip_trivia(file.data().text(), type_parameters.end() as usize)
                    + ">".len();
                return self.grammar_error_at_pos(
                    file.as_node(),
                    start,
                    end - start,
                    &diagnostics::Type_parameter_list_cannot_be_empty,
                    vec![],
                );
            }
        }
        false
    }

    fn check_grammar_parameter_list(
        &mut self,
        store: &'a ast::AstStore,
        parameters: ast::SourceNodeList<'a>,
    ) -> bool {
        let mut seen_optional_parameter = false;
        let parameter_count = parameters.len();

        for (i, parameter_node) in parameters.iter().enumerate() {
            if let Some(dot_dot_dot_token) = store.dot_dot_dot_token(parameter_node) {
                if i != parameter_count - 1 {
                    return self.grammar_error_on_node(
                        dot_dot_dot_token,
                        &diagnostics::A_rest_parameter_must_be_last_in_a_parameter_list,
                        vec![],
                    );
                }
                if store.flags(parameter_node) & ast::NodeFlags::Ambient == 0 {
                    self.check_grammar_for_disallowed_trailing_comma(
                        Some(parameters),
                        &diagnostics::A_rest_parameter_or_binding_pattern_may_not_have_a_trailing_comma,
                    );
                }

                if let Some(question_token) = store.question_token(parameter_node) {
                    return self.grammar_error_on_node(
                        question_token,
                        &diagnostics::A_rest_parameter_cannot_be_optional,
                        vec![],
                    );
                }

                if store.initializer(parameter_node).is_some() {
                    return self.grammar_error_on_node(
                        store.name(parameter_node).unwrap(),
                        &diagnostics::A_rest_parameter_cannot_have_an_initializer,
                        vec![],
                    );
                }
            } else if is_optional_declaration(store, parameter_node) {
                seen_optional_parameter = true;
                // A reparsed '?' token indicates a bracketed name in @param tag
                if store
                    .question_token(parameter_node)
                    .is_some_and(|question_token| {
                        store.flags(question_token) & ast::NodeFlags::Reparsed == 0
                    })
                    && store.initializer(parameter_node).is_some()
                {
                    return self.grammar_error_on_node(
                        store.name(parameter_node).unwrap(),
                        &diagnostics::Parameter_cannot_have_question_mark_and_initializer,
                        vec![],
                    );
                }
            } else if seen_optional_parameter && store.initializer(parameter_node).is_none() {
                return self.grammar_error_on_node(
                    store.name(parameter_node).unwrap(),
                    &diagnostics::A_required_parameter_cannot_follow_an_optional_parameter,
                    vec![],
                );
            }
        }

        false
    }

    fn check_grammar_for_use_strict_simple_parameter_list(&mut self, node: ast::Node) -> bool {
        if self.language_version() >= core::ScriptTarget::ES2016 {
            let store = self.store_for_node(node);
            let body = store.body(node);
            let mut use_strict_directive: Option<ast::Node> = None;
            if let Some(body) = body {
                if ast::is_block(store, body) {
                    let source_file = source_file_of_node(store, node);
                    let statements = store.statements(body).unwrap();
                    let statement_refs = statements.iter().collect::<Vec<_>>();
                    use_strict_directive =
                        binder::find_use_strict_prologue(&source_file, &statement_refs);
                }
            }
            if let Some(use_strict_directive) = use_strict_directive {
                let use_strict_directive = use_strict_directive;
                let parameters = store.parameters(node).unwrap().iter().collect::<Vec<_>>();
                let non_simple_parameters = parameters
                    .iter()
                    .filter(|n| {
                        store.initializer(**n).is_some()
                            || store
                                .name(**n)
                                .is_some_and(|name| ast::is_binding_pattern(store, name))
                            || store.dot_dot_dot_token(**n).is_some()
                    })
                    .collect::<Vec<_>>();
                if !non_simple_parameters.is_empty() {
                    for parameter in &non_simple_parameters {
                        let mut err = self.create_error_diagnostic(
                            **parameter,
                            &diagnostics::This_parameter_is_not_allowed_with_use_strict_directive,
                            Vec::<DiagnosticArg>::new(),
                        );
                        err.add_related_info(new_diagnostic_for_node(
                            store,
                            Some(use_strict_directive),
                            &diagnostics::X_use_strict_directive_used_here,
                            Vec::<DiagnosticArg>::new(),
                        ));
                        self.add_error_diagnostic(err);
                    }

                    let mut err = self.create_error_diagnostic(
                        use_strict_directive,
                        &diagnostics::X_use_strict_directive_cannot_be_used_with_non_simple_parameter_list,
                        Vec::<DiagnosticArg>::new(),
                    );
                    for (index, parameter) in non_simple_parameters.iter().enumerate() {
                        let related_message = if index == 0 {
                            &diagnostics::Non_simple_parameter_declared_here
                        } else {
                            &diagnostics::X_and_here
                        };
                        err.add_related_info(new_diagnostic_for_node(
                            store,
                            Some(**parameter),
                            related_message,
                            Vec::<DiagnosticArg>::new(),
                        ));
                    }
                    self.add_error_diagnostic(err);

                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn check_grammar_function_like_declaration(&mut self, node: ast::Node) -> bool {
        // Prevent cascading error by short-circuit
        let store = self.store_for_node(node);
        let file = source_file_of_node(store, node);
        let type_parameters = store.type_parameters(node);
        let parameters = store.parameters(node).unwrap();
        self.check_grammar_modifiers(node)
            || self.check_grammar_type_parameter_list(type_parameters, &file)
            || self.check_grammar_parameter_list(store, parameters)
            || self.check_grammar_arrow_function(node, &file)
            || (ast::is_function_like_declaration(store, Some(node))
                && self.check_grammar_for_use_strict_simple_parameter_list(node))
    }

    pub(crate) fn check_grammar_class_like_declaration(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let file = source_file_of_node(store, node);
        let type_parameter_list = store.type_parameters(node);
        self.check_grammar_class_declaration_heritage_clauses(node, &file)
            || self.check_grammar_type_parameter_list(type_parameter_list, &file)
    }

    fn check_grammar_arrow_function(
        &mut self,
        node: ast::Node,
        file: &impl ast::SourceFileStoreLike,
    ) -> bool {
        let store = self.store_for_node(node);
        if !ast::is_arrow_function(store, node) {
            return false;
        }

        let type_parameters = store.type_parameters(node);
        if let Some(type_parameters) = type_parameters {
            let first_type_parameter = type_parameters.first();
            let has_constraint = first_type_parameter
                .is_some_and(|type_parameter| store.constraint(type_parameter).is_some());
            if !(type_parameters.len() > 1 || has_constraint) {
                if tspath::file_extension_is_one_of(
                    file.data().file_name_ref(),
                    &[tspath::Extension::Mts, tspath::Extension::Cts],
                ) {
                    // TODO(danielr): should we return early here?
                    self.grammar_error_on_node(
                        first_type_parameter.unwrap(),
                        &diagnostics::This_syntax_is_reserved_in_files_with_the_mts_or_cts_extension_Add_a_trailing_comma_or_explicit_constraint,
                        vec![],
                    );
                }
            }
        }

        let equals_greater_than_token = store.equals_greater_than_token(node).unwrap();
        let start_line = scanner::get_ecma_line_of_position(
            file.data(),
            store.loc(equals_greater_than_token).pos(),
        );
        let end_line = scanner::get_ecma_line_of_position(
            file.data(),
            store.loc(equals_greater_than_token).end(),
        );
        start_line != end_line
            && self.grammar_error_on_node(
                equals_greater_than_token,
                &diagnostics::Line_terminator_not_permitted_before_arrow,
                vec![],
            )
    }

    fn check_grammar_index_signature_parameters(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let parameters = store.parameters(node).unwrap();

        if parameters.is_empty() {
            return self.grammar_error_on_node(
                node,
                &diagnostics::An_index_signature_must_have_exactly_one_parameter,
                vec![],
            );
        }

        let parameter_node = parameters.first().unwrap();
        if parameters.len() != 1 {
            return self.grammar_error_on_node(
                store.name(parameter_node).unwrap(),
                &diagnostics::An_index_signature_must_have_exactly_one_parameter,
                vec![],
            );
        }

        self.check_grammar_for_disallowed_trailing_comma(
            Some(parameters),
            &diagnostics::An_index_signature_cannot_have_a_trailing_comma,
        );
        if let Some(dot_dot_dot_token) = store.dot_dot_dot_token(parameter_node) {
            return self.grammar_error_on_node(
                dot_dot_dot_token,
                &diagnostics::An_index_signature_cannot_have_a_rest_parameter,
                vec![],
            );
        }
        if store.modifiers(parameter_node).is_some() {
            return self.grammar_error_on_node(
                store.name(parameter_node).unwrap(),
                &diagnostics::An_index_signature_parameter_cannot_have_an_accessibility_modifier,
                vec![],
            );
        }
        if let Some(question_token) = store.question_token(parameter_node) {
            return self.grammar_error_on_node(
                question_token,
                &diagnostics::An_index_signature_parameter_cannot_have_a_question_mark,
                vec![],
            );
        }
        if store.initializer(parameter_node).is_some() {
            return self.grammar_error_on_node(
                store.name(parameter_node).unwrap(),
                &diagnostics::An_index_signature_parameter_cannot_have_an_initializer,
                vec![],
            );
        }
        let type_node = store.type_node(parameter_node);
        let Some(type_node) = type_node else {
            return self.grammar_error_on_node(
                store.name(parameter_node).unwrap(),
                &diagnostics::An_index_signature_parameter_must_have_a_type_annotation,
                vec![],
            );
        };
        let t = self.get_type_from_type_node(type_node);
        if some_type(self, t, |checker, t| {
            checker.type_flags(t) & TYPE_FLAGS_STRING_OR_NUMBER_LITERAL_OR_UNIQUE != 0
        }) || self.is_generic_type(t)
        {
            return self.grammar_error_on_node(
                store.name(parameter_node).unwrap(),
                &diagnostics::An_index_signature_parameter_type_cannot_be_a_literal_type_or_generic_type_Consider_using_a_mapped_object_type_instead,
                vec![],
            );
        }
        if !every_type(self, t, |checker, t| checker.is_valid_index_key_type(t)) {
            return self.grammar_error_on_node(
                store.name(parameter_node).unwrap(),
                &diagnostics::An_index_signature_parameter_type_must_be_string_number_symbol_or_a_template_literal_type,
                vec![],
            );
        }
        if store.type_node(node).is_none() {
            return self.grammar_error_on_node(
                node,
                &diagnostics::An_index_signature_must_have_a_type_annotation,
                vec![],
            );
        }
        false
    }

    pub(crate) fn check_grammar_index_signature(&mut self, node: ast::Node) -> bool {
        // Prevent cascading error by short-circuit
        self.check_grammar_modifiers(node) || self.check_grammar_index_signature_parameters(node)
    }

    fn check_grammar_for_at_least_one_type_argument(
        &mut self,
        node: ast::Node,
        type_arguments: Option<ast::SourceNodeList<'a>>,
    ) -> bool {
        if let Some(type_arguments) = type_arguments {
            if type_arguments.is_empty() {
                let source_file_node =
                    ast::get_source_file_of_node(self.store_for_node(node), Some(node)).unwrap();
                let source_file = self.source_file_for_node(node);
                let start = (type_arguments.pos() as usize).saturating_sub("<".len());
                let end = scanner::skip_trivia(source_file.text(), type_arguments.end() as usize)
                    + ">".len();
                return self.grammar_error_at_pos(
                    source_file_node,
                    start,
                    end - start,
                    &diagnostics::Type_argument_list_cannot_be_empty,
                    vec![],
                );
            }
        }
        false
    }

    fn check_grammar_for_disallowed_trailing_comma_in_type_arguments(
        &mut self,
        node: ast::Node,
        type_arguments: Option<ast::SourceNodeList<'a>>,
    ) -> bool {
        let Some(type_arguments) = type_arguments else {
            return false;
        };
        if type_arguments.is_empty() {
            return false;
        }
        let source_file = self.source_file_for_node(node);
        let text = source_file.text().as_bytes();
        let mut pos = type_arguments.end().max(0) as usize;
        while pos > 0 && text.get(pos - 1).is_some_and(|b| b.is_ascii_whitespace()) {
            pos -= 1;
        }
        if text.get(pos.saturating_sub(1)) == Some(&b',') {
            return self.grammar_error_at_pos(
                node,
                pos.saturating_sub(1),
                ",".len(),
                &diagnostics::Trailing_comma_not_allowed,
                vec![],
            );
        }
        false
    }

    pub(crate) fn check_grammar_type_arguments(
        &mut self,
        node: ast::Node,
        type_arguments: Option<ast::SourceNodeList<'a>>,
    ) -> bool {
        self.check_grammar_for_disallowed_trailing_comma_in_type_arguments(node, type_arguments)
            || self.check_grammar_for_at_least_one_type_argument(node, type_arguments)
    }

    pub(crate) fn check_grammar_tagged_template_chain(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if store.question_dot_token(node).is_some()
            || store.flags(node) & ast::NodeFlags::OptionalChain != 0
        {
            return self.grammar_error_on_node(
                store.template(node).unwrap(),
                &diagnostics::Tagged_template_expressions_are_not_permitted_in_an_optional_chain,
                vec![],
            );
        }
        false
    }

    fn check_grammar_heritage_clause(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let types = store.types(node);
        if self.check_grammar_for_disallowed_trailing_comma(
            types,
            &diagnostics::Trailing_comma_not_allowed,
        ) {
            return true;
        }
        if let Some(types) = types {
            if types.is_empty() {
                let list_type = scanner::token_to_string(store.token(node).unwrap());
                // TODO(danielr): why not error on the token?
                return self.grammar_error_at_pos(
                    node,
                    types.pos() as usize,
                    0,
                    &diagnostics::X_0_list_cannot_be_empty,
                    vec![list_type.into()],
                );
            }

            for type_node in types.iter() {
                if self.check_grammar_expression_with_type_arguments(type_node) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn check_grammar_expression_with_type_arguments(
        &mut self,
        node: ast::Node, /*Union[ExpressionWithTypeArguments, TypeQuery]*/
    ) -> bool {
        let store = self.store_for_node(node);
        if ast::is_expression_with_type_arguments(store, node)
            && store
                .expression(node)
                .is_some_and(|expression| store.kind(expression) == ast::Kind::ImportKeyword)
            && store.type_arguments(node).is_some()
        {
            return self.grammar_error_on_node(
                node,
                &diagnostics::This_use_of_import_is_invalid_import_calls_can_be_written_but_they_must_have_parentheses_and_cannot_have_type_arguments,
                vec![],
            );
        }
        let type_argument_list = store.type_arguments(node);
        self.check_grammar_type_arguments(node, type_argument_list)
    }

    fn check_grammar_class_declaration_heritage_clauses(
        &mut self,
        node: ast::Node,
        file: &impl ast::SourceFileStoreLike,
    ) -> bool {
        let mut seen_extends_clause = false;
        let mut seen_implements_clause = false;

        let store = self.store_for_node(node);
        if !self.check_grammar_modifiers(node) && store.heritage_clauses(node).is_some() {
            let heritage_clauses = store.heritage_clauses(node).unwrap();
            for heritage_clause_node in heritage_clauses.iter() {
                let heritage_clause_store = self.store_for_node(heritage_clause_node);
                let heritage_token = heritage_clause_store.token(heritage_clause_node).unwrap();
                if heritage_token == ast::Kind::ExtendsKeyword {
                    if seen_extends_clause {
                        return self.grammar_error_on_first_token(
                            heritage_clause_node,
                            &diagnostics::X_extends_clause_already_seen,
                            vec![],
                        );
                    }

                    if seen_implements_clause {
                        return self.grammar_error_on_first_token(
                            heritage_clause_node,
                            &diagnostics::X_extends_clause_must_precede_implements_clause,
                            vec![],
                        );
                    }

                    let type_nodes = heritage_clause_store.types(heritage_clause_node).unwrap();
                    if type_nodes.len() > 1 {
                        return self.grammar_error_on_first_token(
                            type_nodes.iter().nth(1).unwrap(),
                            &diagnostics::Classes_can_only_extend_a_single_class,
                            vec![],
                        );
                    }
                    seen_extends_clause = true;
                } else {
                    if heritage_token != ast::Kind::ImplementsKeyword {
                        panic!("Unexpected token {:?}", heritage_token);
                    }
                    if seen_implements_clause {
                        return self.grammar_error_on_first_token(
                            heritage_clause_node,
                            &diagnostics::X_implements_clause_already_seen,
                            vec![],
                        );
                    }

                    seen_implements_clause = true;
                }

                // Grammar checking heritageClause inside class declaration
                self.check_grammar_heritage_clause(heritage_clause_node);
            }
        }

        false
    }

    pub(crate) fn check_grammar_interface_declaration(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if let Some(heritage_clauses) = store.heritage_clauses(node) {
            let mut seen_extends_clause = false;
            for heritage_clause_node in heritage_clauses.iter() {
                let heritage_clause_store = self.store_for_node(heritage_clause_node);
                let heritage_token = heritage_clause_store.token(heritage_clause_node).unwrap();

                match heritage_token {
                    ast::Kind::ExtendsKeyword => {
                        if seen_extends_clause {
                            return self.grammar_error_on_first_token(
                                heritage_clause_node,
                                &diagnostics::X_extends_clause_already_seen,
                                vec![],
                            );
                        }
                        seen_extends_clause = true;
                    }
                    ast::Kind::ImplementsKeyword => {
                        return self.grammar_error_on_first_token(
                            heritage_clause_node,
                            &diagnostics::Interface_declaration_cannot_have_implements_clause,
                            vec![],
                        );
                    }
                    _ => panic!("Unexpected token {:?}", heritage_token),
                }

                // Grammar checking heritageClause inside class declaration
                self.check_grammar_heritage_clause(heritage_clause_node);
            }
        }

        false
    }

    pub(crate) fn check_grammar_computed_property_name(&mut self, node: ast::Node) -> bool {
        // If node is not a computedPropertyName, just skip the grammar checking
        let store = self.store_for_node(node);
        if store.kind(node) != ast::Kind::ComputedPropertyName {
            return false;
        }

        let expression = store.expression(node).unwrap();
        let expression_store = self.store_for_node(expression);
        if expression_store.kind(expression) == ast::Kind::BinaryExpression
            && expression_store
                .operator_token(expression)
                .is_some_and(|operator| expression_store.kind(operator) == ast::Kind::CommaToken)
        {
            return self.grammar_error_on_node(
                expression,
                &diagnostics::A_comma_expression_is_not_allowed_in_a_computed_property_name,
                vec![],
            );
        }
        false
    }

    pub(crate) fn check_grammar_for_generator(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if let Some(asterisk_token) = store.asterisk_token(node) {
            if store.has_body_base(node) {
                if store.kind(node) != ast::Kind::FunctionDeclaration
                    && store.kind(node) != ast::Kind::FunctionExpression
                    && store.kind(node) != ast::Kind::MethodDeclaration
                {
                    panic!("Unexpected node kind {:?}", store.kind(node));
                }
                if store.flags(node) & ast::NodeFlags::Ambient != 0 {
                    return self.grammar_error_on_node(
                        asterisk_token,
                        &diagnostics::Generators_are_not_allowed_in_an_ambient_context,
                        vec![],
                    );
                }
                if store.body(node).is_none() {
                    return self.grammar_error_on_node(
                        asterisk_token,
                        &diagnostics::An_overload_signature_cannot_be_declared_as_a_generator,
                        vec![],
                    );
                }
            }
        }

        false
    }

    fn check_grammar_for_invalid_question_mark(
        &mut self,
        postfix_token: Option<ast::Node>,
        message: &'static diagnostics::Message,
    ) -> bool {
        postfix_token.is_some_and(|postfix_token| {
            self.store_for_node(postfix_token).kind(postfix_token) == ast::Kind::QuestionToken
                && self.grammar_error_on_node(postfix_token, message, vec![])
        })
    }

    fn check_grammar_for_invalid_exclamation_token(
        &mut self,
        postfix_token: Option<ast::Node>,
        message: &'static diagnostics::Message,
    ) -> bool {
        postfix_token.is_some_and(|postfix_token| {
            self.store_for_node(postfix_token).kind(postfix_token) == ast::Kind::ExclamationToken
                && self.grammar_error_on_node(postfix_token, message, vec![])
        })
    }

    pub(crate) fn check_grammar_object_literal_expression(
        &mut self,
        node: ast::Node,
        in_destructuring: bool,
    ) -> bool {
        let mut seen: HashMap<String, DeclarationMeaning> = HashMap::new();

        let store = self.store_for_node(node);
        let properties = store.properties(node);
        let Some(properties) = properties else {
            return false;
        };
        for prop in properties.iter() {
            let prop = prop;
            let prop_store = self.store_for_node(prop);
            if prop_store.kind(prop) == ast::Kind::SpreadAssignment {
                if in_destructuring {
                    // a rest property cannot be destructured any further
                    let spread_expression = prop_store.expression(prop).unwrap();
                    let expression = ast::skip_parentheses(
                        self.store_for_node(spread_expression),
                        spread_expression,
                    );
                    if ast::is_array_literal_expression(self.store_for_node(expression), expression)
                        || ast::is_object_literal_expression(
                            self.store_for_node(expression),
                            expression,
                        )
                    {
                        return self.grammar_error_on_node(
                            spread_expression,
                            &diagnostics::A_rest_element_cannot_contain_a_binding_pattern,
                            vec![],
                        );
                    }
                }
                continue;
            }
            let name = prop_store.name(prop).unwrap();
            if self.store_for_node(name).kind(name) == ast::Kind::ComputedPropertyName {
                // If the name is not a ComputedPropertyName, the grammar checking will skip it
                self.check_grammar_computed_property_name(name);
            }

            if prop_store.kind(prop) == ast::Kind::ShorthandPropertyAssignment && !in_destructuring
            {
                let object_assignment_initializer = prop_store.object_assignment_initializer(prop);
                if object_assignment_initializer.is_some() {
                    // having objectAssignmentInitializer is only valid in an ObjectAssignmentPattern.
                    // Outside of destructuring, it is a syntax error.

                    // Try to grab the last node prior to the initializer,
                    // then error on the first token following (which should be the `=` token).
                    let mut last_node_before_initializer: Option<ast::Node> = None;
                    let _ = prop_store.for_each_present_child(prop, |child| {
                        if Some(child) != object_assignment_initializer {
                            last_node_before_initializer = Some(child);
                            return std::ops::ControlFlow::Continue(());
                        }
                        std::ops::ControlFlow::Break(())
                    });

                    self.grammar_error_on_first_token(
                        last_node_before_initializer.unwrap(),
                        &diagnostics::Did_you_mean_to_use_a_Colon_An_can_only_follow_a_property_name_when_the_containing_object_literal_is_part_of_a_destructuring_pattern,
                        vec![],
                    );
                }
            }

            if self.store_for_node(name).kind(name) == ast::Kind::PrivateIdentifier {
                self.grammar_error_on_node(
                    name,
                    &diagnostics::Private_identifiers_are_not_allowed_outside_class_bodies,
                    vec![],
                );
            }

            // Modifiers are never allowed on properties except for 'async' on a method declaration
            let modifiers = prop_store.modifier_nodes(prop);
            if !modifiers.is_empty() {
                if ast::can_have_modifiers(prop_store, prop) {
                    for mod_ in &modifiers {
                        if ast::is_modifier(prop_store, *mod_)
                            && (prop_store.kind(*mod_) != ast::Kind::AsyncKeyword
                                || prop_store.kind(prop) != ast::Kind::MethodDeclaration)
                        {
                            self.grammar_error_on_node(
                                *mod_,
                                &diagnostics::X_0_modifier_cannot_be_used_here,
                                vec![
                                    scanner::get_text_of_node(
                                        self.source_file_for_node(*mod_),
                                        &*mod_,
                                    )
                                    .into(),
                                ],
                            );
                        }
                    }
                } else if ast::can_have_illegal_modifiers(self.store_for_node(prop), prop) {
                    for mod_ in &modifiers {
                        if ast::is_modifier(prop_store, *mod_) {
                            self.grammar_error_on_node(
                                *mod_,
                                &diagnostics::X_0_modifier_cannot_be_used_here,
                                vec![
                                    scanner::get_text_of_node(
                                        self.source_file_for_node(*mod_),
                                        &*mod_,
                                    )
                                    .into(),
                                ],
                            );
                        }
                    }
                }
            }

            // ECMA-262 11.1.5 Object Initializer
            // If previous is not undefined then throw a SyntaxError exception if any of the following conditions are true
            // a.This production is contained in strict code and IsDataDescriptor(previous) is true and
            // IsDataDescriptor(propId.descriptor) is true.
            //    b.IsDataDescriptor(previous) is true and IsAccessorDescriptor(propId.descriptor) is true.
            //    c.IsAccessorDescriptor(previous) is true and IsDataDescriptor(propId.descriptor) is true.
            //    d.IsAccessorDescriptor(previous) is true and IsAccessorDescriptor(propId.descriptor) is true
            // and either both previous and propId.descriptor have[[Get]] fields or both previous and propId.descriptor have[[Set]] fields
            let current_kind = match prop_store.kind(prop) {
                ast::Kind::ShorthandPropertyAssignment | ast::Kind::PropertyAssignment => {
                    let postfix_token = prop_store.postfix_token(prop);

                    // Grammar checking for computedPropertyName and shorthandPropertyAssignment
                    self.check_grammar_for_invalid_exclamation_token(
                        postfix_token,
                        &diagnostics::A_definite_assignment_assertion_is_not_permitted_in_this_context,
                    );
                    self.check_grammar_for_invalid_question_mark(
                        postfix_token,
                        &diagnostics::An_object_member_cannot_be_declared_optional,
                    );

                    if self.store_for_node(name).kind(name) == ast::Kind::NumericLiteral {
                        self.check_grammar_numeric_literal(name);
                    }

                    if self.store_for_node(name).kind(name) == ast::Kind::BigIntLiteral {
                        self.add_error_or_suggestion(
                            true,
                            create_diagnostic_for_node(
                                self.store_for_node(name),
                                name,
                                &diagnostics::A_bigint_literal_cannot_be_used_as_a_property_name,
                            ),
                        );
                    }

                    DECLARATION_MEANING_PROPERTY_ASSIGNMENT
                }
                ast::Kind::MethodDeclaration => DECLARATION_MEANING_METHOD,
                ast::Kind::GetAccessor => DECLARATION_MEANING_GET_ACCESSOR,
                ast::Kind::SetAccessor => DECLARATION_MEANING_SET_ACCESSOR,
                _ => panic!("Unexpected node kind {:?}", prop_store.kind(prop)),
            };

            if !in_destructuring {
                let effective_name = self.get_effective_property_name_for_property_name_node(name);
                let Some(effective_name) = effective_name else {
                    continue;
                };

                let existing_kind = *seen.get(&effective_name).unwrap_or(&0);
                if existing_kind == 0 {
                    seen.insert(effective_name, current_kind);
                } else if current_kind & DECLARATION_MEANING_METHOD != 0
                    && existing_kind & DECLARATION_MEANING_METHOD != 0
                {
                    self.grammar_error_on_node(
                        name,
                        &diagnostics::Duplicate_identifier_0,
                        vec![
                            scanner::get_text_of_node(self.source_file_for_node(name), &name)
                                .into(),
                        ],
                    );
                } else if current_kind & DECLARATION_MEANING_PROPERTY_ASSIGNMENT != 0
                    && existing_kind & DECLARATION_MEANING_PROPERTY_ASSIGNMENT != 0
                {
                    self.grammar_error_on_node(
                        name,
                        &diagnostics::An_object_literal_cannot_have_multiple_properties_with_the_same_name,
                        vec![scanner::get_text_of_node(self.source_file_for_node(name), &name)
                            .into()],
                    );
                } else if current_kind & DECLARATION_MEANING_GET_OR_SET_ACCESSOR != 0
                    && existing_kind & DECLARATION_MEANING_GET_OR_SET_ACCESSOR != 0
                {
                    if existing_kind != DECLARATION_MEANING_GET_OR_SET_ACCESSOR
                        && current_kind != existing_kind
                    {
                        seen.insert(effective_name, current_kind | existing_kind);
                    } else {
                        return self.grammar_error_on_node(name, &diagnostics::An_object_literal_cannot_have_multiple_get_Slashset_accessors_with_the_same_name, vec![]);
                    }
                } else {
                    return self.grammar_error_on_node(name, &diagnostics::An_object_literal_cannot_have_property_and_accessor_with_the_same_name, vec![]);
                }
            }
        }

        false
    }

    pub(crate) fn check_grammar_jsx_element(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        self.check_grammar_jsx_name(store.tag_name(node).unwrap());
        let type_argument_list = store.type_arguments(node);
        self.check_grammar_type_arguments(node, type_argument_list);
        let mut seen: collections::Set<String> = collections::Set::new();
        let attributes = match store.kind(node) {
            ast::Kind::JsxOpeningElement => store.attributes(node),
            ast::Kind::JsxSelfClosingElement => store.attributes(node),
            _ => None,
        }
        .unwrap();
        let attributes = store.properties(attributes).unwrap();
        for attr_node in attributes.iter() {
            let attr_store = self.store_for_node(attr_node);
            if attr_store.kind(attr_node) == ast::Kind::JsxSpreadAttribute {
                continue;
            }
            let name = attr_store.name(attr_node).unwrap();
            let initializer = attr_store.initializer(attr_node);
            let text_of_name = self.node_text(name);
            if !seen.has(&text_of_name) {
                seen.add(text_of_name);
            } else {
                return self.grammar_error_on_node(
                    name,
                    &diagnostics::JSX_elements_cannot_have_multiple_attributes_with_the_same_name,
                    vec![],
                );
            }
            if initializer.is_some_and(|initializer| {
                attr_store.kind(initializer) == ast::Kind::JsxExpression
                    && self
                        .store_for_node(initializer)
                        .expression(initializer)
                        .is_none()
            }) {
                return self.grammar_error_on_node(
                    initializer.unwrap(),
                    &diagnostics::JSX_attributes_must_only_be_assigned_a_non_empty_expression,
                    vec![],
                );
            }
        }
        false
    }

    pub(crate) fn check_grammar_jsx_name(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if ast::is_property_access_expression(store, node)
            && store
                .expression(node)
                .is_some_and(|expression| ast::is_jsx_namespaced_name(store, expression))
        {
            let expression = store.expression(node).unwrap();
            return self.grammar_error_on_node(
                expression,
                &diagnostics::JSX_property_access_expressions_cannot_include_JSX_namespace_names,
                vec![],
            );
        }

        if ast::is_jsx_namespaced_name(store, node)
            && self.compiler_options.get_jsx_transform_enabled()
            && !scanner::is_intrinsic_jsx_name(&self.node_text(store.namespace(node).unwrap()))
        {
            return self.grammar_error_on_node(
                node,
                &diagnostics::React_components_cannot_include_JSX_namespace_names,
                vec![],
            );
        }

        false
    }

    pub(crate) fn check_grammar_jsx_expression(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if store.expression(node).is_some()
            && ast::is_comma_sequence(store, &store.expression(node).unwrap())
        {
            let expression = store.expression(node).unwrap();
            return self.grammar_error_on_node(
                expression,
                &diagnostics::JSX_expressions_may_not_use_the_comma_operator_Did_you_mean_to_write_an_array,
                vec![],
            );
        }

        false
    }

    pub(crate) fn check_grammar_for_in_or_for_of_statement(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if self.check_grammar_statement_in_ambient_context(node) {
            return true;
        }

        if store.kind(node) == ast::Kind::ForOfStatement && store.await_modifier(node).is_some() {
            let await_modifier = store.await_modifier(node).unwrap();
            if store.flags(node) & ast::NodeFlags::AwaitContext == 0 {
                let source_file = self.source_file_for_node(node);
                if ast::is_in_top_level_context(store, node) {
                    if !self.has_parse_diagnostics(&source_file) {
                        if !ast::is_effective_external_module(&source_file, &self.compiler_options)
                        {
                            self.diagnostics().add(create_diagnostic_for_node(
                                    self.store_for_node(await_modifier),
                                    await_modifier,
	                                &diagnostics::X_for_await_loops_are_only_allowed_at_the_top_level_of_a_file_when_that_file_is_a_module_but_this_file_has_no_imports_or_exports_Consider_adding_an_empty_export_to_make_this_file_a_module,
	                            ));
                        }
                        match self.module_kind() {
                            core::ModuleKind::Node16
                            | core::ModuleKind::Node18
                            | core::ModuleKind::Node20
                            | core::ModuleKind::NodeNext => {
                                let source_file_meta_data =
                                    self.program.get_source_file_meta_data(source_file.path());
                                if source_file_meta_data.implied_node_format
                                    == core::ModuleKind::CommonJS
                                {
                                    self.diagnostics().add(create_diagnostic_for_node(
                                            self.store_for_node(await_modifier),
	                                        await_modifier,
	                                        &diagnostics::The_current_file_is_a_CommonJS_module_and_cannot_use_await_at_the_top_level,
	                                    ));
                                } else if self.language_version() < core::ScriptTarget::ES2017 {
                                    self.diagnostics().add(create_diagnostic_for_node(
                                            self.store_for_node(await_modifier),
	                                        await_modifier,
	                                        &diagnostics::Top_level_for_await_loops_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher,
	                                    ));
                                }
                            }
                            core::ModuleKind::ES2022
                            | core::ModuleKind::ESNext
                            | core::ModuleKind::Preserve
                            | core::ModuleKind::System => {
                                if self.language_version() < core::ScriptTarget::ES2017 {
                                    self.diagnostics().add(create_diagnostic_for_node(
                                            self.store_for_node(await_modifier),
	                                        await_modifier,
	                                        &diagnostics::Top_level_for_await_loops_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher,
	                                    ));
                                }
                            }
                            _ => {
                                self.diagnostics().add(create_diagnostic_for_node(
                                        self.store_for_node(await_modifier),
	                                    await_modifier,
	                                    &diagnostics::Top_level_for_await_loops_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher,
	                                ));
                            }
                        }
                    }
                } else {
                    // use of 'for-await-of' in non-async function
                    if !self.has_parse_diagnostics(&source_file) {
                        let mut diagnostic = create_diagnostic_for_node(
                                self.store_for_node(await_modifier),
	                            await_modifier,
	                            &diagnostics::X_for_await_loops_are_only_allowed_within_async_functions_and_at_the_top_levels_of_modules,
	                        );
                        let containing_func = ast::get_containing_function(store, node);
                        if let Some(containing_func) = containing_func {
                            if store.kind(containing_func) != ast::Kind::Constructor {
                                debug::assert(
                                    ast::get_function_flags(store, Some(containing_func))
                                        & ast::FUNCTION_FLAGS_ASYNC
                                        == 0,
                                    Some(
                                        "Enclosing function should never be an async function."
                                            .to_string(),
                                    ),
                                );
                                if has_async_modifier(store, containing_func) {
                                    panic!("Enclosing function should never be an async function.");
                                }
                                let related_info = create_diagnostic_for_node(
                                    store,
                                    containing_func,
                                    &diagnostics::Did_you_mean_to_mark_this_function_as_async,
                                );
                                diagnostic.add_related_info(related_info);
                            }
                        }
                        self.diagnostics().add(diagnostic);
                        return true;
                    }
                }
            }
        }

        if ast::is_for_of_statement(store, node)
            && store.flags(node) & ast::NodeFlags::AwaitContext == 0
            && store
                .initializer(node)
                .is_some_and(|initializer| ast::is_identifier(store, initializer))
            && self.node_text(store.initializer(node).unwrap()) == "async"
        {
            self.grammar_error_on_node(
                store.initializer(node).unwrap(),
                &diagnostics::The_left_hand_side_of_a_for_of_statement_may_not_be_async,
                vec![],
            );
            return false;
        }

        let initializer = store.initializer(node).unwrap();
        if store.kind(initializer) == ast::Kind::VariableDeclarationList {
            let variable_list_node = initializer;
            let variable_list_store = self.store_for_node(variable_list_node);
            if !self.check_grammar_variable_declaration_list(variable_list_node) {
                let declarations = variable_list_store
                    .declarations(variable_list_node)
                    .unwrap();

                // declarations.length can be zero if there is an error in variable declaration in for-of or for-in
                // See http://www.ecma-international.org/ecma-262/6.0/#sec-for-in-and-for-of-statements for details
                // For example:
                //      var let = 10;
                //      for (let of [1,2,3]) {} // this is invalid ES6 syntax
                //      for (let in [1,2,3]) {} // this is invalid ES6 syntax
                // We will then want to skip on grammar checking on variableList declaration
                if declarations.is_empty() {
                    return false;
                }

                if declarations.len() > 1 {
                    let diagnostic = if store.kind(node) == ast::Kind::ForInStatement {
                        &diagnostics::Only_a_single_variable_declaration_is_allowed_in_a_for_in_statement
                    } else {
                        &diagnostics::Only_a_single_variable_declaration_is_allowed_in_a_for_of_statement
                    };
                    return self.grammar_error_on_first_token(
                        declarations.iter().nth(1).unwrap(),
                        diagnostic,
                        vec![],
                    );
                }

                let first_variable_declaration = declarations.first().unwrap();
                let first_store = self.store_for_node(first_variable_declaration);
                if first_store
                    .initializer(first_variable_declaration)
                    .is_some()
                {
                    let diagnostic = if store.kind(node) == ast::Kind::ForInStatement {
                        &diagnostics::The_variable_declaration_of_a_for_in_statement_cannot_have_an_initializer
                    } else {
                        &diagnostics::The_variable_declaration_of_a_for_of_statement_cannot_have_an_initializer
                    };
                    return self.grammar_error_on_node(
                        first_store.name(first_variable_declaration).unwrap(),
                        diagnostic,
                        vec![],
                    );
                }
                if first_store.type_node(first_variable_declaration).is_some() {
                    let diagnostic = if store.kind(node) == ast::Kind::ForInStatement {
                        &diagnostics::The_left_hand_side_of_a_for_in_statement_cannot_use_a_type_annotation
                    } else {
                        &diagnostics::The_left_hand_side_of_a_for_of_statement_cannot_use_a_type_annotation
                    };
                    return self.grammar_error_on_node(
                        first_variable_declaration,
                        diagnostic,
                        vec![],
                    );
                }
            }
        }

        false
    }

    pub(crate) fn check_grammar_accessor(&mut self, accessor: ast::Node) -> bool {
        let store = self.store_for_node(accessor);
        let body = store.body(accessor);
        let parent = store.parent(accessor).unwrap();
        if store.flags(accessor) & ast::NodeFlags::Ambient == 0
            && store.kind(parent) != ast::Kind::TypeLiteral
            && store.kind(parent) != ast::Kind::InterfaceDeclaration
        {
            if body.is_none()
                && !ast::has_syntactic_modifier(store, accessor, ast::ModifierFlags::Abstract)
            {
                return self.grammar_error_at_pos(
                    accessor,
                    (store.loc(accessor).end() as usize).saturating_sub(1),
                    ";".len(),
                    &diagnostics::X_0_expected,
                    vec!["{".into()],
                );
            }
        }
        if let Some(body) = body {
            if ast::has_syntactic_modifier(store, accessor, ast::ModifierFlags::Abstract) {
                return self.grammar_error_on_node(
                    accessor,
                    &diagnostics::An_abstract_accessor_cannot_have_an_implementation,
                    vec![],
                );
            }
            if store.kind(parent) == ast::Kind::TypeLiteral
                || store.kind(parent) == ast::Kind::InterfaceDeclaration
            {
                return self.grammar_error_on_node(
                    body,
                    &diagnostics::An_implementation_cannot_be_declared_in_ambient_contexts,
                    vec![],
                );
            }
        }

        if store.type_parameters(accessor).is_some() {
            return self.grammar_error_on_node(
                store.name(accessor).unwrap(),
                &diagnostics::An_accessor_cannot_have_type_parameters,
                vec![],
            );
        }
        if !self.does_accessor_have_correct_parameter_count(accessor) {
            return self.grammar_error_on_node(
                store.name(accessor).unwrap(),
                core::if_else(
                    store.kind(accessor) == ast::Kind::GetAccessor,
                    &diagnostics::A_get_accessor_cannot_have_parameters,
                    &diagnostics::A_set_accessor_must_have_exactly_one_parameter,
                ),
                vec![],
            );
        }
        if store.kind(accessor) == ast::Kind::SetAccessor {
            if store.type_node(accessor).is_some() {
                return self.grammar_error_on_node(
                    store.name(accessor).unwrap(),
                    &diagnostics::A_set_accessor_cannot_have_a_return_type_annotation,
                    vec![],
                );
            }

            let parameter_node = get_set_accessor_value_parameter(store, accessor);
            if parameter_node.is_none() {
                panic!("Return value does not match parameter count assertion.");
            }
            let parameter_node = parameter_node.unwrap();
            let parameter_store = self.store_for_node(parameter_node);
            if let Some(dot_dot_dot_token) = parameter_store.dot_dot_dot_token(parameter_node) {
                return self.grammar_error_on_node(
                    dot_dot_dot_token,
                    &diagnostics::A_set_accessor_cannot_have_rest_parameter,
                    vec![],
                );
            }
            if let Some(question_token) = parameter_store.question_token(parameter_node) {
                return self.grammar_error_on_node(
                    question_token,
                    &diagnostics::A_set_accessor_cannot_have_an_optional_parameter,
                    vec![],
                );
            }
            if parameter_store.initializer(parameter_node).is_some() {
                return self.grammar_error_on_node(
                    store.name(accessor).unwrap(),
                    &diagnostics::A_set_accessor_parameter_cannot_have_an_initializer,
                    vec![],
                );
            }
        }

        false
    }

    // Does the accessor have the right number of parameters?
    //
    //	A `get` accessor has no parameters or a single `this` parameter.
    //	A `set` accessor has one parameter or a `this` parameter and one more parameter.
    fn does_accessor_have_correct_parameter_count(&mut self, accessor: ast::Node) -> bool {
        // `getAccessorThisParameter` returns `nil` if the accessor's arity is incorrect,
        // even if there is a `this` parameter declared.
        self.get_accessor_this_parameter(accessor).is_some()
            || self
                .store_for_node(accessor)
                .parameters(accessor)
                .is_some_and(|parameters| {
                    parameters.len()
                        == core::if_else(
                            self.store_for_node(accessor).kind(accessor) == ast::Kind::GetAccessor,
                            0,
                            1,
                        )
                })
    }

    pub(crate) fn check_grammar_type_operator_node(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let operator = store.operator(node).unwrap();
        if operator == ast::Kind::UniqueKeyword {
            let inner_type = store.type_node(node).unwrap();
            if store.kind(inner_type) != ast::Kind::SymbolKeyword {
                return self.grammar_error_on_node(
                    inner_type,
                    &diagnostics::X_0_expected,
                    vec![scanner::token_to_string(ast::Kind::SymbolKeyword).into()],
                );
            }
            let parent_node = store.parent(node);
            let parent = ast::walk_up_parenthesized_types(store, parent_node).unwrap();
            let parent_store = self.store_for_node(parent);
            match parent_store.kind(parent) {
                ast::Kind::VariableDeclaration => {
                    let name = parent_store.name(parent).unwrap();
                    if parent_store.kind(name) != ast::Kind::Identifier {
                        return self.grammar_error_on_node(node, &diagnostics::X_unique_symbol_types_may_not_be_used_on_a_variable_declaration_with_a_binding_name, vec![]);
                    }
                    if !is_variable_declaration_in_variable_statement(parent_store, parent) {
                        return self.grammar_error_on_node(node, &diagnostics::X_unique_symbol_types_are_only_allowed_on_variables_in_a_variable_statement, vec![]);
                    }
                    if parent_store.flags(parent_store.parent(parent).unwrap())
                        & ast::NodeFlags::Const
                        == 0
                    {
                        return self.grammar_error_on_node(name, &diagnostics::A_variable_whose_type_is_a_unique_symbol_type_must_be_const, vec![]);
                    }
                }
                ast::Kind::PropertyDeclaration => {
                    if !ast::is_static(parent_store, parent)
                        || !has_readonly_modifier(parent_store, parent)
                    {
                        return self.grammar_error_on_node(
                            parent_store.name(parent).unwrap(),
                            &diagnostics::A_property_of_a_class_whose_type_is_a_unique_symbol_type_must_be_both_static_and_readonly,
                            vec![],
                        );
                    }
                }
                ast::Kind::PropertySignature => {
                    if !ast::has_syntactic_modifier(
                        parent_store,
                        parent,
                        ast::ModifierFlags::Readonly,
                    ) {
                        return self.grammar_error_on_node(
                            parent_store.name(parent).unwrap(),
                            &diagnostics::A_property_of_an_interface_or_type_literal_whose_type_is_a_unique_symbol_type_must_be_readonly,
                            vec![],
                        );
                    }
                }
                _ => {
                    return self.grammar_error_on_node(
                        node,
                        &diagnostics::X_unique_symbol_types_are_not_allowed_here,
                        vec![],
                    );
                }
            }
        } else if operator == ast::Kind::ReadonlyKeyword {
            let inner_type = store.type_node(node).unwrap();
            if store.kind(inner_type) != ast::Kind::ArrayType
                && store.kind(inner_type) != ast::Kind::TupleType
            {
                return self.grammar_error_on_first_token(
                    node,
                    &diagnostics::X_readonly_type_modifier_is_only_permitted_on_array_and_tuple_literal_types,
                    vec![scanner::token_to_string(ast::Kind::SymbolKeyword).into()],
                );
            }
        }

        false
    }

    fn check_grammar_for_invalid_dynamic_name(
        &mut self,
        node: ast::Node,
        message: &'static diagnostics::Message,
    ) -> bool {
        if !self.is_non_bindable_dynamic_name(node) {
            return false;
        }
        let store = self.store_for_node(node);
        let expression = if ast::is_element_access_expression(store, node) {
            ast::skip_parentheses(store, store.argument_expression(node).unwrap())
        } else {
            store.expression(node).unwrap()
        };

        if !ast::is_entity_name_expression(self.store_for_node(expression), expression) {
            return self.grammar_error_on_node(node, message, vec![]);
        }

        false
    }

    // Indicates whether a declaration name is a dynamic name that cannot be late-bound.
    pub(crate) fn is_non_bindable_dynamic_name(&mut self, node: ast::Node) -> bool {
        ast::is_dynamic_name(self.store_for_node(node), node) && !self.is_late_bindable_name(node)
    }

    pub(crate) fn check_grammar_method(
        &mut self,
        node: ast::Node, /*Union[MethodDeclaration, MethodSignature]*/
    ) -> bool {
        if self.check_grammar_function_like_declaration(node) {
            return true;
        }
        let store = self.store_for_node(node);
        let parent = store.parent(node).unwrap();

        if store.kind(node) == ast::Kind::MethodDeclaration {
            if store.kind(parent) == ast::Kind::ObjectLiteralExpression {
                // We only disallow modifier on a method declaration if it is a property of object-literal-expression
                if let Some(modifiers) = store.modifiers(node) {
                    let modifier_nodes = modifiers.nodes();
                    if !(modifier_nodes.len() == 1
                        && modifier_nodes.first().is_some_and(|modifier| {
                            store.kind(modifier) == ast::Kind::AsyncKeyword
                        }))
                    {
                        return self.grammar_error_on_first_token(
                            node,
                            &diagnostics::Modifiers_cannot_appear_here,
                            vec![],
                        );
                    }
                }

                if self.check_grammar_for_invalid_question_mark(
                    store.postfix_token(node),
                    &diagnostics::An_object_member_cannot_be_declared_optional,
                ) {
                    return true;
                }
                if self.check_grammar_for_invalid_exclamation_token(
                    store.postfix_token(node),
                    &diagnostics::A_definite_assignment_assertion_is_not_permitted_in_this_context,
                ) {
                    return true;
                }
                if store.body(node).is_none() {
                    return self.grammar_error_at_pos(
                        node,
                        (store.loc(node).end() as usize).saturating_sub(1),
                        ";".len(),
                        &diagnostics::X_0_expected,
                        vec!["{".into()],
                    );
                }
            }
            if self.check_grammar_for_generator(node) {
                return true;
            }
        }

        if ast::is_class_like(store, parent) {
            // Technically, computed properties in ambient contexts is disallowed
            // for property declarations and accessors too, not just methods.
            // However, property declarations disallow computed names in general,
            // and accessors are not allowed in ambient contexts in general,
            // so this error only really matters for methods.
            if store.flags(node) & ast::NodeFlags::Ambient != 0 {
                return self.check_grammar_for_invalid_dynamic_name(
                    store.name(node).unwrap(),
                    &diagnostics::A_computed_property_name_in_an_ambient_context_must_refer_to_an_expression_whose_type_is_a_literal_type_or_a_unique_symbol_type,
                );
            } else if store.kind(node) == ast::Kind::MethodDeclaration && store.body(node).is_none()
            {
                return self.check_grammar_for_invalid_dynamic_name(
                    store.name(node).unwrap(),
                    &diagnostics::A_computed_property_name_in_a_method_overload_must_refer_to_an_expression_whose_type_is_a_literal_type_or_a_unique_symbol_type,
                );
            }
        } else if store.kind(parent) == ast::Kind::InterfaceDeclaration {
            return self.check_grammar_for_invalid_dynamic_name(
                store.name(node).unwrap(),
                &diagnostics::A_computed_property_name_in_an_interface_must_refer_to_an_expression_whose_type_is_a_literal_type_or_a_unique_symbol_type,
            );
        } else if store.kind(parent) == ast::Kind::TypeLiteral {
            return self.check_grammar_for_invalid_dynamic_name(
                store.name(node).unwrap(),
                &diagnostics::A_computed_property_name_in_a_type_literal_must_refer_to_an_expression_whose_type_is_a_literal_type_or_a_unique_symbol_type,
            );
        }

        false
    }

    pub(crate) fn check_grammar_break_or_continue_statement(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let target_label = store.label(node);
        let mut current: Option<ast::Node> = Some(node);
        while let Some(current_node) = current {
            if ast::is_function_like_or_class_static_block_declaration(
                self.store_for_node(current_node),
                Some(current_node),
            ) {
                return self.grammar_error_on_node(
                    node,
                    &diagnostics::Jump_target_cannot_cross_function_boundary,
                    vec![],
                );
            }

            match self.store_for_node(current_node).kind(current_node) {
                ast::Kind::LabeledStatement => {
                    if let Some(target_label) = target_label {
                        let current_store = self.store_for_node(current_node);
                        if self.node_text(current_store.label(current_node).unwrap())
                            == self.node_text(target_label)
                        {
                            // found matching label - verify that label usage is correct
                            // continue can only target labels that are on iteration statements
                            let statement = current_store.statement(current_node).unwrap();
                            let is_misplaced_continue_label = store.kind(node)
                                == ast::Kind::ContinueStatement
                                && !ast::is_iteration_statement(
                                    self.store_for_node(statement),
                                    &statement,
                                    true, /*lookInLabeledStatements*/
                                );

                            if is_misplaced_continue_label {
                                return self.grammar_error_on_node(
                                    node,
                                    &diagnostics::A_continue_statement_can_only_jump_to_a_label_of_an_enclosing_iteration_statement,
                                    vec![],
                                );
                            }

                            return false;
                        }
                    }
                }
                ast::Kind::SwitchStatement => {
                    if store.kind(node) == ast::Kind::BreakStatement && target_label.is_none() {
                        // unlabeled break within switch statement - ok
                        return false;
                    }
                }
                _ => {
                    if ast::is_iteration_statement(
                        self.store_for_node(current_node),
                        &current_node,
                        false, /*lookInLabeledStatements*/
                    ) && target_label.is_none()
                    {
                        // unlabeled break or continue within iteration statement - ok
                        return false;
                    }
                }
            }

            current = self.store_for_node(current_node).parent(current_node);
        }

        if target_label.is_some() {
            let message = if store.kind(node) == ast::Kind::BreakStatement {
                &diagnostics::A_break_statement_can_only_jump_to_a_label_of_an_enclosing_statement
            } else {
                &diagnostics::A_continue_statement_can_only_jump_to_a_label_of_an_enclosing_iteration_statement
            };

            self.grammar_error_on_node(node, message, vec![])
        } else {
            let message = if store.kind(node) == ast::Kind::BreakStatement {
                &diagnostics::A_break_statement_can_only_be_used_within_an_enclosing_iteration_or_switch_statement
            } else {
                &diagnostics::A_continue_statement_can_only_be_used_within_an_enclosing_iteration_statement
            };
            self.grammar_error_on_node(node, message, vec![])
        }
    }

    pub(crate) fn check_grammar_binding_element(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if store.dot_dot_dot_token(node).is_some() {
            let parent = store.parent(node).unwrap();
            let elements = store.elements(parent).unwrap();
            if Some(node) != elements.last() {
                return self.grammar_error_on_node(
                    node,
                    &diagnostics::A_rest_element_must_be_last_in_a_destructuring_pattern,
                    vec![],
                );
            }
            self.check_grammar_for_disallowed_trailing_comma(
                Some(elements),
                &diagnostics::A_rest_parameter_or_binding_pattern_may_not_have_a_trailing_comma,
            );

            if store.property_name(node).is_some() {
                return self.grammar_error_on_node(
                    store.name(node).unwrap(),
                    &diagnostics::A_rest_element_cannot_have_a_property_name,
                    vec![],
                );
            }
        }

        if store.dot_dot_dot_token(node).is_some() && store.initializer(node).is_some() {
            // Error on equals token which immediately precedes the initializer
            return self.grammar_error_at_pos(
                node,
                (store.loc(store.initializer(node).unwrap()).pos() as usize).saturating_sub(1),
                1,
                &diagnostics::A_rest_element_cannot_have_an_initializer,
                vec![],
            );
        }

        false
    }

    pub(crate) fn check_grammar_variable_declaration(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let node_flags = self.get_combined_node_flags_cached(node);
        let block_scope_kind = node_flags & ast::NodeFlags::BlockScoped;
        let name = store.name(node).unwrap();
        if ast::is_binding_pattern(store, name) {
            match block_scope_kind {
                ast::NodeFlags::AwaitUsing => {
                    return self.grammar_error_on_node(
                        node,
                        &diagnostics::X_0_declarations_may_not_have_binding_patterns,
                        vec!["await using".into()],
                    );
                }
                ast::NodeFlags::Using => {
                    return self.grammar_error_on_node(
                        node,
                        &diagnostics::X_0_declarations_may_not_have_binding_patterns,
                        vec!["using".into()],
                    );
                }
                _ => {}
            }
        }

        let parent = store.parent(node).unwrap();
        let grandparent = store.parent(parent).unwrap();
        if store.kind(grandparent) != ast::Kind::ForInStatement
            && store.kind(grandparent) != ast::Kind::ForOfStatement
        {
            if node_flags & ast::NodeFlags::Ambient != 0 {
                self.check_ambient_initializer(node);
            } else if store.initializer(node).is_none() {
                if ast::is_binding_pattern(store, name) && !ast::is_binding_pattern(store, parent) {
                    return self.grammar_error_on_node(
                        node,
                        &diagnostics::A_destructuring_declaration_must_have_an_initializer,
                        vec![],
                    );
                }
                match block_scope_kind {
                    ast::NodeFlags::AwaitUsing => {
                        return self.grammar_error_on_node(
                            node,
                            &diagnostics::X_0_declarations_must_be_initialized,
                            vec!["await using".into()],
                        );
                    }
                    ast::NodeFlags::Using => {
                        return self.grammar_error_on_node(
                            node,
                            &diagnostics::X_0_declarations_must_be_initialized,
                            vec!["using".into()],
                        );
                    }
                    ast::NodeFlags::Const => {
                        return self.grammar_error_on_node(
                            node,
                            &diagnostics::X_0_declarations_must_be_initialized,
                            vec!["const".into()],
                        );
                    }
                    _ => {}
                }
            }
        }

        let exclamation_token = store.exclamation_token(node);
        let type_node = store.type_node(node);
        let initializer = store.initializer(node);
        if exclamation_token.is_some()
            && (store.kind(grandparent) != ast::Kind::VariableStatement
                || type_node.is_none()
                || initializer.is_some()
                || node_flags & ast::NodeFlags::Ambient != 0)
        {
            let message = if initializer.is_some() {
                &diagnostics::Declarations_with_initializers_cannot_also_have_definite_assignment_assertions
            } else if type_node.is_none() {
                &diagnostics::Declarations_with_definite_assignment_assertions_must_also_have_type_annotations
            } else {
                &diagnostics::A_definite_assignment_assertion_is_not_permitted_in_this_context
            };
            return self.grammar_error_on_node(exclamation_token.unwrap(), message, vec![]);
        }

        if self
            .program
            .get_emit_module_format_of_file(self.source_file_for_node(node))
            < core::ModuleKind::System
            && store.flags(grandparent) & ast::NodeFlags::Ambient == 0
            && ast::has_syntactic_modifier(store, grandparent, ast::ModifierFlags::Export)
        {
            self.check_grammar_for_es_module_marker_in_binding_name(name);
        }

        // 1. LexicalDeclaration : LetOrConst BindingList ;
        // It is a Syntax Error if the BoundNames of BindingList contains "let".
        // 2. ForDeclaration: ForDeclaration : LetOrConst ForBinding
        // It is a Syntax Error if the BoundNames of ForDeclaration contains "let".

        // It is a SyntaxError if a VariableDeclaration or VariableDeclarationNoIn occurs within strict code
        // and its Identifier is eval or arguments
        block_scope_kind != 0 && self.check_grammar_name_in_let_or_const_declarations(name)
    }

    fn check_grammar_for_es_module_marker_in_binding_name(&mut self, name: ast::Node) -> bool {
        let store = self.store_for_node(name);
        if ast::is_identifier(store, name) {
            if self.node_text(name) == "__esModule" {
                return self.grammar_error_on_node_skipped_on_no_emit(
                    name,
                    &diagnostics::Identifier_expected_esModule_is_reserved_as_an_exported_marker_when_transforming_ECMAScript_modules,
                    vec![],
                );
            }
        } else {
            let Some(elements) = store.elements(name) else {
                return false;
            };
            for element in elements.iter() {
                let element_store = self.store_for_node(element);
                if element_store.name(element).is_some() {
                    return self.check_grammar_for_es_module_marker_in_binding_name(
                        element_store.name(element).unwrap(),
                    );
                }
            }
        }
        false
    }

    fn check_grammar_name_in_let_or_const_declarations(
        &mut self,
        name: ast::Node, /*Union[Identifier, BindingPattern]*/
    ) -> bool {
        let store = self.store_for_node(name);
        if store.kind(name) == ast::Kind::Identifier {
            if self.node_text(name) == "let" {
                return self.grammar_error_on_node(name, &diagnostics::X_let_is_not_allowed_to_be_used_as_a_name_in_let_or_const_declarations, vec![]);
            }
        } else {
            let Some(elements) = store.elements(name) else {
                return false;
            };
            for element in elements.iter() {
                let element_store = self.store_for_node(element);
                if let Some(name) = element_store.name(element) {
                    self.check_grammar_name_in_let_or_const_declarations(name);
                }
            }
        }
        false
    }

    pub(crate) fn check_grammar_variable_declaration_list(
        &mut self,
        declaration_list_node: ast::Node,
    ) -> bool {
        let store = self.store_for_node(declaration_list_node);
        let declarations = store.declarations(declaration_list_node).unwrap();
        if self.check_grammar_for_disallowed_trailing_comma(
            Some(declarations),
            &diagnostics::Trailing_comma_not_allowed,
        ) {
            return true;
        }

        if declarations.is_empty() {
            return self.grammar_error_at_pos(
                declaration_list_node,
                declarations.pos() as usize,
                (declarations.end() - declarations.pos()) as usize,
                &diagnostics::Variable_declaration_list_cannot_be_empty,
                vec![],
            );
        }

        let block_scope_flags = store.flags(declaration_list_node) & ast::NodeFlags::BlockScoped;
        let declaration_list_parent = store.parent(declaration_list_node).unwrap();
        if block_scope_flags == ast::NodeFlags::Using
            || block_scope_flags == ast::NodeFlags::AwaitUsing
        {
            if ast::is_for_in_statement(store, declaration_list_parent) {
                return self.grammar_error_on_node(
                    declaration_list_node,
                    core::if_else(block_scope_flags == ast::NodeFlags::Using, &diagnostics::The_left_hand_side_of_a_for_in_statement_cannot_be_a_using_declaration, &diagnostics::The_left_hand_side_of_a_for_in_statement_cannot_be_an_await_using_declaration),
                    vec![],
                );
            }
            if store.flags(declaration_list_node) & ast::NodeFlags::Ambient != 0 {
                return self.grammar_error_on_node(
                    declaration_list_node,
                    core::if_else(
                        block_scope_flags == ast::NodeFlags::Using,
                        &diagnostics::X_using_declarations_are_not_allowed_in_ambient_contexts,
                        &diagnostics::X_await_using_declarations_are_not_allowed_in_ambient_contexts,
                    ),
                    vec![],
                );
            }
            if ast::is_variable_statement(store, declaration_list_parent)
                && (store
                    .parent(declaration_list_parent)
                    .is_some_and(|parent| ast::is_case_clause(store, parent))
                    || store
                        .parent(declaration_list_parent)
                        .is_some_and(|parent| ast::is_default_clause(store, parent)))
            {
                return self.grammar_error_on_node(
                    declaration_list_node,
                    core::if_else(block_scope_flags == ast::NodeFlags::Using, &diagnostics::X_using_declarations_are_not_allowed_in_case_or_default_clauses_unless_contained_within_a_block, &diagnostics::X_await_using_declarations_are_not_allowed_in_case_or_default_clauses_unless_contained_within_a_block),
                    vec![],
                );
            }
        }

        if block_scope_flags == ast::NodeFlags::AwaitUsing {
            return self.check_grammar_await_or_await_using(declaration_list_node);
        }

        false
    }

    pub(crate) fn check_grammar_await_or_await_using(&mut self, node: ast::Node) -> bool {
        // Grammar checking
        let mut has_error = false;
        let store = self.store_for_node(node);
        let container = get_containing_function_or_class_static_block(store, node);
        if container
            .as_ref()
            .is_some_and(|container| ast::is_class_static_block_declaration(store, *container))
        {
            // NOTE: We report this regardless as to whether there are parse diagnostics.
            let message = if ast::is_await_expression(store, node) {
                &diagnostics::X_await_expression_cannot_be_used_inside_a_class_static_block
            } else {
                &diagnostics::X_await_using_statements_cannot_be_used_inside_a_class_static_block
            };
            self.error(node, message, &[] as &[DiagnosticArg]);
            has_error = true;
        } else if store.flags(node) & ast::NodeFlags::AwaitContext == 0 {
            if ast::is_in_top_level_context(store, node) {
                let source_file = self.source_file_for_node(node);
                if !self.has_parse_diagnostics(&source_file) {
                    let mut span = core::TextRange::default();
                    let mut span_calculated = false;
                    if !ast::is_effective_external_module(&source_file, &self.compiler_options) {
                        span = scanner::get_range_of_token_at_position(
                            &source_file,
                            store.loc(node).pos() as usize,
                        );
                        span_calculated = true;
                        let message = if ast::is_await_expression(store, node) {
                            &diagnostics::X_await_expressions_are_only_allowed_at_the_top_level_of_a_file_when_that_file_is_a_module_but_this_file_has_no_imports_or_exports_Consider_adding_an_empty_export_to_make_this_file_a_module
                        } else {
                            &diagnostics::X_await_using_statements_are_only_allowed_at_the_top_level_of_a_file_when_that_file_is_a_module_but_this_file_has_no_imports_or_exports_Consider_adding_an_empty_export_to_make_this_file_a_module
                        };
                        let diagnostic = ast::new_diagnostic(Some(source_file), span, message, &[]);
                        self.diagnostics().add(diagnostic);
                        has_error = true;
                    }
                    match self.module_kind() {
                        core::ModuleKind::Node16
                        | core::ModuleKind::Node18
                        | core::ModuleKind::Node20
                        | core::ModuleKind::NodeNext => {
                            let source_file_meta_data =
                                self.program.get_source_file_meta_data(source_file.path());
                            if source_file_meta_data.implied_node_format
                                == core::ModuleKind::CommonJS
                            {
                                if !span_calculated {
                                    span = scanner::get_range_of_token_at_position(
                                        &source_file,
                                        store.loc(node).pos() as usize,
                                    );
                                }
                                self.diagnostics().add(ast::new_diagnostic(Some(source_file), span, &diagnostics::The_current_file_is_a_CommonJS_module_and_cannot_use_await_at_the_top_level, &[]));
                                has_error = true;
                            } else if self.language_version() < core::ScriptTarget::ES2017 {
                                if !span_calculated {
                                    span = scanner::get_range_of_token_at_position(
                                        &source_file,
                                        store.loc(node).pos() as usize,
                                    );
                                }
                                let message = if ast::is_await_expression(store, node) {
                                    &diagnostics::Top_level_await_expressions_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher
                                } else {
                                    &diagnostics::Top_level_await_using_statements_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher
                                };
                                self.diagnostics().add(ast::new_diagnostic(
                                    Some(source_file),
                                    span,
                                    message,
                                    &[],
                                ));
                                has_error = true;
                            }
                        }
                        core::ModuleKind::ES2022
                        | core::ModuleKind::ESNext
                        | core::ModuleKind::Preserve
                        | core::ModuleKind::System => {
                            if self.language_version() < core::ScriptTarget::ES2017 {
                                if !span_calculated {
                                    span = scanner::get_range_of_token_at_position(
                                        &source_file,
                                        store.loc(node).pos() as usize,
                                    );
                                }
                                let message = if ast::is_await_expression(store, node) {
                                    &diagnostics::Top_level_await_expressions_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher
                                } else {
                                    &diagnostics::Top_level_await_using_statements_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher
                                };
                                self.diagnostics().add(ast::new_diagnostic(
                                    Some(source_file),
                                    span,
                                    message,
                                    &[],
                                ));
                                has_error = true;
                            }
                        }
                        _ => {
                            if !span_calculated {
                                span = scanner::get_range_of_token_at_position(
                                    &source_file,
                                    store.loc(node).pos() as usize,
                                );
                            }
                            let message = if ast::is_await_expression(store, node) {
                                &diagnostics::Top_level_await_expressions_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher
                            } else {
                                &diagnostics::Top_level_await_using_statements_are_only_allowed_when_the_module_option_is_set_to_es2022_esnext_system_node16_node18_node20_nodenext_or_preserve_and_the_target_option_is_set_to_es2017_or_higher
                            };
                            self.diagnostics().add(ast::new_diagnostic(
                                Some(source_file),
                                span,
                                message,
                                &[],
                            ));
                            has_error = true;
                        }
                    }
                }
            } else {
                // use of 'await' in non-async function
                let source_file = self.source_file_for_node(node);
                if !self.has_parse_diagnostics(&source_file) {
                    let span = scanner::get_range_of_token_at_position(
                        &source_file,
                        store.loc(node).pos() as usize,
                    );
                    let message = if ast::is_await_expression(store, node) {
                        &diagnostics::X_await_expressions_are_only_allowed_within_async_functions_and_at_the_top_levels_of_modules
                    } else {
                        &diagnostics::X_await_using_statements_are_only_allowed_within_async_functions_and_at_the_top_levels_of_modules
                    };
                    let mut diagnostic = ast::new_diagnostic(Some(source_file), span, message, &[]);
                    if container.is_some()
                        && store.kind(*container.as_ref().unwrap()) != ast::Kind::Constructor
                        && !has_async_modifier(store, *container.as_ref().unwrap())
                    {
                        let related_info = new_diagnostic_for_node(
                            self.store_for_node(*container.as_ref().unwrap()),
                            Some(*container.as_ref().unwrap()),
                            &diagnostics::Did_you_mean_to_mark_this_function_as_async,
                            vec![] as Vec<DiagnosticArg>,
                        );
                        diagnostic.add_related_info(related_info);
                    }
                    self.diagnostics().add(diagnostic);
                    has_error = true;
                }
            }
        }

        if ast::is_await_expression(store, node)
            && self.is_in_parameter_initializer_before_containing_function(node)
        {
            // NOTE: We report this regardless as to whether there are parse diagnostics.
            self.error(
                node,
                &diagnostics::X_await_expressions_cannot_be_used_in_a_parameter_initializer,
                &[] as &[DiagnosticArg],
            );
            has_error = true;
        }

        has_error
    }

    pub(crate) fn check_grammar_yield_expression(&mut self, node: ast::Node) -> bool {
        let mut has_error = false;
        let store = self.store_for_node(node);
        if store.flags(node) & ast::NodeFlags::YieldContext == 0 {
            self.grammar_error_on_first_token(
                node,
                &diagnostics::A_yield_expression_is_only_allowed_in_a_generator_body,
                vec![],
            );
            has_error = true;
        }
        if self.is_in_parameter_initializer_before_containing_function(node) {
            self.error(
                node,
                &diagnostics::X_yield_expressions_cannot_be_used_in_a_parameter_initializer,
                &[] as &[DiagnosticArg],
            );
            has_error = true;
        }
        has_error
    }

    pub(crate) fn check_grammar_for_disallowed_block_scoped_variable_statement(
        &mut self,
        node: ast::Node,
    ) -> bool {
        let store = self.store_for_node(node);
        let declaration_list = store.declaration_list(node).unwrap();
        if !self.container_allows_block_scoped_variable(store.parent(node).unwrap()) {
            let block_scope_kind =
                self.get_combined_node_flags_cached(declaration_list) & ast::NodeFlags::BlockScoped;
            if block_scope_kind != 0 {
                let keyword = match block_scope_kind {
                    ast::NodeFlags::Let => "let",
                    ast::NodeFlags::Const => "const",
                    ast::NodeFlags::Using => "using",
                    ast::NodeFlags::AwaitUsing => "await using",
                    _ => panic!("Unknown BlockScope flag"),
                };
                self.error(
                    Some(node),
                    &diagnostics::X_0_declarations_can_only_be_declared_inside_a_block,
                    keyword,
                );
            }
        }

        false
    }

    pub(crate) fn container_allows_block_scoped_variable(&mut self, parent: ast::Node) -> bool {
        let store = self.store_for_node(parent);
        match store.kind(parent) {
            ast::Kind::IfStatement
            | ast::Kind::DoStatement
            | ast::Kind::WhileStatement
            | ast::Kind::WithStatement
            | ast::Kind::ForStatement
            | ast::Kind::ForInStatement
            | ast::Kind::ForOfStatement => false,
            ast::Kind::LabeledStatement => {
                self.container_allows_block_scoped_variable(store.parent(parent).unwrap())
            }
            _ => true,
        }
    }

    pub(crate) fn check_grammar_meta_property(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let node_name = store.name(node).unwrap();
        let node_name_ref = node_name;
        let name_text = self.node_text(node_name_ref);
        let keyword_token = store.keyword_token(node).unwrap();

        match keyword_token {
            ast::Kind::NewKeyword => {
                if name_text != "target" {
                    return self.grammar_error_on_node(
                        node_name_ref,
                        &diagnostics::X_0_is_not_a_valid_meta_property_for_keyword_1_Did_you_mean_2,
                        vec![
                            name_text.into(),
                            scanner::token_to_string(keyword_token).into(),
                            "target".into(),
                        ],
                    );
                }
            }
            ast::Kind::ImportKeyword => {
                if name_text != "meta" {
                    let parent = store.parent(node).unwrap();
                    let is_callee = ast::is_call_expression(store, parent)
                        && store.expression(parent) == Some(node);
                    if name_text == "defer" {
                        if !is_callee {
                            return self.grammar_error_at_pos(
                                node,
                                store.loc(node).end() as usize,
                                0,
                                &diagnostics::X_0_expected,
                                vec!["(".into()],
                            );
                        }
                    } else {
                        if is_callee {
                            return self.grammar_error_on_node(
                                node_name_ref,
                                &diagnostics::X_0_is_not_a_valid_meta_property_for_keyword_import_Did_you_mean_meta_or_defer,
                                vec![name_text.into()],
                            );
                        }
                        return self.grammar_error_on_node(
                            node_name_ref,
                            &diagnostics::X_0_is_not_a_valid_meta_property_for_keyword_1_Did_you_mean_2,
                            vec![name_text.into(), scanner::token_to_string(keyword_token).into(), "meta".into()],
                        );
                    }
                }
            }
            _ => {}
        }

        false
    }

    pub(crate) fn check_grammar_constructor_type_parameters(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if let Some(range_) = store.type_parameters(node) {
            let pos = if range_.pos() == range_.end() {
                range_.pos() as usize
            } else {
                let source_file = self.source_file_for_node(node);
                scanner::skip_trivia(source_file.text(), range_.pos() as usize)
            };
            return self.grammar_error_at_pos(
                node,
                pos,
                range_.end() as usize - pos,
                &diagnostics::Type_parameters_cannot_appear_on_a_constructor_declaration,
                vec![],
            );
        }

        false
    }

    pub(crate) fn check_grammar_constructor_type_annotation(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if let Some(t) = store.type_node(node) {
            return self.grammar_error_on_node(
                t,
                &diagnostics::Type_annotation_cannot_appear_on_a_constructor_declaration,
                vec![],
            );
        }
        false
    }

    pub(crate) fn check_grammar_property(
        &mut self,
        node: ast::Node, /*Union[PropertyDeclaration, PropertySignature]*/
    ) -> bool {
        let store = self.store_for_node(node);
        let parent = store.parent(node).unwrap();
        let property_name = store.name(node).unwrap();
        let property_name_store = self.store_for_node(property_name);
        if ast::is_computed_property_name(property_name_store, property_name)
            && property_name_store
                .expression(property_name)
                .is_some_and(|expression| {
                    let expression_store = self.store_for_node(expression);
                    ast::is_binary_expression(expression_store, expression)
                        && expression_store
                            .operator_token(expression)
                            .is_some_and(|token| {
                                expression_store.kind(token) == ast::Kind::InKeyword
                            })
                })
        {
            let members = store.members(parent).unwrap();
            return self.grammar_error_on_node(
                members.first().unwrap(),
                &diagnostics::A_mapped_type_may_not_declare_properties_or_methods,
                vec![],
            );
        }
        let property_name = property_name;
        if ast::is_class_like(store, parent) {
            if ast::is_string_literal(property_name_store, property_name)
                && self.node_text(property_name) == "constructor"
            {
                return self.grammar_error_on_node(
                    property_name,
                    &diagnostics::Classes_may_not_have_a_field_named_constructor,
                    vec![],
                );
            }
            if self.check_grammar_for_invalid_dynamic_name(
                property_name,
                &diagnostics::A_computed_property_name_in_a_class_property_declaration_must_have_a_simple_literal_type_or_a_unique_symbol_type,
            ) {
                return true;
            }
            if ast::is_auto_accessor_property_declaration(store, node)
                && self.check_grammar_for_invalid_question_mark(
                    store.postfix_token(node),
                    &diagnostics::An_accessor_property_cannot_be_declared_optional,
                )
            {
                return true;
            }
        } else if ast::is_interface_declaration(store, parent) {
            if self.check_grammar_for_invalid_dynamic_name(
                property_name,
                &diagnostics::A_computed_property_name_in_an_interface_must_refer_to_an_expression_whose_type_is_a_literal_type_or_a_unique_symbol_type,
            ) {
                return true;
            }
            if !ast::is_property_signature_declaration(store, node) {
                // Interfaces cannot contain property declarations
                panic!("Unexpected node kind {:?}", store.kind(node));
            }
            if let Some(initializer) = store.initializer(node) {
                return self.grammar_error_on_node(
                    initializer,
                    &diagnostics::An_interface_property_cannot_have_an_initializer,
                    vec![],
                );
            }
        } else if ast::is_type_literal_node(store, parent) {
            if self.check_grammar_for_invalid_dynamic_name(
                property_name,
                &diagnostics::A_computed_property_name_in_a_type_literal_must_refer_to_an_expression_whose_type_is_a_literal_type_or_a_unique_symbol_type,
            ) {
                return true;
            }
            if !ast::is_property_signature_declaration(store, node) {
                // Type literals cannot contain property declarations
                panic!("Unexpected node kind {:?}", store.kind(node));
            }
            if let Some(initializer) = store.initializer(node) {
                return self.grammar_error_on_node(
                    initializer,
                    &diagnostics::A_type_literal_property_cannot_have_an_initializer,
                    vec![],
                );
            }
        }

        if store.flags(node) & ast::NodeFlags::Ambient != 0 {
            self.check_ambient_initializer(node);
        }

        if ast::is_property_declaration(store, node) {
            let postfix_token = store.postfix_token(node);
            if let Some(postfix_token) = postfix_token {
                if store.kind(postfix_token) == ast::Kind::ExclamationToken {
                    let message = if store.initializer(node).is_some() {
                        &diagnostics::Declarations_with_initializers_cannot_also_have_definite_assignment_assertions
                    } else if store.type_node(node).is_none() {
                        &diagnostics::Declarations_with_definite_assignment_assertions_must_also_have_type_annotations
                    } else if !ast::is_class_like(store, parent)
                        || store.flags(node) & ast::NodeFlags::Ambient != 0
                        || ast::is_static(store, node)
                        || ast::has_abstract_modifier(store, node)
                    {
                        &diagnostics::A_definite_assignment_assertion_is_not_permitted_in_this_context
                    } else {
                        return false;
                    };
                    return self.grammar_error_on_node(postfix_token, message, vec![]);
                }
            }
        }

        false
    }

    fn check_ambient_initializer(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let (initializer, type_node) = match store.kind(node) {
            ast::Kind::VariableDeclaration
            | ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature => (store.initializer(node), store.type_node(node)),
            _ => panic!("Unexpected node kind {:?}", store.kind(node)),
        };

        if let Some(initializer) = initializer {
            let is_invalid_initializer =
                !(is_initializer_string_or_number_literal_expression(store, initializer)
                    || self.is_initializer_simple_literal_enum_reference(initializer)
                    || store.kind(initializer) == ast::Kind::TrueKeyword
                    || store.kind(initializer) == ast::Kind::FalseKeyword
                    || is_initializer_big_int_literal_expression(store, initializer));
            let is_const_or_readonly = is_declaration_readonly(store, node)
                || ast::is_variable_declaration(store, node) && self.is_var_const_like(node);
            if is_const_or_readonly && type_node.is_none() {
                if is_invalid_initializer {
                    return self.grammar_error_on_node(
                        initializer,
                        &diagnostics::A_const_initializer_in_an_ambient_context_must_be_a_string_or_numeric_literal_or_literal_enum_reference,
                        vec![],
                    );
                }
            } else {
                return self.grammar_error_on_node(
                    initializer,
                    &diagnostics::Initializers_are_not_allowed_in_ambient_contexts,
                    vec![],
                );
            }
        }

        false
    }

    fn is_initializer_simple_literal_enum_reference(&mut self, expr: ast::Node) -> bool {
        let store = self.store_for_node(expr);
        if ast::is_property_access_expression(store, expr) {
            let t = self.check_expression_cached(expr);
            return self.type_flags(t) & TYPE_FLAGS_ENUM_LIKE != 0;
        }

        if ast::is_element_access_expression(store, expr) {
            return is_initializer_string_or_number_literal_expression(
                store,
                store.argument_expression(expr).unwrap(),
            ) && ast::is_entity_name_expression(store, store.expression(expr).unwrap())
                && {
                    let t = self.check_expression_cached(expr);
                    self.type_flags(t) & TYPE_FLAGS_ENUM_LIKE != 0
                };
        }

        false
    }

    fn check_grammar_top_level_element_for_required_declare_modifier(
        &mut self,
        node: ast::Node,
    ) -> bool {
        let store = self.store_for_node(node);
        // A declare modifier is required for any top level .d.ts declaration except export=, export default, export as namespace
        // interfaces and imports categories:
        //
        //  DeclarationElement:
        //     ExportAssignment
        //     export_opt   InterfaceDeclaration
        //     export_opt   TypeAliasDeclaration
        //     export_opt   ImportDeclaration
        //     export_opt   ExternalImportDeclaration
        //     export_opt   AmbientDeclaration
        //
        // TODO: The spec needs to be amended to reflect this grammar.
        if store.kind(node) == ast::Kind::InterfaceDeclaration
            || store.kind(node) == ast::Kind::TypeAliasDeclaration
            || store.kind(node) == ast::Kind::ImportDeclaration
            || store.kind(node) == ast::Kind::JSImportDeclaration
            || store.kind(node) == ast::Kind::ImportEqualsDeclaration
            || store.kind(node) == ast::Kind::ExportDeclaration
            || store.kind(node) == ast::Kind::ExportAssignment
            || store.kind(node) == ast::Kind::NamespaceExportDeclaration
            || ast::has_syntactic_modifier(
                store,
                node,
                ast::ModifierFlags::Ambient
                    | ast::ModifierFlags::Export
                    | ast::ModifierFlags::Default,
            )
        {
            return false;
        }

        self.grammar_error_on_first_token(
            node,
            &diagnostics::Top_level_declarations_in_d_ts_files_must_start_with_either_a_declare_or_export_modifier,
            vec![],
        )
    }

    fn check_grammar_top_level_elements_for_required_declare_modifier(
        &mut self,
        file: &ast::SourceFile,
    ) -> bool {
        let store = file.store();
        let statements = file.statements_view();
        for decl in statements.iter() {
            if ast::is_declaration_node(file.store(), decl)
                || store.kind(decl) == ast::Kind::VariableStatement
            {
                if self.check_grammar_top_level_element_for_required_declare_modifier(decl) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn check_grammar_source_file(&mut self, node: &ast::SourceFile) -> bool {
        node.store().flags(node.as_node()) & ast::NodeFlags::Ambient != 0
            && self.check_grammar_top_level_elements_for_required_declare_modifier(node)
    }

    pub(crate) fn check_grammar_statement_in_ambient_context(&mut self, node: ast::Node) -> bool {
        if self.store_for_node(node).flags(node) & ast::NodeFlags::Ambient != 0 {
            let store = self.store_for_node(node);
            let parent = store.parent(node).unwrap();
            // Find containing block which is either Block, ModuleBlock, SourceFile
            let has_reported = self
                .semantic_state
                .node_has_reported_statement_in_ambient_context(node);
            if !has_reported
                && (ast::is_function_like(store, Some(parent)) || ast::is_accessor(store, parent))
            {
                let reported = self.grammar_error_on_first_token(
                    node,
                    &diagnostics::An_implementation_cannot_be_declared_in_ambient_contexts,
                    vec![],
                );
                self.semantic_state
                    .set_node_has_reported_statement_in_ambient_context(node, reported);
                return reported;
            }

            // We are either parented by another statement, or some sort of block.
            // If we're in a block, we only want to really report an error once
            // to prevent noisiness.  So use a bit on the block to indicate if
            // this has already been reported, and don't report if it has.
            //
            if store.kind(parent) == ast::Kind::Block
                || store.kind(parent) == ast::Kind::ModuleBlock
                || store.kind(parent) == ast::Kind::SourceFile
            {
                let has_reported = self
                    .semantic_state
                    .node_has_reported_statement_in_ambient_context(parent);
                // Check if the containing block ever report this error
                if !has_reported {
                    let reported = self.grammar_error_on_first_token(
                        node,
                        &diagnostics::Statements_are_not_allowed_in_ambient_contexts,
                        vec![],
                    );
                    self.semantic_state
                        .set_node_has_reported_statement_in_ambient_context(parent, reported);
                    return reported;
                }
            } else {
                // We must be parented by a statement.  If so, there's no need
                // to report the error as our parent will have already done it.
                // debug.Assert(ast.IsStatement(node.Parent)) // !!! commented out in strada - fails if uncommented
            }
        }
        false
    }
}

fn is_initializer_string_or_number_literal_expression(
    store: &ast::AstStore,
    expr: ast::Node,
) -> bool {
    if ast::is_string_or_numeric_literal_like(store, expr) {
        return true;
    }
    if store.kind(expr) != ast::Kind::PrefixUnaryExpression {
        return false;
    }
    store.operator(expr) == Some(ast::Kind::MinusToken)
        && store
            .operand(expr)
            .is_some_and(|operand| store.kind(operand) == ast::Kind::NumericLiteral)
}

fn is_initializer_big_int_literal_expression(store: &ast::AstStore, expr: ast::Node) -> bool {
    if store.kind(expr) == ast::Kind::BigIntLiteral {
        return true;
    }

    if store.kind(expr) == ast::Kind::PrefixUnaryExpression {
        return store.operator(expr) == Some(ast::Kind::MinusToken)
            && store
                .operand(expr)
                .is_some_and(|operand| store.kind(operand) == ast::Kind::BigIntLiteral);
    }

    false
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn check_grammar_numeric_literal(&mut self, node: ast::Node) {
        let _store = self.store_for_node(node);
        let node_text = scanner::get_text_of_node(self.source_file_for_node(node), &node);

        // Realism (size) checking
        // We should test against `getTextOfNode(node)` rather than `node.text`, because `node.text` for large numeric literals can contain "."
        // e.g. `node.text` for numeric literal `1100000000000000000000` is `1.1e21`.
        let is_fractional = node_text.contains('.');
        let is_scientific = self
            .store_for_node(node)
            .token_flags(node)
            .is_some_and(|flags| (flags & ast::TokenFlags::SCIENTIFIC).0 != 0);

        // Scientific notation (e.g. 2e54 and 1e00000000010) can't be converted to bigint
        // Fractional numbers (e.g. 9000000000000000.001) are inherently imprecise anyway
        if is_fractional || is_scientific {
            return;
        }

        // Here `node` is guaranteed to be a numeric literal representing an integer.
        // We need to judge whether the integer `node` represents is <= 2 ** 53 - 1, which can be accomplished by comparing to `value` defined below because:
        // 1) when `node` represents an integer <= 2 ** 53 - 1, `node.text` is its exact string representation and thus `value` precisely represents the integer.
        // 2) otherwise, although `node.text` may be imprecise string representation, its mathematical value and consequently `value` cannot be less than 2 ** 53,
        //    thus the result of the predicate won't be affected.
        let value = jsnum::from_string(&self.store_for_node(node).text(node));
        if jsnum::compare(value, jsnum::MAX_SAFE_INTEGER) <= 0 {
            return;
        }

        self.add_error_or_suggestion(
            false,
            create_diagnostic_for_node(
                self.store_for_node(node),
                node,
                &diagnostics::Numeric_literals_with_absolute_values_equal_to_2_53_or_greater_are_too_large_to_be_represented_accurately_as_integers,
            ),
        );
    }

    pub(crate) fn check_grammar_big_int_literal(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let parent = store.parent(node).unwrap();
        let literal_type = ast::is_literal_type_node(store, parent)
            || ast::is_prefix_unary_expression(store, parent)
                && ast::is_literal_type_node(store, store.parent(parent).unwrap());
        if !literal_type {
            // Don't error on BigInt literals in ambient contexts
            if store.flags(node) & ast::NodeFlags::Ambient == 0
                && self.language_version() < core::ScriptTarget::ES2020
            {
                if self.grammar_error_on_node(
                    node,
                    &diagnostics::BigInt_literals_are_not_available_when_targeting_lower_than_ES2020,
                    vec![],
                ) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn check_grammar_import_clause(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        match store.phase_modifier(node) {
            Some(ast::Kind::TypeKeyword) => {
                if store.name(node).is_some() && store.named_bindings(node).is_some() {
                    return self.grammar_error_on_node(node, &diagnostics::A_type_only_import_can_specify_a_default_import_or_named_bindings_but_not_both, vec![]);
                }
                if store.named_bindings(node).is_some_and(|named_bindings| {
                    store.kind(named_bindings) == ast::Kind::NamedImports
                }) {
                    return self.check_grammar_type_only_named_imports_or_exports(
                        store.named_bindings(node).unwrap(),
                    );
                }
            }
            Some(ast::Kind::DeferKeyword) => {
                if store.name(node).is_some() {
                    return self.grammar_error_on_node(
                        node,
                        &diagnostics::Default_imports_are_not_allowed_in_a_deferred_import,
                        vec![],
                    );
                }
                if store.named_bindings(node).is_some_and(|named_bindings| {
                    store.kind(named_bindings) == ast::Kind::NamedImports
                }) {
                    return self.grammar_error_on_node(
                        node,
                        &diagnostics::Named_imports_are_not_allowed_in_a_deferred_import,
                        vec![],
                    );
                }
                if self.module_kind() != core::ModuleKind::ESNext
                    && self.module_kind() != core::ModuleKind::Preserve
                {
                    return self.grammar_error_on_node(node, &diagnostics::Deferred_imports_are_only_supported_when_the_module_flag_is_set_to_esnext_or_preserve, vec![]);
                }
            }
            _ => {}
        }
        false
    }

    fn check_grammar_type_only_named_imports_or_exports(
        &mut self,
        named_bindings: ast::Node,
    ) -> bool {
        let store = self.store_for_node(named_bindings);
        let node_list = store.elements(named_bindings).unwrap();
        for specifier in node_list.iter() {
            let specifier_store = self.store_for_node(specifier);
            let (specifier_is_type_only, message) = if specifier_store.kind(specifier)
                == ast::Kind::ImportSpecifier
            {
                (
                    specifier_store.is_type_only(specifier).unwrap_or(false),
                    &diagnostics::The_type_modifier_cannot_be_used_on_a_named_import_when_import_type_is_used_on_its_import_statement,
                )
            } else {
                (
                    specifier_store.is_type_only(specifier).unwrap_or(false),
                    &diagnostics::The_type_modifier_cannot_be_used_on_a_named_export_when_export_type_is_used_on_its_export_statement,
                )
            };

            if specifier_is_type_only {
                return self.grammar_error_on_first_token(specifier, message, vec![]);
            }
        }

        false
    }

    pub(crate) fn check_grammar_import_call_expression(&mut self, node: ast::Node) -> bool {
        if self.compiler_options.verbatim_module_syntax == core::TSTrue
            && self.module_kind() == core::ModuleKind::CommonJS
        {
            return self.grammar_error_on_node(
                node,
                get_verbatim_module_syntax_error_message(self.store_for_node(node), node),
                vec![],
            );
        }

        let store = self.store_for_node(node);
        if store
            .expression(node)
            .is_some_and(|expression| store.kind(expression) == ast::Kind::MetaProperty)
        {
            if self.module_kind() != core::ModuleKind::ESNext
                && self.module_kind() != core::ModuleKind::Preserve
            {
                return self.grammar_error_on_node(node, &diagnostics::Deferred_imports_are_only_supported_when_the_module_flag_is_set_to_esnext_or_preserve, vec![]);
            }
        } else if self.module_kind() == core::ModuleKind::ES2015 {
            return self.grammar_error_on_node(
                node,
                &diagnostics::Dynamic_imports_are_only_supported_when_the_module_flag_is_set_to_es2020_es2022_esnext_commonjs_amd_system_umd_node16_node18_node20_or_nodenext,
                vec![],
            );
        }

        if store.type_arguments(node).is_some() {
            return self.grammar_error_on_node(
                node,
                &diagnostics::This_use_of_import_is_invalid_import_calls_can_be_written_but_they_must_have_parentheses_and_cannot_have_type_arguments,
                vec![],
            );
        }

        let node_arguments = store.arguments(node).unwrap();
        if !(core::ModuleKind::Node16 <= self.module_kind()
            && self.module_kind() <= core::ModuleKind::NodeNext)
            && self.module_kind() != core::ModuleKind::ESNext
            && self.module_kind() != core::ModuleKind::Preserve
        {
            // We are allowed trailing comma after proposal-import-assertions.
            self.check_grammar_for_disallowed_trailing_comma(
                Some(node_arguments),
                &diagnostics::Trailing_comma_not_allowed,
            );

            if node_arguments.len() > 1 {
                let import_attributes_argument = node_arguments.iter().nth(1).unwrap();
                return self.grammar_error_on_node(
                    import_attributes_argument,
                    &diagnostics::Dynamic_imports_only_support_a_second_argument_when_the_module_option_is_set_to_esnext_node16_node18_node20_nodenext_or_preserve,
                    vec![],
                );
            }
        }

        if node_arguments.is_empty() || node_arguments.len() > 2 {
            return self.grammar_error_on_node(
                node,
                &diagnostics::Dynamic_imports_can_only_accept_a_module_specifier_and_an_optional_set_of_attributes_as_arguments,
                vec![],
            );
        }

        // see: parseArgumentOrArrayLiteralElement, ...we use this function which parse arguments of callExpression to parse specifier for dynamic import.
        // parseArgumentOrArrayLiteralElement allows spread element to be in an argument list which is not allowed as specifier in dynamic import.
        let spread_element = node_arguments
            .iter()
            .find(|argument| ast::is_spread_element(store, *argument));
        if let Some(spread_element) = spread_element {
            return self.grammar_error_on_node(
                spread_element,
                &diagnostics::Argument_of_dynamic_import_cannot_be_spread_element,
                vec![],
            );
        }
        false
    }
}
