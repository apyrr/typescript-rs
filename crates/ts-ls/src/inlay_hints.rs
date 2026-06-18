use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_debug as debug;
use ts_evaluator as evaluator;
use ts_lsproto as lsproto;
use ts_nodebuilder as nodebuilder;
use ts_printer as printer;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::LanguageService;
use crate::lsconv;
use crate::lsutil;
use crate::utilities;

impl LanguageService<'_> {
    pub fn provide_inlay_hint(
        &self,
        ctx: &core::Context,
        params: &lsproto::InlayHintParams,
    ) -> Result<lsproto::InlayHintResponse, core::Error> {
        let user_preferences = self.user_preferences();
        let inlay_hint_preferences = user_preferences.inlay_hints.clone();
        if !is_any_inlay_hint_enabled(inlay_hint_preferences.clone()) {
            return Ok(lsproto::InlayHintsOrNull {
                inlay_hints: None,
                ..Default::default()
            });
        }

        let (program, file) = self.get_program_and_file(params.text_document.uri.to_string());
        let quote_preference = lsutil::get_quote_preference(file, &user_preferences);

        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            file,
            |checker| {
                let range = lsproto::Range {
                    start: lsproto::Position {
                        line: params.range.start.line,
                        character: params.range.start.character,
                    },
                    end: lsproto::Position {
                        line: params.range.end.line,
                        character: params.range.end.character,
                    },
                };
                let mut inlay_hint_state = InlayHintState {
                    ctx,
                    span: self.converters.from_lsp_range(file, range),
                    preferences: inlay_hint_preferences,
                    quote_preference,
                    file,
                    checker,
                    converters: &self.converters,
                    result: Vec::new(),
                };
                inlay_hint_state.visit(file.as_node());
                Ok(lsproto::InlayHintsOrNull {
                    inlay_hints: Some(inlay_hint_state.result.into_iter().map(Some).collect()),
                    ..Default::default()
                })
            },
        )
    }
}

pub struct InlayHintState<'a, 'b, 'state> {
    pub ctx: &'a core::Context,
    pub span: core::TextRange,
    pub preferences: lsutil::InlayHintsPreferences,
    pub quote_preference: lsutil::QuotePreference,
    pub file: &'a ast::SourceFile,
    pub checker: &'b mut checker::Checker<'a, 'state>,
    pub converters: &'a lsconv::Converters,
    pub result: Vec<lsproto::InlayHint>,
}

impl<'a, 'b, 'state> InlayHintState<'a, 'b, 'state> {
    fn store(&self) -> &ast::AstStore {
        self.file.store()
    }

    pub(crate) fn visit(&mut self, node: ast::Node) -> bool {
        let node = &node;
        let loc = self.store().loc(*node);
        if loc.end() - loc.pos() == 0
            || self
                .store()
                .flags(*node)
                .intersects(ast::NodeFlags::Reparsed)
        {
            return false;
        }

        match self.store().kind(*node) {
            ast::Kind::ModuleDeclaration
            | ast::Kind::ClassDeclaration
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::FunctionDeclaration
            | ast::Kind::ClassExpression
            | ast::Kind::FunctionExpression
            | ast::Kind::MethodDeclaration
            | ast::Kind::ArrowFunction => {
                if self.ctx.err().is_some() {
                    return true;
                }
            }
            _ => {}
        }

        if !self.span.intersects(loc) {
            return false;
        }

        if ast::is_type_node(self.store(), *node)
            && !ast::is_expression_with_type_arguments(self.store(), *node)
        {
            return false;
        }

        if self.preferences.include_inlay_variable_type_hints.is_true()
            && ast::is_variable_declaration(self.store(), *node)
        {
            self.visit_variable_like_declaration(*node);
        } else if self
            .preferences
            .include_inlay_property_declaration_type_hints
            .is_true()
            && ast::is_property_declaration(self.store(), *node)
        {
            self.visit_variable_like_declaration(*node);
        } else if self
            .preferences
            .include_inlay_enum_member_value_hints
            .is_true()
            && ast::is_enum_member(self.store(), *node)
        {
            self.visit_enum_member(*node);
        } else if should_show_parameter_name_hints(self.preferences.clone())
            && (ast::is_call_expression(self.store(), *node)
                || ast::is_new_expression(self.store(), *node))
        {
            self.visit_call_or_new_expression(*node);
        } else {
            if self
                .preferences
                .include_inlay_function_parameter_type_hints
                .is_true()
                && ast::is_function_like_declaration(self.store(), Some(*node))
                && ast::has_context_sensitive_parameters(self.store(), node)
            {
                self.visit_function_like_for_parameter_type(*node);
            }
            if self
                .preferences
                .include_inlay_function_like_return_type_hints
                .is_true()
                && is_signature_supporting_return_annotation(self.store(), *node)
            {
                self.visit_function_declaration_like_for_return_type(*node);
            }
        }
        let children: Vec<_> = self.store().children(*node).into_iter().flatten().collect();
        for child in children {
            if self.visit(child) {
                return true;
            }
        }
        false
    }

    // FunctionDeclaration | MethodDeclaration | GetAccessor | FunctionExpression | ArrowFunction
    pub(crate) fn visit_function_declaration_like_for_return_type(&mut self, decl: ast::Node) {
        let decl = &decl;
        if ast::is_arrow_function(self.store(), *decl)
            && !astnav::has_child_of_kind(*decl, ast::Kind::OpenParenToken, self.file)
        {
            return;
        }

        let type_annotation = self.store().r#type(*decl);
        if type_annotation.is_some() || self.store().body(*decl).is_none() {
            return;
        }

        let signature = self.checker.get_signature_from_declaration_public(*decl);

        let type_predicate = self
            .checker
            .get_type_predicate_of_signature_public(signature);

        if let Some(type_predicate) = type_predicate {
            if self
                .checker
                .type_predicate_type_public(type_predicate)
                .is_some()
            {
                let hint_parts = self.type_predicate_to_inlay_hint_parts(type_predicate);
                self.add_type_hints(hint_parts, self.get_type_annotation_position(*decl));
                return;
            }
        }

        let return_type = self.checker.get_return_type_of_signature_public(signature);
        if is_module_reference_type(self.checker, return_type) {
            return;
        }

        let hint_parts = self.type_to_inlay_hint_parts(return_type);
        self.add_type_hints(hint_parts, self.get_type_annotation_position(*decl));
    }

    pub(crate) fn visit_call_or_new_expression(&mut self, expr: ast::Node) {
        let expr = &expr;
        let args = self.store().arguments(*expr);
        let Some(args) = args else {
            return;
        };
        if args.is_empty() {
            return;
        }
        let args: Vec<_> = args.iter().collect();

        let signature = self.checker.get_resolved_signature_public(*expr);
        let Some(signature) = signature else {
            return;
        };

        let mut signature_param_pos = 0;
        for original_arg in args.iter() {
            let arg = ast::skip_parentheses(self.store(), *original_arg);
            if should_show_literal_parameter_name_hints_only(self.preferences.clone())
                && !is_hintable_literal(self.store(), arg)
            {
                signature_param_pos += 1;
                continue;
            }

            let mut spread_args = 0;
            if ast::is_spread_element(self.store(), arg) {
                let expression = self.store().expression(arg).unwrap();
                let spread_type = self.checker.get_type_at_location(expression);
                if self.checker.is_tuple_type_public(spread_type) {
                    let element_flags = self.checker.tuple_element_flags_public(spread_type);
                    let fixed_length = self.checker.tuple_fixed_length_public(spread_type);
                    if fixed_length == 0 {
                        continue;
                    }
                    let first_optional_index = element_flags
                        .iter()
                        .position(|f| *f & checker::ELEMENT_FLAGS_REQUIRED == 0)
                        .map(|i| i as i32)
                        .unwrap_or(-1);
                    let required_args = if first_optional_index < 0 {
                        fixed_length as i32
                    } else {
                        first_optional_index
                    };
                    if required_args > 0 {
                        spread_args = required_args;
                    }
                }
            }

            let identifier_info =
                self.get_parameter_identifier_info_at_position(signature, signature_param_pos);
            signature_param_pos += if spread_args > 0 { spread_args } else { 1 };
            let Some(identifier_info) = identifier_info else {
                return;
            };

            let parameter = identifier_info.parameter;
            let parameter_name = identifier_info.name.clone();
            let is_first_variadic_argument = identifier_info.is_rest_parameter;
            let parameter_name_not_same_as_argument = self
                .preferences
                .include_inlay_parameter_name_hints_when_argument_matches_name
                .is_true()
                || !identifier_or_access_expression_postfix_matches_parameter_name(
                    self.store(),
                    arg,
                    &parameter_name,
                );
            if !parameter_name_not_same_as_argument && !is_first_variadic_argument {
                continue;
            }

            if self.leading_comments_contains_parameter_name(arg, &parameter_name) {
                continue;
            }

            self.add_parameter_hints(
                &parameter_name,
                &parameter,
                astnav::get_start_of_node(*original_arg, self.file),
                is_first_variadic_argument,
            )
        }
    }

    pub(crate) fn visit_enum_member(&mut self, member: ast::Node) {
        let member = &member;
        if self.store().initializer(*member).is_some() {
            return;
        }

        let enum_value = self.checker.get_enum_member_value_public(*member);
        if enum_value.is_some() {
            self.add_enum_member_value_hints(
                &evaluator::any_to_string(&enum_value),
                self.store().loc(*member).end(),
            );
        }
    }

    pub(crate) fn visit_variable_like_declaration(&mut self, decl: ast::Node) {
        let decl = &decl;
        let name = self.store().name(*decl).unwrap();
        let property_declaration_has_non_any_type =
            if ast::is_property_declaration(self.store(), *decl) {
                let declaration_type = self.checker.get_type_at_location(*decl);
                self.checker.type_flags_public(declaration_type) & checker::TYPE_FLAGS_ANY == 0
            } else {
                false
            };
        if self.store().initializer(*decl).is_none() && !property_declaration_has_non_any_type
            || ast::is_binding_pattern(self.store(), name)
            || (ast::is_variable_declaration(self.store(), *decl)
                && !is_hintable_declaration(self.store(), *decl))
        {
            return;
        }

        if self.store().r#type(*decl).is_some() {
            return;
        }

        let declaration_type = self.checker.get_type_at_location(*decl);
        if is_module_reference_type(self.checker, declaration_type) {
            return;
        }

        let hint_parts = self.type_to_inlay_hint_parts(declaration_type);
        let mut hint_text = String::new();
        if let Some(string) = &hint_parts.string {
            hint_text = string.clone();
        } else if let Some(label_parts) = &hint_parts.inlay_hint_label_parts {
            for part in label_parts {
                hint_text.push_str(&part.value);
            }
        }
        if !self
            .preferences
            .include_inlay_variable_type_hints_when_type_matches_name
            .is_true()
            && !ast::is_computed_property_name(self.store(), name)
            && stringutil::equate_string_case_insensitive(&self.store().text(name), &hint_text)
        {
            return;
        }
        self.add_type_hints(hint_parts, self.store().loc(name).end());
    }

    pub(crate) fn visit_function_like_for_parameter_type(&mut self, node: ast::Node) {
        let node = &node;
        let signature = self.checker.get_signature_from_declaration_public(*node);

        let mut pos = 0;
        let Some(parameters) = self.store().parameters(*node) else {
            return;
        };
        let parameters: Vec<_> = parameters.iter().collect();
        for param in parameters {
            if is_hintable_declaration(self.store(), param) {
                let symbol = if ast::is_this_parameter(self.store(), param) {
                    self.checker.signature_this_parameter_public(signature)
                } else {
                    Some(self.checker.signature_parameters_public(signature)[pos as usize].clone())
                };
                self.add_parameter_type_hint(param, symbol);
            }
            if ast::is_this_parameter(self.store(), param) {
                continue;
            }
            pos += 1;
        }
    }

    pub(crate) fn add_parameter_type_hint(
        &mut self,
        node: ast::Node,
        symbol: Option<ast::SymbolIdentity>,
    ) {
        if self.store().r#type(node).is_some() || symbol.is_none() {
            return;
        }
        let type_hints = self.get_parameter_declaration_type_hints(symbol.unwrap());
        let Some(type_hints) = type_hints else {
            return;
        };
        let pos = if let Some(question_token) = self.store().question_token(node) {
            self.store().loc(question_token).end()
        } else {
            self.store().loc(self.store().name(node).unwrap()).end()
        };
        self.add_type_hints(type_hints, pos);
    }

    pub(crate) fn get_parameter_declaration_type_hints(
        &mut self,
        symbol: ast::SymbolIdentity,
    ) -> Option<lsproto::StringOrInlayHintLabelParts> {
        let value_declaration = self.checker.symbol_value_declaration_public(symbol)?;
        if !ast::is_parameter_declaration(self.store(), value_declaration) {
            return None;
        }

        let signature_param_type = self
            .checker
            .get_type_of_symbol_identity_at_location_public(symbol, Some(value_declaration))?;
        if is_module_reference_type(self.checker, signature_param_type) {
            return None;
        }

        Some(self.type_to_inlay_hint_parts(signature_param_type))
    }

    pub fn type_to_inlay_hint_parts(
        &mut self,
        t: checker::TypeHandle,
    ) -> lsproto::StringOrInlayHintLabelParts {
        let flags = nodebuilder::FLAGS_IGNORE_ERRORS
            | nodebuilder::FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE
            | nodebuilder::FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE;
        let text = self.checker.type_to_string_ex_public(t, None, flags, None);
        lsproto::StringOrInlayHintLabelParts {
            string: Some(text),
            ..Default::default()
        }
    }

    pub fn type_predicate_to_inlay_hint_parts(
        &mut self,
        type_predicate: checker::TypePredicateHandle,
    ) -> lsproto::StringOrInlayHintLabelParts {
        let _flags = nodebuilder::FLAGS_IGNORE_ERRORS
            | nodebuilder::FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE
            | nodebuilder::FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE;
        let text = self.checker.type_predicate_to_string_public(type_predicate);
        lsproto::StringOrInlayHintLabelParts {
            string: Some(text),
            ..Default::default()
        }
    }

    pub fn add_type_hints(
        &mut self,
        mut hint: lsproto::StringOrInlayHintLabelParts,
        position: i32,
    ) {
        if let Some(string) = &mut hint.string {
            *string = format!(": {string}");
        } else if let Some(parts) = &mut hint.inlay_hint_label_parts {
            parts.insert(
                0,
                lsproto::InlayHintLabelPart {
                    value: ": ".to_string(),
                    ..Default::default()
                },
            );
        }
        self.result.push(lsproto::InlayHint {
            label: hint,
            position: self
                .converters
                .position_to_line_and_character(self.file, position),
            kind: Some(lsproto::InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
        });
    }

    pub fn add_enum_member_value_hints(&mut self, text: &str, position: i32) {
        self.result.push(lsproto::InlayHint {
            label: lsproto::StringOrInlayHintLabelParts {
                string: Some(format!("= {text}")),
                ..Default::default()
            },
            position: self
                .converters
                .position_to_line_and_character(self.file, position),
            kind: None,
            text_edits: None,
            tooltip: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
        });
    }

    pub fn add_parameter_hints(
        &mut self,
        text: &str,
        parameter: &ast::IdentifierNode,
        position: i32,
        is_first_variadic_argument: bool,
    ) {
        let hint_text = format!(
            "{}{}",
            if is_first_variadic_argument {
                "..."
            } else {
                ""
            },
            text
        );
        let display_parts = vec![
            self.get_node_display_part(&hint_text, *parameter),
            lsproto::InlayHintLabelPart {
                value: ":".to_string(),
                ..Default::default()
            },
        ];
        let label_parts = lsproto::StringOrInlayHintLabelParts {
            inlay_hint_label_parts: Some(display_parts),
            ..Default::default()
        };

        self.result.push(lsproto::InlayHint {
            label: label_parts,
            position: self
                .converters
                .position_to_line_and_character(self.file, position),
            kind: Some(lsproto::InlayHintKind::PARAMETER),
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: Some(true),
            data: None,
        });
    }

    pub(crate) fn get_inlay_hint_label_parts(
        &self,
        node: ast::Node,
        _id_to_symbol: std::collections::HashMap<&ast::IdentifierNode, ast::SymbolIdentity>,
    ) -> Vec<lsproto::InlayHintLabelPart> {
        vec![lsproto::InlayHintLabelPart {
            value: scanner::get_text_of_node(self.file, &node),
            ..Default::default()
        }]
    }

    pub(crate) fn visit_for_display_parts(
        &self,
        node: ast::Node,
        _id_to_symbol: &std::collections::HashMap<&ast::IdentifierNode, ast::SymbolIdentity>,
        parts: &mut Vec<lsproto::InlayHintLabelPart>,
    ) {
        let token_string = scanner::token_to_string(self.store().kind(node));
        if !token_string.is_empty() {
            parts.push(lsproto::InlayHintLabelPart {
                value: token_string,
                ..Default::default()
            });
            return;
        }

        if ast::is_literal_expression(self.store(), &node) {
            parts.push(lsproto::InlayHintLabelPart {
                value: self.get_literal_text(node),
                ..Default::default()
            });
            return;
        }

        parts.push(lsproto::InlayHintLabelPart {
            value: scanner::get_text_of_node(self.file, &node),
            ..Default::default()
        });
    }

    pub fn visit_display_part_list(
        &self,
        nodes: &[ast::Node],
        separator: &str,
        id_to_symbol: &std::collections::HashMap<&ast::IdentifierNode, ast::SymbolIdentity>,
        parts: &mut Vec<lsproto::InlayHintLabelPart>,
    ) {
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 {
                push_part(parts, separator);
            }
            self.visit_for_display_parts(*node, id_to_symbol, parts);
        }
    }

    pub(crate) fn visit_parameters_and_type_parameters(
        &self,
        node: ast::Node,
        id_to_symbol: &std::collections::HashMap<&ast::IdentifierNode, ast::SymbolIdentity>,
        parts: &mut Vec<lsproto::InlayHintLabelPart>,
    ) {
        let type_parameters = self.store().type_parameters(node);
        if let Some(type_parameters) = type_parameters
            && !type_parameters.is_empty()
        {
            push_part(parts, "<");
            let type_parameters: Vec<_> = type_parameters.iter().collect();
            self.visit_display_part_list(&type_parameters, ", ", id_to_symbol, parts);
            push_part(parts, ">");
        }
        push_part(parts, "(");
        if let Some(parameters) = self.store().parameters(node) {
            let parameters: Vec<_> = parameters.iter().collect();
            self.visit_display_part_list(&parameters, ", ", id_to_symbol, parts);
        }
        push_part(parts, ")");
    }

    pub fn get_node_display_part(
        &self,
        text: &str,
        node: ast::Node,
    ) -> lsproto::InlayHintLabelPart {
        let pos = astnav::get_start_of_node(node, self.file);
        let end = self.store().loc(node).end();
        lsproto::InlayHintLabelPart {
            value: text.to_string(),
            location: Some(lsproto::Location {
                uri: lsconv::file_name_to_document_uri(&self.file.file_name()),
                range: self
                    .converters
                    .to_lsp_range(self.file, core::new_text_range(pos, end)),
            }),
            ..Default::default()
        }
    }

    pub(crate) fn get_literal_text(&self, node: ast::Node) -> String {
        match self.store().kind(node) {
            ast::Kind::StringLiteral => {
                let text = self.store().text(node);
                if self.quote_preference == lsutil::QuotePreference::Single {
                    return format!(
                        "'{}'",
                        printer::escape_string(text, printer::QuoteChar::SingleQuote)
                    );
                }
                format!(
                    "\"{}\"",
                    printer::escape_string(text, printer::QuoteChar::DoubleQuote)
                )
            }
            ast::Kind::TemplateHead | ast::Kind::TemplateMiddle | ast::Kind::TemplateTail => {
                let text = self.store().text(node);
                let mut raw_text = self.store().raw_text(node).unwrap_or_default();
                if raw_text.is_empty() {
                    raw_text = printer::escape_string(text.clone(), printer::QuoteChar::Backtick);
                }
                match self.store().kind(node) {
                    ast::Kind::TemplateHead => format!("`{}${{", raw_text),
                    ast::Kind::TemplateMiddle => format!("}}{}${{", raw_text),
                    ast::Kind::TemplateTail => format!("}}{}`", raw_text),
                    _ => text,
                }
            }
            _ => self.store().text(node),
        }
    }

    pub fn get_parameter_identifier_info_at_position(
        &mut self,
        signature: checker::SignatureHandle,
        pos: i32,
    ) -> Option<ParameterInfo> {
        let parameters = self.checker.signature_parameters_public(signature);
        let param_count = parameters.len() as i32
            - if self.checker.signature_has_rest_parameter_public(signature) {
                1
            } else {
                0
            };
        if pos < param_count {
            let param = parameters[pos as usize].clone();
            let value_declaration = self.checker.symbol_value_declaration_public(param)?;
            let param_id = get_parameter_declaration_identifier(self.store(), value_declaration)?;
            return Some(ParameterInfo {
                parameter: param_id,
                name: self.store().text(param_id),
                is_rest_parameter: false,
            });
        }

        if (param_count as usize) >= parameters.len() {
            return None;
        }
        let rest_parameter = parameters[param_count as usize];
        let value_declaration = self
            .checker
            .symbol_value_declaration_public(rest_parameter)?;
        let rest_id = get_parameter_declaration_identifier(self.store(), value_declaration)?;

        let rest_type = self
            .checker
            .get_type_of_symbol_identity_public(rest_parameter)?;
        if self.checker.is_tuple_type_public(rest_type) {
            let associated_names = self.checker.tuple_labeled_declarations_public(rest_type);
            let index = pos - param_count;
            if (index as usize) < associated_names.len() {
                if let Some(associated_name) = associated_names[index as usize] {
                    let associated_store = self.file.store();
                    debug::assert(
                        ast::is_identifier(
                            associated_store,
                            associated_store.name(associated_name).unwrap(),
                        ),
                        Some("expected tuple label identifier".to_string()),
                    );
                    let is_rest_tuple_element =
                        if ast::is_named_tuple_member(associated_store, associated_name) {
                            associated_store
                                .dot_dot_dot_token(associated_name)
                                .is_some()
                        } else {
                            associated_store
                                .dot_dot_dot_token(associated_name)
                                .is_some()
                        };
                    let name = associated_store.name(associated_name).unwrap();
                    return Some(ParameterInfo {
                        parameter: name,
                        name: associated_store.text(name),
                        is_rest_parameter: is_rest_tuple_element,
                    });
                }
            }

            return None;
        }

        if pos == param_count {
            let name = self.checker.symbol_name_public(rest_parameter)?;
            return Some(ParameterInfo {
                parameter: rest_id,
                name,
                is_rest_parameter: true,
            });
        }
        None
    }

    pub(crate) fn leading_comments_contains_parameter_name(
        &self,
        node: ast::Node,
        name: &str,
    ) -> bool {
        if !scanner::is_identifier_text(name, self.file.language_variant()) {
            return false;
        }

        let ranges = utilities::get_leading_comment_ranges_of_node(&node, self.file);
        let file_text = self.file.text();
        for range in ranges {
            let comment_text = file_text[range.pos() as usize..range.end() as usize]
                .trim_matches(|ch: char| ch.is_whitespace() || ch == '/' || ch == '*');
            if comment_text == name {
                return true;
            }
        }

        false
    }

    pub(crate) fn get_type_annotation_position(&self, decl: ast::Node) -> i32 {
        let close_paren_token =
            astnav::find_child_of_kind_info(decl, ast::Kind::CloseParenToken, self.file);
        if let Some(close_paren_token) = close_paren_token {
            return close_paren_token.loc.end();
        }
        self.store()
            .parameters(decl)
            .map(|parameters| parameters.end())
            .unwrap_or_else(|| self.store().loc(decl).end())
    }
}

pub struct ParameterInfo {
    pub parameter: ast::IdentifierNode,
    pub name: String,
    pub is_rest_parameter: bool,
}

pub fn should_show_parameter_name_hints(preferences: lsutil::InlayHintsPreferences) -> bool {
    preferences.include_inlay_parameter_name_hints
        == lsutil::IncludeInlayParameterNameHints::Literals
        || preferences.include_inlay_parameter_name_hints
            == lsutil::IncludeInlayParameterNameHints::All
}

pub fn should_show_literal_parameter_name_hints_only(
    preferences: lsutil::InlayHintsPreferences,
) -> bool {
    preferences.include_inlay_parameter_name_hints
        == lsutil::IncludeInlayParameterNameHints::Literals
}

// node is FunctionDeclaration | ArrowFunction | FunctionExpression | MethodDeclaration | GetAccessor
pub(crate) fn is_signature_supporting_return_annotation(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    ast::is_arrow_function(store, node)
        || ast::is_function_expression(store, node)
        || ast::is_function_declaration(store, node)
        || ast::is_method_declaration(store, node)
        || ast::is_get_accessor_declaration(store, node)
}

pub(crate) fn is_hintable_declaration(store: &ast::AstStore, node: ast::Node) -> bool {
    if (ast::is_part_of_parameter_declaration(store, node)
        || ast::is_variable_declaration(store, node) && ast::is_var_const(store, node))
        && store.initializer(node).is_some()
    {
        let initializer_node = store.initializer(node).unwrap();
        let initializer = ast::skip_parentheses(store, initializer_node);
        return !(is_hintable_literal(store, initializer)
            || ast::is_new_expression(store, initializer)
            || ast::is_object_literal_expression(store, initializer)
            || ast::is_assertion_expression(store, initializer));
    }
    true
}

pub(crate) fn is_hintable_literal(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node) {
        ast::Kind::PrefixUnaryExpression => {
            let operand = store.operand(node).unwrap();
            ast::is_literal_expression(store, &operand)
                || ast::is_identifier(store, operand)
                    && ast::is_infinity_or_nan_string(&store.text(operand))
        }
        ast::Kind::TrueKeyword
        | ast::Kind::FalseKeyword
        | ast::Kind::NullKeyword
        | ast::Kind::NoSubstitutionTemplateLiteral
        | ast::Kind::TemplateExpression => true,
        ast::Kind::Identifier => {
            let name = store.text(node);
            name == "undefined" || ast::is_infinity_or_nan_string(&name)
        }
        _ => ast::is_literal_expression(store, &node),
    }
}

pub(crate) fn is_module_reference_type(
    checker: &mut checker::Checker<'_, '_>,
    t: checker::TypeHandle,
) -> bool {
    let symbol = checker.type_symbol_public(t);
    symbol.is_some_and(|symbol| {
        checker
            .symbol_flags_public(symbol)
            .is_some_and(|flags| flags & ast::SYMBOL_FLAGS_MODULE != 0)
    })
}

pub(crate) fn get_parameter_declaration_identifier(
    store: &ast::AstStore,
    value_declaration: ast::Node,
) -> Option<ast::IdentifierNode> {
    if ast::is_parameter_declaration(store, value_declaration)
        && value_declaration.store_id() == store.store_id()
        && ast::is_identifier(store, store.name(value_declaration).unwrap())
    {
        let name = store.name(value_declaration).unwrap();
        return Some(name);
    }
    None
}

pub fn identifier_or_access_expression_postfix_matches_parameter_name(
    store: &ast::AstStore,
    expr: ast::Node,
    parameter_name: &str,
) -> bool {
    if ast::is_identifier(store, expr) {
        return store.text(expr) == parameter_name;
    }
    if ast::is_property_access_expression(store, expr) {
        return store
            .name(expr)
            .is_some_and(|name| store.text(name) == parameter_name);
    }
    false
}

pub fn is_any_inlay_hint_enabled(preferences: lsutil::InlayHintsPreferences) -> bool {
    preferences.include_inlay_parameter_name_hints != lsutil::IncludeInlayParameterNameHints::None
        || preferences
            .include_inlay_function_parameter_type_hints
            .is_true()
        || preferences.include_inlay_variable_type_hints.is_true()
        || preferences
            .include_inlay_property_declaration_type_hints
            .is_true()
        || preferences
            .include_inlay_function_like_return_type_hints
            .is_true()
        || preferences.include_inlay_enum_member_value_hints.is_true()
}

pub fn push_part(parts: &mut Vec<lsproto::InlayHintLabelPart>, value: &str) {
    parts.push(lsproto::InlayHintLabelPart {
        value: value.to_string(),
        ..Default::default()
    });
}
