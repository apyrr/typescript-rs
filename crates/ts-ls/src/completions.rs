use std::sync::OnceLock;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_collections::{self as collections, FastHashMap as HashMap, FastHashMapExt};
use ts_compiler as compiler;
use ts_core as core;
use ts_debug as debug;
use ts_evaluator as evaluator;
use ts_jsnum as jsnum;
use ts_lsproto as lsproto;
use ts_modulespecifiers::CheckerShape;
use ts_nodebuilder as nodebuilder;
use ts_printer as printer;
use ts_printer::EmitTextWriter;
use ts_scanner as scanner;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

use crate::LanguageService;
use crate::autoimport;
use crate::lsutil;
use crate::signaturehelp::get_immediately_containing_argument_info;
use crate::utilities::{
    CaseClauseTracker, TrackerAddValue, TrackerHasValue, get_all_super_type_nodes,
    get_contextual_type_from_parent, get_possible_generic_signatures,
    get_possible_type_arguments_info, is_in_comment,
    is_in_right_side_of_internal_import_equals_declaration, is_in_string, is_type_keyword,
    new_case_clause_tracker, position_belongs_to_node, quote, skip_constraint,
};

pub const ERR_NEEDS_AUTO_IMPORTS: &str = "completion list needs auto imports";

type CompletionSymbol = ast::SymbolIdentity;

fn node_symbol_identity<N: AsRef<ast::Node>>(
    type_checker: &mut checker::Checker<'_, '_>,
    node: N,
) -> Option<CompletionSymbol> {
    type_checker.source_node_symbol_public(node_handle(node))
}

fn completion_symbol_name(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> String {
    type_checker.symbol_name_public(symbol).unwrap_or_default()
}

fn completion_symbol_flags(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> ast::SymbolFlags {
    type_checker
        .symbol_flags_public(symbol)
        .unwrap_or(ast::SYMBOL_FLAGS_NONE)
}

fn completion_symbol_combined_flags(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> ast::SymbolFlags {
    type_checker
        .symbol_combined_local_and_export_flags_public(symbol)
        .unwrap_or_else(|| completion_symbol_flags(type_checker, symbol))
}

fn completion_symbol_declarations(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> Vec<ast::Node> {
    type_checker.collect_symbol_declarations_public(symbol)
}

fn completion_symbol_value_declaration(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> Option<ast::Node> {
    type_checker.symbol_value_declaration_public(symbol)
}

pub fn is_static_property(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
) -> bool {
    completion_symbol_value_declaration(type_checker, symbol).is_some_and(|value_declaration| {
        let Some(declaration_store) = type_checker.source_file_store(value_declaration) else {
            return false;
        };
        node_modifier_flags(declaration_store, value_declaration) & ast::MODIFIER_FLAGS_STATIC
            != ast::MODIFIER_FLAGS_NONE
            && node_parent(declaration_store, value_declaration)
                .as_ref()
                .is_some_and(|parent| ast::is_class_like(declaration_store, *parent))
    })
}

fn completion_symbol_name_for_display(
    _store: &ast::AstStore,
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> String {
    if let Some(value_declaration) = completion_symbol_value_declaration(type_checker, symbol)
        && let Some(declaration_store) = type_checker.source_file_store(value_declaration)
        && ast::is_private_identifier_class_element_declaration(
            declaration_store,
            value_declaration,
        )
        && let Some(name) = node_name(declaration_store, value_declaration)
    {
        return node_text(declaration_store, name);
    }
    let name = completion_symbol_name(type_checker, symbol);
    name
}

fn is_known_symbol_name(name: &str) -> bool {
    name.starts_with(&(ast::INTERNAL_SYMBOL_NAME_PREFIX.to_string() + "#"))
        || name
            .strip_prefix(ast::INTERNAL_SYMBOL_NAME_PREFIX)
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()))
}

fn is_external_module_symbol_identity(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> bool {
    completion_symbol_declarations(type_checker, symbol)
        .iter()
        .any(|decl| {
            ast::is_source_file(store, *decl)
                && (store
                    .as_source_file(*decl)
                    .external_module_indicator()
                    .is_some()
                    || store
                        .as_source_file(*decl)
                        .common_js_module_indicator()
                        .is_some())
        })
}

fn get_completion_symbol_kind(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
    location: &ast::Node,
) -> lsutil::ScriptElementKind {
    let flags = completion_symbol_combined_flags(type_checker, symbol);
    if flags & ast::SYMBOL_FLAGS_CLASS != 0 {
        return lsutil::ScriptElementKind::ClassElement;
    }
    if flags & ast::SYMBOL_FLAGS_ENUM != 0 {
        return lsutil::ScriptElementKind::EnumElement;
    }
    if flags & ast::SYMBOL_FLAGS_TYPE_ALIAS != 0 {
        return lsutil::ScriptElementKind::TypeElement;
    }
    if flags & ast::SYMBOL_FLAGS_INTERFACE != 0 {
        return lsutil::ScriptElementKind::InterfaceElement;
    }
    if flags & ast::SYMBOL_FLAGS_TYPE_PARAMETER != 0 {
        return lsutil::ScriptElementKind::TypeParameterElement;
    }
    if flags & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0 {
        return lsutil::ScriptElementKind::EnumMemberElement;
    }
    if flags & ast::SYMBOL_FLAGS_ALIAS != 0 {
        return lsutil::ScriptElementKind::Alias;
    }
    if flags & ast::SYMBOL_FLAGS_MODULE != 0 {
        return lsutil::ScriptElementKind::ModuleElement;
    }
    if flags & ast::SYMBOL_FLAGS_METHOD != 0 {
        return lsutil::ScriptElementKind::MemberFunctionElement;
    }
    if flags & ast::SYMBOL_FLAGS_PROPERTY != 0 {
        return lsutil::ScriptElementKind::MemberVariableElement;
    }
    if flags & ast::SYMBOL_FLAGS_FUNCTION != 0 {
        return lsutil::ScriptElementKind::FunctionElement;
    }
    if flags & ast::SYMBOL_FLAGS_VARIABLE != 0 {
        if store.kind(*location) == ast::Kind::ThisKeyword && ast::is_expression(store, *location) {
            return lsutil::ScriptElementKind::ParameterElement;
        }
        return lsutil::ScriptElementKind::VariableElement;
    }
    lsutil::ScriptElementKind::Unknown
}

fn get_completion_symbol_modifiers(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> lsutil::ScriptElementKindModifier {
    let declarations = completion_symbol_declarations(type_checker, symbol);
    let mut modifiers = declarations
        .first()
        .and_then(|declaration| {
            type_checker
                .source_file_store(*declaration)
                .map(|declaration_store| {
                    lsutil::get_node_modifiers(declaration_store, None, *declaration)
                })
        })
        .unwrap_or(lsutil::ScriptElementKindModifier::NONE);
    if completion_symbol_flags(type_checker, symbol) & ast::SYMBOL_FLAGS_OPTIONAL != 0 {
        modifiers |= lsutil::ScriptElementKindModifier::OPTIONAL;
    }
    modifiers
}

fn node_handle<N: AsRef<ast::Node>>(node: N) -> ast::Node {
    *node.as_ref()
}

fn node_parent<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.parent(node_handle(node))
}

fn node_name<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::DeclarationName> {
    store.name(node_handle(node))
}

fn node_text<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> String {
    store.text(node_handle(node))
}

fn node_expression<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.expression(node_handle(node))
}

fn node_initializer<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.initializer(node_handle(node))
}

fn node_tag_name<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.tag_name(node_handle(node))
}

fn node_property_name<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.property_name(node_handle(node))
}

fn node_property_name_or_name<N: AsRef<ast::Node>>(
    store: &ast::AstStore,
    node: N,
) -> Option<ast::Node> {
    store.property_name_or_name(node_handle(node))
}

fn node_question_dot_token<N: AsRef<ast::Node>>(
    store: &ast::AstStore,
    node: N,
) -> Option<ast::Node> {
    store.question_dot_token(node_handle(node))
}

fn node_attributes<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.attributes(node_handle(node))
}

fn node_label<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.label(node_handle(node))
}

fn node_module_specifier<N: AsRef<ast::Node>>(
    store: &ast::AstStore,
    node: N,
) -> Option<ast::Expression> {
    store.module_specifier(node_handle(node))
}

fn node_statement_list<N: AsRef<ast::Node>>(
    store: &ast::AstStore,
    node: N,
) -> Option<ast::SourceNodeList<'_>> {
    store.statements(node_handle(node))
}

fn node_body<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.body(node_handle(node))
}

fn node_type<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    store.r#type(node_handle(node))
}

fn node_type_arguments<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Vec<ast::Node> {
    store
        .type_arguments(node_handle(node))
        .map(|args| args.iter().collect())
        .unwrap_or_default()
}

fn node_parameters<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Vec<ast::Node> {
    store
        .parameters(node_handle(node))
        .map(|parameters| parameters.iter().collect())
        .unwrap_or_default()
}

fn node_members<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Vec<ast::Node> {
    store
        .members(node_handle(node))
        .map(|members| members.iter().collect())
        .unwrap_or_default()
}

fn node_properties<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Vec<ast::Node> {
    store
        .properties(node_handle(node))
        .map(|properties| properties.iter().collect())
        .unwrap_or_default()
}

fn node_elements<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Vec<ast::Node> {
    store
        .elements(node_handle(node))
        .map(|elements| elements.iter().collect())
        .unwrap_or_default()
}

fn node_modifier_flags<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> ast::ModifierFlags {
    store
        .modifiers(node_handle(node))
        .map(|modifiers| modifiers.modifier_flags())
        .unwrap_or(ast::ModifierFlags::NONE)
}

fn node_type_expression<N: AsRef<ast::Node>>(store: &ast::AstStore, node: N) -> Option<ast::Node> {
    node_type(store, node)
}

fn synthetic_node_list(
    factory: &mut ast::NodeFactory,
    nodes: impl IntoIterator<Item = ast::Node>,
) -> ast::NodeList {
    factory.new_node_list(
        core::new_text_range(-1, -1),
        core::new_text_range(-1, -1),
        nodes,
    )
}

impl LanguageService<'_> {
    pub fn get_exhaustive_case_snippets<'a>(
        &'a self,
        ctx: &core::Context,
        case_block: ast::Node,
        file: &'a ast::SourceFile,
        position: i32,
        options: &core::CompilerOptions,
        program: &compiler::Program,
        checker: &mut checker::Checker<'a, '_>,
    ) -> Result<Option<lsproto::CompletionItem>, core::Error> {
        let store = file.store();
        let clauses: Vec<ast::Node> = store
            .clauses(case_block)
            .map(|list| list.iter().collect())
            .unwrap_or_default();
        let switch_expression =
            node_parent(store, &case_block).and_then(|parent| node_expression(store, parent));
        let Some(switch_expression) = switch_expression else {
            return Ok(None);
        };
        let switch_type = checker.get_type_at_location(switch_expression);
        if checker.is_union_type_public(switch_type)
            && checker
                .type_types_public(switch_type)
                .iter()
                .all(|t| is_literal(checker, *t))
        {
            // Collect constant values in existing clauses.
            let mut tracker = new_case_clause_tracker(store, checker, &clauses);
            let target = options.get_emit_script_target();
            let quote_preference = lsutil::get_quote_preference(file, &self.user_preferences());
            // Tolerate a nil import adder in untitled files.
            let mut import_adder: Option<autoimport::ImportAdder<'a>> = None;
            if !tspath::is_dynamic_file_name(&file.file_name()) {
                let view = self.get_prepared_auto_import_view(file)?;
                if let Some(view) = view {
                    import_adder = Some(autoimport::ImportAdder::new(
                        ctx,
                        program,
                        file,
                        view,
                        self.format_options(),
                        &self.converters,
                        self.user_preferences(),
                    ));
                }
            }

            let mut elements = Vec::new();
            let mut factory = ast::new_node_factory(ast::NodeFactoryHooks::default());
            for t in checker.type_types_public(switch_type) {
                // Enums
                if checker.is_enum_literal_type_public(t) {
                    debug::assert(
                        checker.type_symbol_public(t).is_some(),
                        Some("An enum member type should have a symbol".to_string()),
                    );
                    debug::assert(
                        checker
                            .type_symbol_public(t)
                            .and_then(|symbol| checker.symbol_parent_public(symbol))
                            .is_some(),
                        Some(
                            "An enum member type should have a parent symbol (the enum symbol)"
                                .to_string(),
                        ),
                    );
                    // Filter existing enums by their values
                    let mut enum_value = None;
                    if let Some(symbol) = checker.type_symbol_public(t) {
                        let value_declaration = checker.symbol_value_declaration_public(symbol);
                        if let Some(value_declaration) = value_declaration {
                            enum_value =
                                Some(checker.get_enum_member_value_public(value_declaration));
                        }
                    }
                    if let Some(enum_value) = enum_value {
                        match enum_value {
                            evaluator::Value::String(v) => {
                                if tracker.has_value(&TrackerHasValue::String(v.clone())) {
                                    continue;
                                }
                                tracker.add_value(TrackerAddValue::String(v));
                            }
                            evaluator::Value::Number(v) => {
                                if tracker.has_value(&TrackerHasValue::Number(v)) {
                                    continue;
                                }
                                tracker.add_value(TrackerAddValue::Number(v));
                            }
                            _ => {}
                        }
                    }
                    let mut type_emit_context = printer::new_emit_context();
                    let (type_node, id_to_symbol) = checker.type_to_type_node_for_ls_public(
                        &mut type_emit_context,
                        t,
                        Some(case_block),
                        nodebuilder::FLAGS_NONE,
                        nodebuilder::INTERNAL_FLAGS_NONE,
                    );
                    let Some(type_node) = type_node else {
                        return Ok(None);
                    };
                    let type_node = autoimport::type_node_to_auto_importable_type_node(
                        checker,
                        type_emit_context.factory.node_factory.store(),
                        &mut factory,
                        &type_node,
                        import_adder.as_mut(),
                        id_to_symbol,
                    )?;
                    let expr =
                        type_node_to_expression(&type_node, target, quote_preference, &mut factory);
                    let Some(expr) = expr else {
                        return Ok(None);
                    };
                    elements.push(expr.clone());
                } else {
                    let value = checker.literal_value_public(t);
                    let tracker_value = match &value {
                        checker::LiteralValue::String(v) => {
                            Some(TrackerHasValue::String(v.clone()))
                        }
                        checker::LiteralValue::Number(v) => Some(TrackerHasValue::Number(*v)),
                        checker::LiteralValue::PseudoBigInt(v)
                        | checker::LiteralValue::BigInt(v) => {
                            Some(TrackerHasValue::BigInt(v.clone()))
                        }
                        _ => None,
                    };
                    if tracker_value
                        .as_ref()
                        .is_some_and(|value| !tracker.has_value(value))
                    {
                        match value {
                            checker::LiteralValue::PseudoBigInt(mut v)
                            | checker::LiteralValue::BigInt(mut v) => {
                                let big_int = if v.negative {
                                    v.negative = false;
                                    let literal = factory.new_big_int_literal(
                                        &(v.to_string() + "n"),
                                        ast::TOKEN_FLAGS_NONE,
                                    );
                                    factory
                                        .new_prefix_unary_expression(ast::Kind::MinusToken, literal)
                                } else {
                                    factory.new_big_int_literal(
                                        &(v.to_string() + "n"),
                                        ast::TOKEN_FLAGS_NONE,
                                    )
                                };
                                elements.push(big_int);
                            }
                            checker::LiteralValue::Number(v) => {
                                let number = if v.0 < 0.0 {
                                    let literal = factory.new_numeric_literal(
                                        &jsnum::Number(v.0.abs()).to_string(),
                                        ast::TOKEN_FLAGS_NONE,
                                    );
                                    factory
                                        .new_prefix_unary_expression(ast::Kind::MinusToken, literal)
                                } else {
                                    factory
                                        .new_numeric_literal(&v.to_string(), ast::TOKEN_FLAGS_NONE)
                                };
                                elements.push(number);
                            }
                            checker::LiteralValue::String(v) => {
                                let literal = factory.new_string_literal(
                                    v,
                                    if quote_preference == lsutil::QuotePreference::Single {
                                        ast::TOKEN_FLAGS_SINGLE_QUOTE
                                    } else {
                                        ast::TOKEN_FLAGS_NONE
                                    },
                                );
                                elements.push(literal);
                            }
                            _ => {}
                        }
                    }
                }
            }
            if elements.is_empty() {
                return Ok(None);
            }

            let new_clauses: Vec<_> = elements
                .iter()
                .map(|element| {
                    let statements = synthetic_node_list(&mut factory, Vec::new());
                    factory.new_case_or_default_clause(
                        ast::Kind::CaseClause,
                        Some(element.clone()),
                        statements,
                    )
                })
                .collect();
            let new_line_char = self.format_options().new_line_character.clone();
            let mut printer = create_snippet_printer(printer::PrinterOptions {
                remove_comments: true,
                new_line: core::get_new_line_kind(&new_line_char),
                ..Default::default()
            });
            let insert_text = new_clauses
                .iter()
                .enumerate()
                .map(|(i, clause)| {
                    if client_supports_item_snippet(ctx) {
                        format!(
                            "{}${}",
                            printer.print_and_format_node(ctx.clone(), *clause, file),
                            i + 1
                        )
                    } else {
                        printer.print_unescaped_node(*clause)
                    }
                })
                .collect::<Vec<_>>()
                .join(&new_line_char);

            let first_clause = printer.print_unescaped_node(new_clauses[0]);
            let name = first_clause + " ...";

            let additional_text_edits = import_adder.map(|import_adder| import_adder.edits());

            return Ok(Some(lsproto::CompletionItem {
                label: name.clone(),
                kind: Some(lsproto::CompletionItemKind::SNIPPET),
                sort_text: Some(SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string()),
                insert_text: str_ptr_to(insert_text),
                additional_text_edits,
                insert_text_format: if client_supports_item_snippet(ctx) {
                    Some(lsproto::InsertTextFormat::Snippet)
                } else {
                    None
                },
                data: Some(lsproto::CompletionItemData {
                    file_name: file.file_name().to_string(),
                    position,
                    name,
                    source: COMPLETION_SOURCE_SWITCH_CASES.to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }));
        }
        Ok(None)
    }

    pub fn get_completion_data<'a>(
        &self,
        _ctx: &core::Context,
        type_checker: &mut checker::Checker<'a, '_>,
        file: &'a ast::SourceFile,
        position: i32,
        preferences: lsutil::UserPreferences,
        for_item_resolve: bool,
    ) -> Result<Option<CompletionData<'a>>, core::Error> {
        let store = file.store();
        let in_checked_file = is_checked_file(file, self.get_program().options());
        let current_token = astnav::get_token_at_position(file, position);

        let inside_comment = current_token
            .as_ref()
            .and_then(|current_token| is_in_comment(file, position, current_token));

        let mut is_in_snippet_scope = false;
        if inside_comment.is_some() {
            return Ok(None);
        }

        // The decision to provide completion depends on the contextToken, which is determined through the previousToken.
        // Note: 'previousToken' (and thus 'contextToken') can be undefined if we are the beginning of the file
        let is_js_only_location = ast::is_source_file_js(file);
        let (fallback_context_token, fallback_previous_token) = get_relevant_tokens(position, file);
        let (scanned_context_token, scanned_previous_token) =
            get_relevant_token_infos(position, file);
        let use_scanned_context = scanned_context_token.is_some_and(|token| {
            fallback_context_token.is_none()
                || token.node.is_none() && is_scanned_completion_context_token(token.kind)
        });
        let context_token_info = if use_scanned_context {
            scanned_context_token
        } else {
            fallback_context_token.map(|token| astnav::TokenInfo::from_node(store, token))
        };
        let mut context_token = context_token_info.and_then(|token| token.node);
        let previous_token = if use_scanned_context {
            scanned_previous_token.and_then(|token| token.node)
        } else {
            fallback_previous_token
        };

        // Find the node where completion is requested on.
        // Also determine whether we are trying to complete with members of that node
        // or attributes of a JSX tag.
        let mut node = current_token;
        let mut property_access_to_convert = None;
        let mut is_right_of_dot = false;
        let mut is_right_of_question_dot = false;
        let mut is_right_of_open_tag = false;
        let mut is_starting_close_tag = false;
        let mut jsx_initializer = JsxInitializer::default();
        let mut is_jsx_identifier_expected = false;
        let mut import_statement_completion = None;
        let mut location =
            astnav::get_touching_property_name(file, position).unwrap_or_else(|| file.as_node());
        let mut keyword_filters = KEYWORD_COMPLETION_FILTERS_NONE;
        let mut is_new_identifier_location = false;
        // !!! flags := CompletionInfoFlagsNone
        let mut default_commit_characters = Vec::new();

        if let Some(context_token_info_value) = context_token_info {
            if let Some(context_token_value) = context_token_info_value.node {
                let import_statement_completion_info =
                    self.get_import_statement_completion_info(&context_token_value, file);
                if import_statement_completion_info.keyword_completion != ast::Kind::Unknown {
                    if import_statement_completion_info.is_keyword_only_completion {
                        return Ok(Some(CompletionData::Keyword(CompletionDataKeyword {
                            keyword_completions: vec![lsproto::CompletionItem {
                                label: scanner::token_to_string(
                                    import_statement_completion_info.keyword_completion,
                                ),
                                kind: Some(lsproto::CompletionItemKind::KEYWORD),
                                sort_text: Some(SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string()),
                                ..Default::default()
                            }],
                            is_new_identifier_location: import_statement_completion_info
                                .is_new_identifier_location,
                        })));
                    }
                    keyword_filters = keyword_filters_from_syntax_kind(
                        import_statement_completion_info.keyword_completion,
                    );
                }
                if import_statement_completion_info.replacement_span.is_some()
                    && preferences
                        .include_completions_for_import_statements
                        .is_true()
                {
                    // !!! flags |= CompletionInfoFlags.IsImportStatementCompletion;
                    is_new_identifier_location =
                        import_statement_completion_info.is_new_identifier_location;
                    import_statement_completion = Some(import_statement_completion_info);
                }
                // Bail out if this is a known invalid completion location.
                if is_completion_list_blocker(
                    &context_token_value,
                    previous_token.as_ref(),
                    &location,
                    file,
                    position,
                    type_checker,
                ) {
                    if keyword_filters != KEYWORD_COMPLETION_FILTERS_NONE {
                        let (is_new_identifier_location, _) =
                            compute_commit_characters_and_is_new_identifier(
                                Some(&context_token_value),
                                file,
                                position,
                            );
                        return Ok(Some(CompletionData::Keyword(keyword_completion_data(
                            keyword_filters,
                            is_js_only_location,
                            is_new_identifier_location,
                        ))));
                    }
                    return Ok(None);
                }
            }

            let Some(mut parent) = context_token_info_value.parent.or_else(|| {
                context_token.and_then(|context_token| node_parent(store, context_token))
            }) else {
                return Ok(None);
            };
            let context_token_kind = context_token_info_value.kind;
            if context_token_kind == ast::Kind::DotToken
                || context_token_kind == ast::Kind::QuestionDotToken
            {
                is_right_of_dot = context_token_kind == ast::Kind::DotToken;
                is_right_of_question_dot = context_token_kind == ast::Kind::QuestionDotToken;
                match store.kind(parent) {
                    ast::Kind::PropertyAccessExpression => {
                        property_access_to_convert = Some(parent);
                        node = node_expression(store, parent);
                        let left_most_access_expression =
                            ast::get_leftmost_access_expression(store, parent);
                        if ast::node_is_missing(store, Some(left_most_access_expression))
                            || ((ast::is_call_expression(store, node.unwrap())
                                || ast::is_function_like(store, node))
                                && store.loc(node.unwrap()).end()
                                    == context_token_info_value.loc.pos()
                                && lsutil::get_last_token_info(node, file).is_none_or(
                                    |last_token| last_token.kind != ast::Kind::CloseParenToken,
                                ))
                        {
                            // This is likely dot from incorrectly parsed expression and user is starting to write spread
                            // eg: Math.min(./**/)
                            // const x = function (./**/) {}
                            // ({./**/})
                            return Ok(None);
                        }
                    }
                    ast::Kind::QualifiedName => {
                        node = store.left(parent);
                    }
                    ast::Kind::ModuleDeclaration => {
                        node = node_name(store, &parent);
                    }
                    ast::Kind::ImportType => {
                        node = Some(parent);
                    }
                    ast::Kind::MetaProperty => {
                        let Some(token) = lsutil::get_first_token_info(Some(parent), file) else {
                            return Ok(None);
                        };
                        if token.kind != ast::Kind::ImportKeyword
                            && token.kind != ast::Kind::NewKeyword
                        {
                            panic!("Unexpected token kind: {}", token.kind.to_string());
                        }
                        let Some(token_node) = token.node else {
                            return Ok(None);
                        };
                        node = Some(token_node);
                    }
                    _ => {
                        // There is nothing that precedes the dot, so this likely just a stray character
                        // or leading into a '...' token. Just bail out instead.
                        return Ok(None);
                    }
                }
            } else {
                if context_token.is_none()
                    && !is_scanned_completion_context_token(context_token_kind)
                    && !is_scanned_type_context_token(
                        store,
                        context_token_kind,
                        Some(parent),
                        location,
                    )
                {
                    return Ok(None);
                }

                if context_token.is_some() {
                    // <UI.Test /* completion position */ />
                    // If the tagname is a property access expression, we will then walk up to the top most of property access expression.
                    // Then, try to get a JSX container and its associated attributes type.
                    if store.kind(parent) == ast::Kind::PropertyAccessExpression {
                        context_token = Some(parent);
                        let Some(next_parent) = node_parent(store, parent) else {
                            return Ok(None);
                        };
                        parent = next_parent;
                    }

                    // Fix location
                    if parent == location {
                        match current_token.map(|token| store.kind(token)) {
                            Some(ast::Kind::GreaterThanToken) => {
                                if store.kind(parent) == ast::Kind::JsxElement
                                    || store.kind(parent) == ast::Kind::JsxOpeningElement
                                {
                                    location = current_token.unwrap();
                                }
                            }
                            Some(ast::Kind::LessThanSlashToken) => {
                                if store.kind(parent) == ast::Kind::JsxSelfClosingElement {
                                    location = current_token.unwrap();
                                }
                            }
                            _ => {}
                        }
                    }

                    let Some(context_token_value) = context_token else {
                        return Ok(None);
                    };
                    match store.kind(parent) {
                        ast::Kind::JsxClosingElement => {
                            if store.kind(context_token_value) == ast::Kind::LessThanSlashToken {
                                is_starting_close_tag = true;
                                location = context_token_value;
                            }
                        }
                        ast::Kind::BinaryExpression => {
                            if binary_expression_may_be_open_tag(store, parent) {
                                is_jsx_identifier_expected = true;
                            }
                        }
                        ast::Kind::JsxSelfClosingElement
                        | ast::Kind::JsxElement
                        | ast::Kind::JsxOpeningElement => {
                            is_jsx_identifier_expected = true;
                            if store.kind(context_token_value) == ast::Kind::LessThanToken {
                                is_right_of_open_tag = true;
                                location = context_token_value;
                            }
                        }
                        ast::Kind::JsxExpression | ast::Kind::JsxSpreadAttribute => {
                            // First case is for `<div foo={true} [||] />` or `<div foo={true} [||] ></div>`,
                            // `parent` will be `{true}` and `previousToken` will be `}`.
                            // Second case is for `<div foo={true} t[||] ></div>`.
                            // Second case must not match for `<div foo={undefine[||]}></div>`.
                            if previous_token.is_some_and(|previous_token| {
                                store.kind(previous_token) == ast::Kind::CloseBraceToken
                                    || store.kind(previous_token) == ast::Kind::Identifier
                                        && node_parent(store, &previous_token).is_some_and(
                                            |parent| store.kind(parent) == ast::Kind::JsxAttribute,
                                        )
                            }) {
                                is_jsx_identifier_expected = true;
                            }
                        }
                        ast::Kind::JsxAttribute => {
                            // For `<div className="x" [||] ></div>`, `parent` will be JsxAttribute and `previousToken` will be its initializer.
                            if previous_token.is_some_and(|previous_token| {
                                node_initializer(store, parent)
                                    .is_some_and(|initializer| initializer == previous_token)
                                    && store.loc(previous_token).end() < position
                            }) {
                                is_jsx_identifier_expected = true;
                            } else if let Some(previous_token) = previous_token {
                                match store.kind(previous_token) {
                                    ast::Kind::EqualsToken => {
                                        jsx_initializer.is_initializer = true;
                                    }
                                    ast::Kind::Identifier => {
                                        is_jsx_identifier_expected = true;
                                        // For `<div x=[|f/**/|]`, `parent` will be `x` and `previousToken.parent` will be `f` (which is its own JsxAttribute).
                                        // Note for `<div someBool f>` we don't want to treat this as a jsx inializer, instead it's the attribute name.
                                        if node_parent(store, &previous_token)
                                            .is_none_or(|previous_parent| parent != previous_parent)
                                            && node_initializer(store, parent).is_none()
                                            && astnav::has_child_of_kind(
                                                parent,
                                                ast::Kind::EqualsToken,
                                                file,
                                            )
                                        {
                                            jsx_initializer.initializer = Some(previous_token);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let mut completion_kind = COMPLETION_KIND_NONE;
        let has_unresolved_auto_imports = false;
        // This also gets mutated in nested-functions after the return
        let mut symbols: Vec<CompletionSymbol> = Vec::new();
        let mut auto_imports = Vec::new();
        // Keys are indexes of `symbols`.
        let mut symbol_to_origin_info_map = HashMap::new();
        let mut symbol_to_sort_text_map = HashMap::new();
        let mut seen_property_symbols = collections::Set::new();
        let is_type_only_location = import_statement_completion.as_ref().is_some_and(|_| {
            node_parent(store, location)
                .is_some_and(|parent| ast::is_type_only_import_or_export_declaration(store, parent))
        }) || !is_context_token_value_location(
            store,
            context_token.as_ref(),
        ) && (is_possibly_type_argument_position(
            context_token.as_ref(),
            file,
            type_checker,
        ) || ast::is_part_of_type_node(store, location)
            || is_context_token_type_location(store, context_token.as_ref())
            || is_context_token_info_type_location(store, context_token_info.as_ref(), location));

        let add_symbol_origin_info =
            |symbols: &Vec<CompletionSymbol>,
             symbol_to_origin_info_map: &mut HashMap<usize, SymbolOriginInfo>,
             seen_property_symbols: &mut collections::Set<CompletionSymbol>,
             symbol: CompletionSymbol,
             insert_question_dot: bool,
             insert_await: bool| {
                if insert_await && seen_property_symbols.add_if_absent(symbol) {
                    symbol_to_origin_info_map.insert(
                        symbols.len() - 1,
                        SymbolOriginInfo {
                            kind: get_nullable_symbol_origin_info_kind(
                                SYMBOL_ORIGIN_INFO_KIND_PROMISE,
                                insert_question_dot,
                            ),
                            data: None,
                            ..Default::default()
                        },
                    );
                } else if insert_question_dot {
                    symbol_to_origin_info_map.insert(
                        symbols.len() - 1,
                        SymbolOriginInfo {
                            kind: SYMBOL_ORIGIN_INFO_KIND_NULLABLE,
                            data: None,
                            ..Default::default()
                        },
                    );
                }
            };

        let add_symbol_sort_info =
            |symbol_to_sort_text_map: &mut HashMap<CompletionSymbol, SortText>,
             symbol: CompletionSymbol,
             type_checker: &mut checker::Checker<'a, '_>| {
                if is_static_property(type_checker, symbol) {
                    symbol_to_sort_text_map
                        .insert(symbol, SORT_TEXT_LOCAL_DECLARATION_PRIORITY.to_string());
                }
            };
        let add_property_symbol = |symbols: &mut Vec<CompletionSymbol>,
                                   symbol_to_origin_info_map: &mut HashMap<
            usize,
            SymbolOriginInfo,
        >,
                                   symbol_to_sort_text_map: &mut HashMap<
            CompletionSymbol,
            SortText,
        >,
                                   seen_property_symbols: &mut collections::Set<
            CompletionSymbol,
        >,
                                   type_checker: &mut checker::Checker<'a, '_>,
                                   symbol: CompletionSymbol,
                                   insert_await: bool,
                                   insert_question_dot: bool| {
            // For a computed property with an accessible name like `Symbol.iterator`,
            // we'll add a completion for the *name* `Symbol` instead of for the property.
            // If this is e.g. [Symbol.iterator], add a completion for `Symbol`.
            let computed_property_name = completion_symbol_declarations(type_checker, symbol)
                .iter()
                .find_map(|decl| {
                    let declaration_store = type_checker.source_file_store(*decl)?;
                    let name = ast::get_name_of_declaration(declaration_store, Some(*decl));
                    if name.as_ref().is_some_and(|name| {
                        declaration_store.kind(*name) == ast::Kind::ComputedPropertyName
                    }) {
                        name.map(|name| (declaration_store, name))
                    } else {
                        None
                    }
                });

            if let Some((computed_property_store, computed_property_name)) = computed_property_name
            {
                let computed_expression =
                    node_expression(computed_property_store, computed_property_name);
                let left_most_name = computed_expression
                    .as_ref()
                    .and_then(|name| get_left_most_name(computed_property_store, name)); // The completion is for `Symbol`, not `iterator`.
                let mut name_symbol = None;
                if let Some(left_most_name) = left_most_name {
                    name_symbol = type_checker.get_symbol_at_location_public(left_most_name);
                }
                // If this is nested like for `namespace N { export const sym = Symbol(); }`, we'll add the completion for `N`.
                let mut first_accessible_symbol = None;
                if let Some(name_symbol) = name_symbol {
                    let enclosing_declaration = context_token.unwrap_or(location);
                    first_accessible_symbol = get_first_symbol_in_chain(
                        name_symbol,
                        &enclosing_declaration,
                        type_checker,
                    );
                }
                let mut first_accessible_symbol_id: Option<CompletionSymbol> = None;
                if let Some(first_accessible_symbol) = first_accessible_symbol.as_ref() {
                    first_accessible_symbol_id = Some(*first_accessible_symbol);
                }
                if let Some(first_accessible_symbol) = first_accessible_symbol_id
                    && seen_property_symbols.add_if_absent(first_accessible_symbol)
                {
                    symbols.push(first_accessible_symbol);
                    symbol_to_sort_text_map.insert(
                        first_accessible_symbol,
                        SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string(),
                    );
                    let module_symbol = type_checker.symbol_parent_public(first_accessible_symbol);
                    let member_in_exports = if let Some(module_symbol) = module_symbol.as_ref() {
                        type_checker
                            .symbol_export_symbol_public(*module_symbol)
                            .or_else(|| type_checker.get_export_symbol_public(*module_symbol))
                    } else {
                        None
                    };
                    if module_symbol.is_none()
                        || module_symbol.as_ref().is_none_or(|module_symbol| {
                            !is_external_module_symbol_identity(store, type_checker, *module_symbol)
                        })
                        || member_in_exports.is_none_or(|member_in_exports| {
                            first_accessible_symbol != member_in_exports
                        })
                    {
                        symbol_to_origin_info_map.insert(
                            symbols.len() - 1,
                            SymbolOriginInfo {
                                kind: get_nullable_symbol_origin_info_kind(
                                    SYMBOL_ORIGIN_INFO_KIND_SYMBOL_MEMBER,
                                    insert_question_dot,
                                ),
                                data: None,
                                ..Default::default()
                            },
                        );
                    } else {
                        // !!! auto-import symbol
                    }
                } else if first_accessible_symbol_id.is_none_or(|first_accessible_symbol| {
                    !seen_property_symbols.has(&first_accessible_symbol)
                }) {
                    symbols.push(symbol);
                    add_symbol_origin_info(
                        symbols,
                        symbol_to_origin_info_map,
                        seen_property_symbols,
                        symbol,
                        insert_question_dot,
                        insert_await,
                    );
                    add_symbol_sort_info(symbol_to_sort_text_map, symbol, type_checker);
                }
            } else {
                symbols.push(symbol);
                add_symbol_origin_info(
                    symbols,
                    symbol_to_origin_info_map,
                    seen_property_symbols,
                    symbol,
                    insert_question_dot,
                    insert_await,
                );
                add_symbol_sort_info(symbol_to_sort_text_map, symbol, type_checker);
            }
        };

        let add_type_properties =
            |symbols: &mut Vec<CompletionSymbol>,
             symbol_to_origin_info_map: &mut HashMap<usize, SymbolOriginInfo>,
             symbol_to_sort_text_map: &mut HashMap<CompletionSymbol, SortText>,
             seen_property_symbols: &mut collections::Set<CompletionSymbol>,
             type_checker: &mut checker::Checker<'a, '_>,
             is_new_identifier_location: &mut bool,
             default_commit_characters: &mut Vec<String>,
             t: checker::TypeHandle,
             insert_await: bool,
             insert_question_dot: bool| {
                if type_checker.get_string_index_type_public(t).is_some() {
                    *is_new_identifier_location = true;
                    *default_commit_characters = Vec::new();
                }
                if is_right_of_question_dot && !type_checker.get_call_signatures(t).is_empty() {
                    *is_new_identifier_location = true;
                    if default_commit_characters.is_empty() {
                        *default_commit_characters = ALL_COMMIT_CHARACTERS
                            .iter()
                            .map(|s| s.to_string())
                            .collect(); // Only invalid commit character here would be `(`.
                    }
                }

                let property_access_parent;
                let property_access =
                    if node.is_some_and(|node| store.kind(node) == ast::Kind::ImportType) {
                        node.unwrap()
                    } else {
                        property_access_parent = node_parent(store, &node.unwrap());
                        property_access_parent.unwrap_or(location)
                    };

                if in_checked_file {
                    let properties = type_checker.get_apparent_properties(t);
                    for symbol in properties {
                        if type_checker.is_valid_property_access_for_completions_public(
                            property_access,
                            t,
                            symbol,
                        ) {
                            add_property_symbol(
                                symbols,
                                symbol_to_origin_info_map,
                                symbol_to_sort_text_map,
                                seen_property_symbols,
                                type_checker,
                                symbol,
                                false, /*insertAwait*/
                                insert_question_dot,
                            );
                        }
                    }
                } else {
                    // In javascript files, for union types, we don't just get the members that
                    // the individual types have in common, we also include all the members that
                    // each individual type has. This is because we're going to add all identifiers
                    // anyways. So we might as well elevate the members that were at least part
                    // of the individual types to a higher status since we know what they are.                    // lazily fill checker caches on the same query-local checker instance.
                    let properties = get_properties_for_completion(t, type_checker);
                    for symbol in properties {
                        if type_checker.is_valid_property_access_for_completions_public(
                            property_access,
                            t,
                            symbol,
                        ) {
                            symbols.push(symbol);
                        }
                    }
                }

                if insert_await {
                    let promise_type = type_checker.get_promised_type_of_promise_public(t);
                    if let Some(promise_type) = promise_type {
                        let properties = type_checker.get_apparent_properties(promise_type);
                        for symbol in properties {
                            if type_checker.is_valid_property_access_for_completions_public(
                                property_access,
                                promise_type,
                                symbol,
                            ) {
                                add_property_symbol(
                                    symbols,
                                    symbol_to_origin_info_map,
                                    symbol_to_sort_text_map,
                                    seen_property_symbols,
                                    type_checker,
                                    symbol,
                                    true, /*insertAwait*/
                                    insert_question_dot,
                                );
                            }
                        }
                    }
                }
            };

        if is_right_of_dot || is_right_of_question_dot {
            // Right of dot member completion list
            completion_kind = COMPLETION_KIND_PROPERTY_ACCESS;

            // Since this is qualified name check it's a type node location
            let node_value = node.unwrap();
            let is_import_type = ast::is_literal_import_type_node(store, node_value);
            let is_type_location = (is_import_type
                && !store.is_type_of(node_value).unwrap_or(false))
                || node_parent(store, &node_value)
                    .is_some_and(|parent| ast::is_part_of_type_node(store, &parent))
                || is_possibly_type_argument_position(context_token.as_ref(), file, type_checker);
            let is_rhs_of_import_declaration =
                is_in_right_side_of_internal_import_equals_declaration(store, node_value);
            if ast::is_entity_name(store, &node_value)
                || is_import_type
                || ast::is_property_access_expression(store, node_value)
            {
                let is_namespace_name = node_parent(store, &node_value)
                    .is_some_and(|parent| ast::is_module_declaration(store, parent));
                if is_namespace_name {
                    is_new_identifier_location = true;
                    default_commit_characters = Vec::new();
                }
                let symbol = type_checker.get_symbol_at_location_public(node_value);
                if let Some(symbol) = symbol {
                    let symbol = type_checker.skip_alias_public(symbol).unwrap_or(symbol);
                    let symbol_flags = completion_symbol_flags(type_checker, symbol);
                    if symbol_flags & (ast::SYMBOL_FLAGS_MODULE | ast::SYMBOL_FLAGS_ENUM)
                        != ast::SYMBOL_FLAGS_NONE
                    {
                        let value_access_node = if is_import_type {
                            node_value
                        } else {
                            node_parent(store, &node_value).unwrap_or(location)
                        };
                        // Extract module or enum members
                        let exported_symbols = type_checker.get_exports_of_module_public(symbol);
                        for exported_symbol in exported_symbols {
                            let is_valid_access = if is_namespace_name {
                                // At `namespace N.M/**/`, if this is the only declaration of `M`, don't include `M` as a completion.
                                completion_symbol_flags(type_checker, exported_symbol)
                                    & ast::SYMBOL_FLAGS_NAMESPACE
                                    != ast::SYMBOL_FLAGS_NONE
                                    && !completion_symbol_declarations(
                                        type_checker,
                                        exported_symbol,
                                    )
                                    .iter()
                                    .all(|declaration| {
                                        node_parent(store, declaration).is_some_and(|parent| {
                                            parent
                                                == *node_parent(store, &node_value)
                                                    .as_ref()
                                                    .unwrap_or(&location)
                                        })
                                    })
                            } else if is_rhs_of_import_declaration {
                                // Any kind is allowed when dotting off namespace in internal import equals declaration
                                symbol_can_be_referenced_at_type_location(
                                    exported_symbol,
                                    type_checker,
                                    collections::Set::new(),
                                ) || {
                                    let exported_name =
                                        completion_symbol_name(type_checker, exported_symbol);
                                    type_checker.is_valid_property_access_public(
                                        value_access_node,
                                        &exported_name,
                                    )
                                }
                            } else if is_type_location {
                                symbol_can_be_referenced_at_type_location(
                                    exported_symbol,
                                    type_checker,
                                    collections::Set::new(),
                                )
                            } else {
                                let exported_name =
                                    completion_symbol_name(type_checker, exported_symbol);
                                type_checker.is_valid_property_access_public(
                                    value_access_node,
                                    &exported_name,
                                )
                            };
                            if is_valid_access {
                                symbols.push(exported_symbol);
                            }
                        }

                        // If the module is merged with a value, we must get the type of the class and add its properties (for inherited static methods).
                        if !is_type_location
                            && completion_symbol_declarations(type_checker, symbol)
                                .iter()
                                .any(|decl| {
                                    store.kind(*decl) != ast::Kind::SourceFile
                                        && store.kind(*decl) != ast::Kind::ModuleDeclaration
                                        && store.kind(*decl) != ast::Kind::EnumDeclaration
                                })
                        {
                            let symbol_type = type_checker
                                .get_type_of_symbol_identity_at_location_public(
                                    symbol,
                                    Some(node_value),
                                )
                                .unwrap_or_else(|| type_checker.get_error_type());
                            let mut t = type_checker.get_non_optional_type_public(symbol_type);
                            let mut insert_question_dot = false;
                            if type_checker.is_nullable_type_public(t) {
                                let can_correct_to_question_dot = is_right_of_dot
                                    && !is_right_of_question_dot
                                    && !preferences
                                        .include_automatic_optional_chain_completions
                                        .is_false();
                                if can_correct_to_question_dot || is_right_of_question_dot {
                                    t = type_checker.get_non_nullable_type_public(t);
                                    if can_correct_to_question_dot {
                                        insert_question_dot = true;
                                    }
                                }
                            }
                            add_type_properties(
                                &mut symbols,
                                &mut symbol_to_origin_info_map,
                                &mut symbol_to_sort_text_map,
                                &mut seen_property_symbols,
                                type_checker,
                                &mut is_new_identifier_location,
                                &mut default_commit_characters,
                                t,
                                store.flags(node_value) & ast::NodeFlags::AwaitContext
                                    != ast::NodeFlags::None,
                                insert_question_dot,
                            );
                        }
                    }
                }
            }

            if !is_type_location || checker::is_in_type_query(store, node_value) {
                // microsoft/TypeScript#39946. Pulling on the type of a node inside of a function with a contextual `this` parameter can result in a circularity
                // if the `node` is part of the exprssion of a `yield` or `return`. This circularity doesn't exist at compile time because
                // we will check (and cache) the type of `this` *before* checking the type of the node.
                type_checker.try_get_this_type_at_ex_public(
                    node_value, false, /*includeGlobalThis*/
                    None,
                );
                let node_type =
                    get_type_at_location_for_member_completion(type_checker, store, node_value);
                let mut t = type_checker.get_non_optional_type_public(node_type);

                if !is_type_location {
                    let mut insert_question_dot = false;
                    if type_checker.is_nullable_type_public(t) {
                        let can_correct_to_question_dot = is_right_of_dot
                            && !is_right_of_question_dot
                            && !preferences
                                .include_automatic_optional_chain_completions
                                .is_false();

                        if can_correct_to_question_dot || is_right_of_question_dot {
                            t = type_checker.get_non_nullable_type_public(t);
                            if can_correct_to_question_dot {
                                insert_question_dot = true;
                            }
                        }
                    }
                    add_type_properties(
                        &mut symbols,
                        &mut symbol_to_origin_info_map,
                        &mut symbol_to_sort_text_map,
                        &mut seen_property_symbols,
                        type_checker,
                        &mut is_new_identifier_location,
                        &mut default_commit_characters,
                        t,
                        store.flags(node_value) & ast::NodeFlags::AwaitContext
                            != ast::NodeFlags::None,
                        insert_question_dot,
                    );
                } else {
                    let non_nullable_type = type_checker.get_non_nullable_type_public(t);
                    add_type_properties(
                        &mut symbols,
                        &mut symbol_to_origin_info_map,
                        &mut symbol_to_sort_text_map,
                        &mut seen_property_symbols,
                        type_checker,
                        &mut is_new_identifier_location,
                        &mut default_commit_characters,
                        non_nullable_type,
                        false, /*insertAwait*/
                        false, /*insertQuestionDot*/
                    );
                }
            }
        } else {
            let mut globals_result = GLOBALS_SEARCH_CONTINUE;

            if is_right_of_open_tag {
                symbols = type_checker.get_jsx_intrinsic_tag_names_at(location);
                globals_result = GLOBALS_SEARCH_CONTINUE;
                completion_kind = COMPLETION_KIND_GLOBAL;
                keyword_filters = KEYWORD_COMPLETION_FILTERS_NONE;
            } else if is_starting_close_tag {
                let tag_parent = node_parent(store, context_token.unwrap()).unwrap();
                let tag_grandparent = node_parent(store, tag_parent).unwrap();
                let opening_element = store.opening_element(tag_grandparent).unwrap();
                let tag_name = store.tag_name(opening_element).unwrap();
                let tag_symbol = type_checker.get_symbol_at_location_public(tag_name);
                if let Some(tag_symbol) = tag_symbol {
                    symbols = vec![tag_symbol];
                }
                globals_result = GLOBALS_SEARCH_SUCCESS;
                completion_kind = COMPLETION_KIND_GLOBAL;
                keyword_filters = KEYWORD_COMPLETION_FILTERS_NONE;
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                if let Some(type_literal_node) =
                    try_get_type_literal_node(store, context_token.as_ref())
                {
                    let type_literal_parent = node_parent(store, &type_literal_node);
                    let intersection_type_node = if type_literal_parent
                        .as_ref()
                        .is_some_and(|parent| ast::is_intersection_type_node(store, *parent))
                    {
                        type_literal_parent
                    } else {
                        None
                    };
                    let stable_container_type_node =
                        intersection_type_node.unwrap_or(type_literal_node);
                    let container_expected_type = get_constraint_of_type_argument_property(
                        store,
                        Some(&stable_container_type_node),
                        type_checker,
                    );
                    if let Some(container_expected_type) = container_expected_type {
                        let container_actual_type =
                            type_checker.get_type_from_type_node_public(stable_container_type_node);
                        let members =
                            get_properties_for_completion(container_expected_type, type_checker);
                        let existing_members =
                            get_properties_for_completion(container_actual_type, type_checker);
                        let mut existing_member_names = collections::Set::new();
                        for member in existing_members {
                            existing_member_names.add(completion_symbol_name(type_checker, member));
                        }
                        symbols.extend(members.into_iter().filter(|member| {
                            !existing_member_names
                                .has(&completion_symbol_name(type_checker, *member))
                        }));
                        completion_kind = COMPLETION_KIND_OBJECT_PROPERTY_DECLARATION;
                        is_new_identifier_location = true;
                        globals_result = GLOBALS_SEARCH_SUCCESS;
                    }
                }
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                if context_token.is_none_or(|context_token| {
                    store.kind(context_token) != ast::Kind::DotDotDotToken
                }) {
                    if let Some(object_like_container) = try_get_object_like_completion_container(
                        store,
                        context_token_info.as_ref(),
                        position,
                        file,
                    ) {
                        // We're looking up possible property names from contextual/inferred/declared type.
                        completion_kind = COMPLETION_KIND_OBJECT_PROPERTY_DECLARATION;

                        let mut type_members: Vec<CompletionSymbol> = Vec::new();
                        let mut existing_members = Vec::new();

                        if store.kind(object_like_container) == ast::Kind::ObjectLiteralExpression {
                            let instantiated_type = try_get_object_literal_contextual_type(
                                store,
                                &object_like_container,
                                type_checker,
                            );
                            // Check completions for Object property value shorthand
                            if let Some(instantiated_type) = instantiated_type {
                                let completions_type = type_checker.get_contextual_type_public(
                                    object_like_container,
                                    checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES,
                                );
                                let t = completions_type.unwrap_or(instantiated_type);
                                let string_index_type =
                                    type_checker.get_string_index_type_public(t);
                                let number_index_type =
                                    type_checker.get_number_index_type_public(t);
                                is_new_identifier_location =
                                    string_index_type.is_some() || number_index_type.is_some();
                                type_members = get_properties_for_object_expression(
                                    store,
                                    instantiated_type,
                                    completions_type,
                                    &object_like_container,
                                    type_checker,
                                );
                                existing_members = node_properties(store, &object_like_container);

                                if type_members.is_empty() && number_index_type.is_none() {
                                    globals_result = GLOBALS_SEARCH_CONTINUE;
                                } else {
                                    globals_result = GLOBALS_SEARCH_SUCCESS;
                                }
                            } else if store.flags(object_like_container)
                                & ast::NODE_FLAGS_IN_WITH_STATEMENT
                                != ast::NODE_FLAGS_NONE
                            {
                                globals_result = GLOBALS_SEARCH_FAIL;
                            }
                        } else {
                            if store.kind(object_like_container) != ast::Kind::ObjectBindingPattern
                            {
                                panic!(
                                    "Expected 'objectLikeContainer' to be an object binding pattern."
                                );
                            }
                            // We are *only* completing on properties from the type being destructured.
                            is_new_identifier_location = false;
                            let object_like_parent =
                                node_parent(store, object_like_container).unwrap();
                            let root_declaration =
                                ast::get_root_declaration(store, object_like_parent);
                            if !ast::is_variable_like(store, &root_declaration) {
                                panic!("Root declaration is not variable-like.");
                            }

                            // We don't want to complete using the type acquired by the shape
                            // of the binding pattern; we are only interested in types acquired
                            // through type declaration or inference.
                            // Also proceed if rootDeclaration is a parameter and if its containing function expression/arrow function is contextually typed -
                            // type of parameter will flow in from the contextual type of the function.
                            let mut can_get_type = ast::has_initializer(store, &root_declaration)
                                || ast::get_type_annotation_node(store, &root_declaration)
                                    .is_some()
                                || node_parent(store, &root_declaration)
                                    .and_then(|parent| node_parent(store, parent))
                                    .is_some_and(|parent| {
                                        store.kind(parent) == ast::Kind::ForOfStatement
                                    });
                            if !can_get_type && store.kind(root_declaration) == ast::Kind::Parameter
                            {
                                let root_declaration_parent = node_parent(store, &root_declaration);
                                if root_declaration_parent
                                    .as_ref()
                                    .is_some_and(|parent| ast::is_expression(store, *parent))
                                {
                                    can_get_type = type_checker
                                        .get_contextual_type_public(
                                            *root_declaration_parent.as_ref().unwrap(),
                                            checker::CONTEXT_FLAGS_NONE,
                                        )
                                        .is_some();
                                } else if root_declaration_parent.as_ref().is_some_and(|parent| {
                                    store.kind(*parent) == ast::Kind::MethodDeclaration
                                }) || root_declaration_parent.as_ref().is_some_and(
                                    |parent| store.kind(*parent) == ast::Kind::SetAccessor,
                                ) {
                                    let parent_parent = root_declaration_parent
                                        .as_ref()
                                        .and_then(|parent| node_parent(store, parent));
                                    can_get_type = parent_parent
                                        .as_ref()
                                        .is_some_and(|parent| ast::is_expression(store, *parent))
                                        && type_checker
                                            .get_contextual_type_public(
                                                *parent_parent.as_ref().unwrap(),
                                                checker::CONTEXT_FLAGS_NONE,
                                            )
                                            .is_some();
                                }
                            }
                            if can_get_type {
                                let type_for_object =
                                    type_checker.get_type_at_location(object_like_container);
                                type_members = type_checker
                                    .get_properties_of_type_public(type_for_object)
                                    .into_iter()
                                    .filter(|property_symbol| {
                                        type_checker.is_property_accessible_public(
                                            object_like_container,
                                            false, /*isSuper*/
                                            false, /*isWrite*/
                                            type_for_object,
                                            *property_symbol,
                                        )
                                    })
                                    .collect();
                                existing_members = node_elements(store, &object_like_container);
                                globals_result = GLOBALS_SEARCH_SUCCESS;
                            } else {
                                globals_result = GLOBALS_SEARCH_FAIL;
                            }
                        }
                        if !type_members.is_empty() {
                            // Add filtered items to the completion list.
                            let existing_member_refs = existing_members.iter().collect();
                            let (filtered_members, spread_member_names) =
                                filter_object_members_list(
                                    type_members,
                                    existing_member_refs,
                                    file,
                                    position,
                                    type_checker,
                                );
                            symbols.extend(filtered_members.iter().cloned());

                            // Set sort texts.
                            let transform_object_literal_members = preferences
                                .include_completions_with_object_literal_method_snippets
                                .is_true()
                                && store.kind(object_like_container)
                                    == ast::Kind::ObjectLiteralExpression;
                            for member in filtered_members {
                                let member_name = completion_symbol_name(type_checker, member);
                                if spread_member_names.has(&member_name) {
                                    symbol_to_sort_text_map.insert(
                                        member,
                                        SORT_TEXT_MEMBER_DECLARED_BY_SPREAD_ASSIGNMENT.to_string(),
                                    );
                                }
                                if completion_symbol_flags(type_checker, member)
                                    & ast::SYMBOL_FLAGS_OPTIONAL
                                    != ast::SYMBOL_FLAGS_NONE
                                    && !symbol_to_sort_text_map.contains_key(&member)
                                {
                                    symbol_to_sort_text_map
                                        .insert(member, SORT_TEXT_OPTIONAL_MEMBER.to_string());
                                }
                                if transform_object_literal_members {
                                    // !!! object literal member snippet completions
                                }
                            }
                        }
                    }
                }
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE && import_statement_completion.is_some() {
                is_new_identifier_location = true;
                if !for_item_resolve && !tspath::is_dynamic_file_name(&file.file_name()) {
                    if let Some(view) = self.get_prepared_auto_import_view(file)? {
                        auto_imports = view.get_completions(
                            type_checker,
                            "",
                            self.create_lsp_position(position, file),
                            is_right_of_open_tag,
                            is_type_only_location,
                        );
                    }
                }
                globals_result = GLOBALS_SEARCH_SUCCESS;
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                if let Some(context_token_value) = context_token {
                    let mut named_imports_or_exports = None;
                    if store.kind(context_token_value) == ast::Kind::OpenBraceToken
                        || store.kind(context_token_value) == ast::Kind::CommaToken
                    {
                        if node_parent(store, &context_token_value)
                            .as_ref()
                            .is_some_and(|parent| is_named_imports_or_exports(store, *parent))
                        {
                            named_imports_or_exports = node_parent(store, &context_token_value);
                        }
                    } else if is_type_keyword_token_or_identifier(store, context_token_value)
                        && node_parent(store, &context_token_value)
                            .and_then(|parent| node_parent(store, parent))
                            .as_ref()
                            .is_some_and(|parent| is_named_imports_or_exports(store, *parent))
                    {
                        named_imports_or_exports = node_parent(store, &context_token_value)
                            .and_then(|parent| node_parent(store, parent));
                    }

                    if let Some(named_imports_or_exports) = named_imports_or_exports {
                        // We can at least offer `type` at `import { |`
                        if !is_type_keyword_token_or_identifier(store, context_token_value) {
                            keyword_filters = KEYWORD_COMPLETION_FILTERS_TYPE_KEYWORDS;
                        }

                        // try to show exported member for imported/re-exported module
                        let module_specifier =
                            if store.kind(named_imports_or_exports) == ast::Kind::NamedImports {
                                let import_clause =
                                    node_parent(store, named_imports_or_exports).unwrap();
                                let import_declaration = node_parent(store, import_clause).unwrap();
                                node_module_specifier(store, import_declaration)
                            } else {
                                let export_declaration =
                                    node_parent(store, named_imports_or_exports).unwrap();
                                node_module_specifier(store, export_declaration)
                            };
                        if module_specifier.is_none() {
                            is_new_identifier_location = true;
                            globals_result = if store.kind(named_imports_or_exports)
                                == ast::Kind::NamedImports
                            {
                                GLOBALS_SEARCH_FAIL
                            } else {
                                GLOBALS_SEARCH_CONTINUE
                            };
                        } else {
                            let module_specifier_node = module_specifier.unwrap();
                            let module_specifier_symbol =
                                type_checker.get_symbol_at_location_public(module_specifier_node);
                            if let Some(module_specifier_symbol) = module_specifier_symbol {
                                completion_kind = COMPLETION_KIND_MEMBER_LIKE;
                                is_new_identifier_location = false;
                                let exports = type_checker
                                    .get_exports_and_properties_of_module(module_specifier_symbol);
                                let mut existing = collections::Set::new();
                                for element in node_elements(store, named_imports_or_exports) {
                                    if is_currently_editing_node(&element, file, position) {
                                        continue;
                                    }
                                    if let Some(name) = node_property_name_or_name(store, element) {
                                        existing.add(node_text(store, name));
                                    }
                                }
                                symbols.extend(exports.into_iter().filter(|symbol| {
                                    let name = completion_symbol_name_for_display(
                                        store,
                                        type_checker,
                                        *symbol,
                                    );
                                    name != ast::INTERNAL_SYMBOL_NAME_DEFAULT
                                        && !existing.has(&name)
                                }));
                                if symbols.is_empty() {
                                    // If there's nothing else to import, don't offer `type` either.
                                    keyword_filters = KEYWORD_COMPLETION_FILTERS_NONE;
                                }
                                globals_result = GLOBALS_SEARCH_SUCCESS;
                            } else {
                                is_new_identifier_location = true;
                                globals_result = GLOBALS_SEARCH_FAIL;
                            }
                        }
                    }
                }
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                // import { x } from "foo" with { | }
                if let Some(context_token_value) = context_token {
                    let import_attributes = match store.kind(context_token_value) {
                        ast::Kind::OpenBraceToken | ast::Kind::CommaToken => {
                            node_parent(store, &context_token_value)
                        }
                        ast::Kind::ColonToken => node_parent(store, &context_token_value)
                            .and_then(|parent| node_parent(store, parent)),
                        _ => None,
                    };
                    if let Some(import_attributes) = import_attributes {
                        if ast::is_import_attributes(store, import_attributes) {
                            let elements =
                                Some(store.source_attributes(import_attributes).unwrap().nodes());
                            let attribute_names = elements
                                .as_ref()
                                .cloned()
                                .unwrap_or_default()
                                .iter()
                                .filter_map(|el| store.name(*el))
                                .map(|name| node_text(store, name))
                                .collect::<Vec<_>>();
                            let existing = collections::Set::new_from_items(attribute_names);
                            let import_attributes_type =
                                type_checker.get_type_at_location(import_attributes);
                            let uniques = type_checker
                                .get_apparent_properties(import_attributes_type)
                                .into_iter()
                                .filter(|symbol| {
                                    let name = completion_symbol_name_for_display(
                                        store,
                                        type_checker,
                                        *symbol,
                                    );
                                    !existing.has(&name)
                                })
                                .collect::<Vec<_>>();
                            symbols.extend(uniques);
                            globals_result = GLOBALS_SEARCH_SUCCESS;
                        }
                    }
                }
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                if let Some(context_token_value) = context_token {
                    let named_exports = if store.kind(context_token_value)
                        == ast::Kind::OpenBraceToken
                        || store.kind(context_token_value) == ast::Kind::CommaToken
                    {
                        let parent = node_parent(store, &context_token_value);
                        if parent
                            .as_ref()
                            .is_some_and(|parent| ast::is_named_exports(store, *parent))
                        {
                            parent
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(named_exports) = named_exports {
                        let locals_container =
                            ast::find_ancestor(store, Some(named_exports), |store, node| {
                                ast::is_source_file(store, node)
                                    || ast::is_module_declaration(store, node)
                            });
                        completion_kind = COMPLETION_KIND_NONE;
                        is_new_identifier_location = false;
                        if let Some(locals_container) = locals_container {
                            let local_symbol = node_symbol_identity(type_checker, locals_container);
                            type_checker.for_each_source_node_local_public(
                                locals_container,
                                local_symbol,
                                |_, symbol_identity, is_exported| {
                                    symbols.push(symbol_identity);
                                    if is_exported {
                                        symbol_to_sort_text_map.insert(
                                            symbol_identity,
                                            SORT_TEXT_OPTIONAL_MEMBER.to_string(),
                                        );
                                    }
                                },
                            );
                        }
                        globals_result = GLOBALS_SEARCH_SUCCESS;
                    }
                }
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                if try_get_constructor_like_completion_container(store, context_token.as_ref())
                    .is_some()
                {
                    // no members, only keywords
                    completion_kind = COMPLETION_KIND_NONE;
                    // Declaring new property/method/accessor
                    is_new_identifier_location = true;
                    // Has keywords for constructor parameter
                    keyword_filters = KEYWORD_COMPLETION_FILTERS_CONSTRUCTOR_PARAMETER_KEYWORDS;
                    globals_result = GLOBALS_SEARCH_SUCCESS;
                }
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                let decl = try_get_object_type_declaration_completion_container(
                    file,
                    context_token.as_ref(),
                    &location,
                    position,
                );
                if let Some(decl) = decl {
                    // We're looking up possible property names from parent type.
                    completion_kind = COMPLETION_KIND_MEMBER_LIKE;
                    // Declaring new property/method/accessor
                    is_new_identifier_location = true;
                    if context_token.is_some_and(|context_token| {
                        store.kind(context_token) == ast::Kind::AsteriskToken
                    }) {
                        keyword_filters = KEYWORD_COMPLETION_FILTERS_NONE;
                    } else if ast::is_class_like(store, decl) {
                        keyword_filters = KEYWORD_COMPLETION_FILTERS_CLASS_ELEMENT_KEYWORDS;
                    } else {
                        keyword_filters = KEYWORD_COMPLETION_FILTERS_INTERFACE_ELEMENT_KEYWORDS;
                    }

                    // If you're in an interface you don't want to repeat things from super-interface. So just stop here.
                    if ast::is_class_like(store, decl) {
                        let class_element = if context_token.is_some_and(|context_token| {
                            store.kind(context_token) == ast::Kind::SemicolonToken
                        }) {
                            node_parent(store, &context_token.unwrap())
                                .and_then(|parent| node_parent(store, parent))
                        } else {
                            node_parent(store, &context_token.unwrap())
                        };
                        let mut class_element_modifier_flags = ast::ModifierFlags::None;
                        if class_element.as_ref().is_some_and(|class_element| {
                            ast::is_class_element(store, *class_element)
                        }) {
                            class_element_modifier_flags =
                                node_modifier_flags(store, class_element.as_ref().unwrap());
                        }
                        // If this is context token is not something we are editing now, consider if this would lead to be modifier.
                        if context_token.is_some_and(|context_token| {
                            store.kind(context_token) == ast::Kind::Identifier
                                && !is_currently_editing_node(&context_token, file, position)
                        }) {
                            match node_text(store, &context_token.unwrap()).as_str() {
                                "private" => {
                                    class_element_modifier_flags |= ast::ModifierFlags::Private
                                }
                                "static" => {
                                    class_element_modifier_flags |= ast::ModifierFlags::Static
                                }
                                "override" => {
                                    class_element_modifier_flags |= ast::ModifierFlags::Override
                                }
                                _ => {}
                            }
                        }
                        if class_element.as_ref().is_some_and(|class_element| {
                            ast::is_class_static_block_declaration(store, *class_element)
                        }) {
                            class_element_modifier_flags |= ast::ModifierFlags::Static;
                        }

                        // No member list for private methods
                        if class_element_modifier_flags & ast::ModifierFlags::Private
                            == ast::ModifierFlags::None
                        {
                            // List of property symbols of base type that are not private and already implemented
                            let extends_type_node =
                                ast::get_class_extends_heritage_element(store, &decl);
                            let base_type_nodes: Vec<ast::Node> = if ast::is_class_like(store, decl)
                                && class_element_modifier_flags & ast::ModifierFlags::Override
                                    != ast::ModifierFlags::None
                            {
                                extends_type_node.into_iter().collect()
                            } else {
                                get_all_super_type_nodes(store, decl)
                            };
                            let mut base_symbols: Vec<CompletionSymbol> = Vec::new();
                            for base_type_node in &base_type_nodes {
                                // base-type lookup.
                                let t = (type_checker).get_type_at_location(*base_type_node);
                                if class_element_modifier_flags & ast::ModifierFlags::Static
                                    != ast::ModifierFlags::None
                                {
                                    if let Some(symbol) = type_checker.type_symbol_public(t) {
                                        // static base members.
                                        let symbol_type = (type_checker)
                                            .get_type_of_symbol_identity_at_location_public(
                                                symbol,
                                                Some(decl),
                                            )
                                            .unwrap_or_else(|| type_checker.get_error_type());
                                        base_symbols.extend(
                                            (type_checker)
                                                .get_properties_of_type_public(symbol_type),
                                        );
                                    }
                                } else {
                                    base_symbols
                                        .extend((type_checker).get_properties_of_type_public(t));
                                }
                            }

                            let filtered = filter_class_members_list(
                                base_symbols,
                                node_members(store, &decl),
                                class_element_modifier_flags,
                                file,
                                position,
                                type_checker,
                            );
                            symbols.extend(filtered);
                            for (index, symbol) in symbols.iter().enumerate() {
                                let declaration =
                                    completion_symbol_value_declaration(type_checker, *symbol);
                                if declaration.is_some_and(|declaration| {
                                    ast::is_class_element(store, declaration)
                                        && node_name(store, &declaration).is_some()
                                        && ast::is_computed_property_name(
                                            store,
                                            node_name(store, &declaration).unwrap(),
                                        )
                                }) {
                                    let origin = SymbolOriginInfo {
                                        kind: SYMBOL_ORIGIN_INFO_KIND_COMPUTED_PROPERTY_NAME,
                                        data: Some(SymbolOriginInfoData::ComputedPropertyName(
                                            SymbolOriginInfoComputedPropertyName {
                                                // symbol graph as the Go implementation.
                                                symbol_name: type_checker
                                                    .symbol_identity_to_string_public(*symbol)
                                                    .unwrap_or_default(),
                                            },
                                        )),
                                        ..Default::default()
                                    };
                                    symbol_to_origin_info_map.insert(index, origin);
                                }
                            }
                        }
                    }
                    globals_result = GLOBALS_SEARCH_SUCCESS;
                }
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                if let Some(jsx_container) =
                    try_get_containing_jsx_element(context_token.as_ref(), file)
                {
                    // Cursor is inside a JSX self-closing element or opening element.
                    // PORT NOTE: reshaped for borrowck; AST accessors return owned handles here.
                    let attributes = node_attributes(store, jsx_container).unwrap();
                    let attrs_type = type_checker.get_contextual_type_for_jsx_attribute_public(
                        attributes,
                        checker::CONTEXT_FLAGS_NONE,
                    );
                    if let Some(attrs_type) = attrs_type {
                        let completions_type = type_checker
                            .get_contextual_type_for_jsx_attribute_public(
                                attributes,
                                checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES,
                            );
                        let attribute_properties = node_properties(store, attributes);
                        let attribute_property_refs = attribute_properties.iter().collect();
                        let (filtered_symbols, spread_member_names) = filter_jsx_attributes(
                            get_properties_for_object_expression(
                                store,
                                attrs_type,
                                completions_type,
                                &attributes,
                                type_checker,
                            ),
                            attribute_property_refs,
                            store,
                            file,
                            position,
                            type_checker,
                        );

                        symbols.extend(filtered_symbols.iter().cloned());
                        // Set sort texts.
                        for symbol in filtered_symbols {
                            let symbol_name =
                                completion_symbol_name_for_display(store, type_checker, symbol);
                            if spread_member_names.has(&symbol_name) {
                                symbol_to_sort_text_map.insert(
                                    symbol,
                                    SORT_TEXT_MEMBER_DECLARED_BY_SPREAD_ASSIGNMENT.to_string(),
                                );
                            }
                            if completion_symbol_flags(type_checker, symbol)
                                & ast::SYMBOL_FLAGS_OPTIONAL
                                != ast::SYMBOL_FLAGS_NONE
                                && !symbol_to_sort_text_map.contains_key(&symbol)
                            {
                                symbol_to_sort_text_map
                                    .insert(symbol, SORT_TEXT_OPTIONAL_MEMBER.to_string());
                            }
                        }

                        completion_kind = COMPLETION_KIND_MEMBER_LIKE;
                        is_new_identifier_location = false;
                        globals_result = GLOBALS_SEARCH_SUCCESS;
                    }
                }
            }

            if globals_result == GLOBALS_SEARCH_CONTINUE {
                if try_get_function_like_body_completion_container(store, context_token.as_ref())
                    .is_some()
                {
                    keyword_filters = KEYWORD_COMPLETION_FILTERS_FUNCTION_LIKE_BODY_KEYWORDS;
                } else {
                    keyword_filters = KEYWORD_COMPLETION_FILTERS_ALL;
                }
                // Get all entities in the current scope.
                completion_kind = COMPLETION_KIND_GLOBAL;
                let commit_info = compute_commit_characters_and_is_new_identifier(
                    context_token.as_ref(),
                    file,
                    position,
                );
                is_new_identifier_location = commit_info.0;
                default_commit_characters = commit_info.1;

                if previous_token != context_token && previous_token.is_none() {
                    panic!(
                        "Expected 'contextToken' to be defined when different from 'previousToken'."
                    );
                }

                let adjusted_position = if previous_token != context_token {
                    astnav::get_start_of_node(previous_token.unwrap(), file)
                } else {
                    position
                };

                let scanned_context_parent = context_token_info.and_then(|token| {
                    if token.node.is_none()
                        && is_scanned_type_context_token(store, token.kind, token.parent, location)
                    {
                        token.parent
                    } else {
                        None
                    }
                });
                let scope_context_token =
                    context_token.as_ref().copied().or(scanned_context_parent);
                let mut scope_node =
                    get_scope_node(scope_context_token.as_ref(), adjusted_position, file);
                if scope_node.is_none() {
                    scope_node = Some(file.as_node());
                }
                is_in_snippet_scope = is_snippet_scope(store, scope_node.unwrap());

                let symbol_meanings = if is_type_only_location {
                    ast::SYMBOL_FLAGS_NONE
                } else {
                    ast::SYMBOL_FLAGS_VALUE
                } | ast::SYMBOL_FLAGS_TYPE
                    | ast::SYMBOL_FLAGS_NAMESPACE
                    | ast::SYMBOL_FLAGS_ALIAS;
                let type_only_alias_needs_promotion =
                    previous_token.is_some_and(|previous_token| {
                        !ast::is_valid_type_only_alias_use_site(store, &previous_token)
                    });
                let scope_node = scope_node.unwrap();
                symbols
                    .extend(type_checker.get_symbols_in_scope_public(scope_node, symbol_meanings));
                for (index, symbol) in symbols.iter().enumerate() {
                    if !type_checker.is_arguments_symbol(*symbol)
                        && !completion_symbol_declarations(type_checker, *symbol)
                            .iter()
                            .copied()
                            .any(|decl| {
                                if decl.store_id() != store.store_id() {
                                    return false;
                                }
                                ast::get_source_file_of_node(store, Some(decl))
                                    .is_some_and(|source_file| source_file == file.as_node())
                            })
                    {
                        symbol_to_sort_text_map
                            .insert(*symbol, SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string());
                    }
                    if type_only_alias_needs_promotion
                        && completion_symbol_flags(type_checker, *symbol) & ast::SYMBOL_FLAGS_VALUE
                            == ast::SYMBOL_FLAGS_NONE
                    {
                        let type_only_alias_declaration =
                            completion_symbol_declarations(type_checker, *symbol)
                                .iter()
                                .copied()
                                .find(|decl| {
                                    if decl.store_id() != store.store_id() {
                                        return false;
                                    }
                                    ast::is_type_only_import_declaration(store, *decl)
                                });
                        if let Some(type_only_alias_declaration) = type_only_alias_declaration {
                            let origin = SymbolOriginInfo {
                                kind: SYMBOL_ORIGIN_INFO_KIND_TYPE_ONLY_ALIAS,
                                data: Some(SymbolOriginInfoData::TypeOnlyAlias(
                                    SymbolOriginInfoTypeOnlyAlias {
                                        declaration: type_only_alias_declaration,
                                    },
                                )),
                                ..Default::default()
                            };
                            symbol_to_origin_info_map.insert(index, origin);
                        }
                    }
                }

                // Need to insert 'this.' before properties of `this` type.
                if store.kind(scope_node) != ast::Kind::SourceFile {
                    let this_type = type_checker.try_get_this_type_at_ex_public(
                        scope_node,
                        false, /*includeGlobalThis*/
                        if node_parent(store, &scope_node)
                            .as_ref()
                            .is_some_and(|parent| ast::is_class_like(store, *parent))
                        {
                            Some(scope_node)
                        } else {
                            None
                        },
                    );
                    if let Some(this_type) = this_type {
                        if !is_probably_global_type(this_type, file, type_checker) {
                            for symbol in get_properties_for_completion(this_type, type_checker) {
                                symbols.push(symbol);
                                symbol_to_origin_info_map.insert(
                                    symbols.len() - 1,
                                    SymbolOriginInfo {
                                        kind: SYMBOL_ORIGIN_INFO_KIND_THIS_TYPE,
                                        data: None,
                                        ..Default::default()
                                    },
                                );
                                symbol_to_sort_text_map
                                    .insert(symbol, SORT_TEXT_SUGGESTED_CLASS_MEMBERS.to_string());
                            }
                        }
                    }
                }

                if !for_item_resolve
                    && !tspath::is_dynamic_file_name(&file.file_name())
                    && (import_statement_completion.is_some()
                        || (!preferences
                            .include_completions_for_module_exports
                            .is_false()))
                {
                    let mut lower_case_token_text = String::new();
                    let mut usage_position = self.create_lsp_position(position, file);
                    if previous_token
                        .is_some_and(|previous_token| ast::is_identifier(store, previous_token))
                    {
                        let previous_token = previous_token.unwrap();
                        usage_position = self.create_lsp_position(
                            scanner::get_token_pos_of_node(&previous_token, file, false) as i32,
                            file,
                        );
                        if !(Some(previous_token) == context_token
                            && import_statement_completion.is_some())
                        {
                            lower_case_token_text =
                                node_text(store, &previous_token).to_ascii_lowercase();
                        }
                    }
                    if let Some(view) = self.get_prepared_auto_import_view(file)? {
                        auto_imports = view.get_completions(
                            type_checker,
                            &lower_case_token_text,
                            usage_position,
                            is_right_of_open_tag,
                            is_type_only_location,
                        );
                    }
                }
                if is_type_only_location {
                    if context_token.is_some_and(|context_token| {
                        node_parent(store, &context_token)
                            .as_ref()
                            .is_some_and(|parent| ast::is_assertion_expression(store, parent))
                    }) {
                        keyword_filters = KEYWORD_COMPLETION_FILTERS_TYPE_ASSERTION_KEYWORDS;
                    } else {
                        keyword_filters = KEYWORD_COMPLETION_FILTERS_TYPE_KEYWORDS;
                    }
                }

                globals_result = GLOBALS_SEARCH_SUCCESS;
            }

            if globals_result != GLOBALS_SEARCH_SUCCESS {
                if keyword_filters != KEYWORD_COMPLETION_FILTERS_NONE {
                    return Ok(Some(CompletionData::Keyword(keyword_completion_data(
                        keyword_filters,
                        is_js_only_location,
                        is_new_identifier_location,
                    ))));
                }
                return Ok(None);
            }
        }

        let mut contextual_type_or_constraint = None;
        if let Some(previous_token) = previous_token {
            contextual_type_or_constraint =
                get_contextual_type(&previous_token, position, file, type_checker);
            if contextual_type_or_constraint.is_none() {
                contextual_type_or_constraint = get_constraint_of_type_argument_property(
                    store,
                    Some(&previous_token),
                    type_checker,
                );
            }
        } else if let Some(token) = context_token_info
            && token.node.is_none()
            && token.kind == ast::Kind::ColonToken
            && let Some(parent) = token.parent
        {
            contextual_type_or_constraint = match store.kind(parent) {
                ast::Kind::ConditionalExpression => get_contextual_type_for_conditional_expression(
                    &parent,
                    position,
                    file,
                    type_checker,
                ),
                ast::Kind::PropertyAssignment => {
                    let by_object_property = node_parent(store, &parent).and_then(|object| {
                        let object_type = type_checker
                            .get_contextual_type_public(object, checker::CONTEXT_FLAGS_NONE)?;
                        let name = node_name(store, &parent)?;
                        let (name, ok) = ast::try_get_text_of_property_name(store, name);
                        ok.then_some(name).and_then(|name| {
                            type_checker
                                .get_type_of_property_of_contextual_type_public(object_type, &name)
                        })
                    });
                    by_object_property.or_else(|| {
                        type_checker.get_contextual_type_for_object_literal_element_public(
                            parent,
                            checker::CONTEXT_FLAGS_NONE,
                        )
                    })
                }
                _ => None,
            };
        }

        // exclude literal suggestions after <input type="text" [||] /> microsoft/TypeScript#51667) and after closing quote (microsoft/TypeScript#52675)
        // for strings getStringLiteralCompletions handles completions
        let is_literal_expected = !(previous_token
            .is_some_and(|previous_token| ast::is_string_literal_like(store, previous_token)))
            && !is_jsx_identifier_expected;
        let mut literals = Vec::new();
        if is_literal_expected {
            let contextual_type_or_constraint =
                contextual_type_or_constraint.map(|t| skip_constraint(t, type_checker));
            let types = if contextual_type_or_constraint
                .is_some_and(|t| type_checker.is_union_type_public(t))
            {
                type_checker.type_types_public(contextual_type_or_constraint.unwrap())
            } else if let Some(contextual_type_or_constraint) = contextual_type_or_constraint {
                vec![contextual_type_or_constraint]
            } else {
                Vec::new()
            };
            literals = types
                .into_iter()
                .filter_map(|t| {
                    if is_literal(type_checker, t) && !type_checker.is_enum_literal_type_public(t) {
                        match type_checker.literal_value_public(t) {
                            checker::LiteralValue::String(value) => {
                                Some(LiteralValue::String(value))
                            }
                            checker::LiteralValue::Number(value) => {
                                Some(LiteralValue::Number(value))
                            }
                            checker::LiteralValue::PseudoBigInt(value)
                            | checker::LiteralValue::BigInt(value) => {
                                Some(LiteralValue::PseudoBigInt(value.clone()))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
                .collect();
        }

        let mut recommended_completion = None;
        if previous_token.is_some() && contextual_type_or_constraint.is_some() {
            recommended_completion = get_recommended_completion(
                store,
                &previous_token.unwrap(),
                contextual_type_or_constraint.unwrap(),
                type_checker,
            );
        }

        if default_commit_characters.is_empty() {
            default_commit_characters = get_default_commit_characters(is_new_identifier_location);
        }

        Ok(Some(CompletionData::Data(CompletionDataData {
            symbols,
            auto_imports,
            completion_kind,
            is_in_snippet_scope,
            property_access_to_convert,
            is_new_identifier_location,
            location: Some(location),
            keyword_filters,
            literals,
            symbol_to_origin_info_map,
            symbol_to_sort_text_map,
            recommended_completion,
            previous_token,
            context_token,
            jsx_initializer,
            is_type_only_location,
            is_jsx_identifier_expected,
            is_right_of_open_tag,
            is_right_of_dot_or_question_dot: is_right_of_dot || is_right_of_question_dot,
            import_statement_completion,
            has_unresolved_auto_imports,
            default_commit_characters,
            _marker: std::marker::PhantomData,
        })))
    }

    pub fn completion_info_from_data<'a>(
        &'a self,
        ctx: &core::Context,
        type_checker: &mut checker::Checker<'a, '_>,
        file: &'a ast::SourceFile,
        compiler_options: &core::CompilerOptions,
        mut data: CompletionDataData<'a>,
        position: i32,
        optional_replacement_span: Option<lsproto::Range>,
    ) -> Result<lsproto::CompletionList, core::Error> {
        let store = file.store();
        let keyword_filters = data.keyword_filters;
        let is_new_identifier_location = data.is_new_identifier_location;
        let context_token = data.context_token;
        let mut literals = data.literals.clone();
        let preferences = self.user_preferences();
        // Verify if the file is JSX language variant
        if file.language_variant() == core::LanguageVariant::JSX {
            if let Some(location) = data.location.as_ref() {
                if let Some(list) =
                    self.get_jsx_closing_tag_completion(ctx, location, file, position)
                {
                    return Ok(list);
                }
            }
        }

        // When the completion is for the expression of a case clause (e.g. `case |`),
        // filter literals & enum symbols whose values are already present in existing case clauses.
        if let Some(context_token) = context_token {
            let case_clause = ast::find_ancestor(store, Some(context_token), ast::is_case_clause);
            if let Some(case_clause) = case_clause {
                if store.kind(context_token) == ast::Kind::CaseKeyword
                    || node_expression(store, case_clause)
                        .as_ref()
                        .is_some_and(|expression| {
                            ast::is_node_descendant_of(
                                store,
                                Some(context_token),
                                Some(*expression),
                            )
                        })
                {
                    let case_block = node_parent(store, case_clause).unwrap();
                    let case_block_clauses: Vec<ast::Node> =
                        store.clauses(case_block).unwrap().iter().collect();
                    let tracker =
                        new_case_clause_tracker(store, type_checker, case_block_clauses.as_slice());
                    literals.retain(|literal| {
                        let value = match literal {
                            LiteralValue::String(value) => TrackerHasValue::String(value.clone()),
                            LiteralValue::Number(value) => TrackerHasValue::Number(value.clone()),
                            LiteralValue::PseudoBigInt(value) => {
                                TrackerHasValue::BigInt(value.clone())
                            }
                        };
                        !tracker.has_value(&value)
                    });
                    data.symbols.retain(|symbol| {
                        let value_declaration =
                            completion_symbol_value_declaration(type_checker, *symbol);
                        if let Some(value_declaration) = value_declaration
                            && ast::is_enum_member(store, value_declaration)
                        {
                            match type_checker.get_enum_member_value_public(value_declaration) {
                                evaluator::Value::String(value) => {
                                    if tracker.has_value(&TrackerHasValue::String(value)) {
                                        return false;
                                    }
                                }
                                evaluator::Value::Number(value) => {
                                    if tracker.has_value(&TrackerHasValue::Number(value)) {
                                        return false;
                                    }
                                }
                                _ => {}
                            }
                        }
                        true
                    });
                }
            }
        }

        let is_checked = is_checked_file(file, compiler_options);
        if is_checked
            && !is_new_identifier_location
            && data.symbols.is_empty()
            && keyword_filters == KEYWORD_COMPLETION_FILTERS_NONE
        {
            return Ok(lsproto::CompletionList::default());
        }

        let (mut unique_names, mut sorted_entries) = self.get_completion_entries_from_symbols(
            ctx,
            type_checker,
            &data,
            None, /*replacementToken*/
            position,
            file,
            compiler_options,
        );

        if data.keyword_filters != KEYWORD_COMPLETION_FILTERS_NONE {
            let keyword_completions =
                get_keyword_completions(data.keyword_filters, ast::is_source_file_js(file));
            for keyword_entry in keyword_completions {
                if data.is_type_only_location
                    && is_type_keyword(scanner::string_to_token(&keyword_entry.label))
                    || !data.is_type_only_location
                        && is_contextual_keyword_in_auto_importable_expression_space(
                            &keyword_entry.label,
                        )
                    || !unique_names.has(&keyword_entry.label)
                {
                    unique_names.add(keyword_entry.label.clone());
                    sorted_entries.push(keyword_entry);
                }
            }
        }

        for keyword_entry in get_contextual_keywords(file, context_token.as_ref(), position) {
            if !unique_names.has(&keyword_entry.label) {
                unique_names.add(keyword_entry.label.clone());
                sorted_entries.push(keyword_entry);
            }
        }

        for literal in literals {
            let literal_entry =
                create_completion_item_for_literal(file, preferences.clone(), &literal);
            unique_names.add(literal_entry.label.clone());
            sorted_entries.push(literal_entry);
        }

        if !is_checked {
            sorted_entries = self.get_js_completion_entries(
                ctx,
                file,
                position,
                &mut unique_names,
                sorted_entries,
            );
        }

        if let Some(context_token) = context_token {
            if !data.is_right_of_open_tag && !data.is_right_of_dot_or_question_dot {
                if let Some(case_block) =
                    ast::find_ancestor_kind(store, Some(context_token), ast::Kind::CaseBlock)
                {
                    let cases_item = self.get_exhaustive_case_snippets(
                        ctx,
                        case_block,
                        file,
                        position,
                        compiler_options,
                        self.program.as_ref().unwrap(),
                        type_checker,
                    )?;
                    if let Some(cases_item) = cases_item {
                        sorted_entries.push(cases_item);
                    }
                }
            }
        }

        let item_defaults = self.set_item_defaults(
            ctx,
            position,
            file,
            &mut sorted_entries,
            Some(&data.default_commit_characters),
            optional_replacement_span,
        );

        Ok(lsproto::CompletionList {
            is_incomplete: data.has_unresolved_auto_imports,
            item_defaults,
            apply_kind: None,
            items: sorted_entries,
        })
    }

    pub fn get_completion_entries_from_symbols<'a>(
        &self,
        ctx: &core::Context,
        type_checker: &mut checker::Checker<'a, '_>,
        data: &CompletionDataData<'a>,
        replacement_token: Option<&ast::Node>,
        position: i32,
        file: &'a ast::SourceFile,
        compiler_options: &core::CompilerOptions,
    ) -> (collections::Set<String>, Vec<lsproto::CompletionItem>) {
        let _store = file.store();
        let closest_symbol_declaration = get_closest_symbol_declaration(
            file.store(),
            data.context_token.as_ref(),
            data.location.as_ref().unwrap(),
        );
        let use_semicolons = lsutil::probably_uses_semicolons(file);
        let is_member_completion = is_member_completion_kind(data.completion_kind);
        let mut sorted_entries = Vec::with_capacity(data.symbols.len() + data.auto_imports.len());
        // Tracks unique names.
        // Value is set to false for global variables or completions from external module exports, because we can have multiple of those;
        // true otherwise. Based on the order we add things we will always see locals first, then globals, then module exports.
        // So adding a completion for a local will prevent us from adding completions for external module exports sharing the same name.
        let mut uniques: HashMap<String, bool> = HashMap::new();
        for (index, symbol) in data.symbols.iter().enumerate() {
            let origin = data.symbol_to_origin_info_map.get(&index);
            let (name, needs_convert_property_access) =
                get_completion_entry_display_name_for_symbol(
                    file.store(),
                    type_checker,
                    *symbol,
                    origin,
                    data.completion_kind,
                    data.is_jsx_identifier_expected,
                );
            if name.is_empty()
                || uniques.get(&name).copied().unwrap_or(false)
                    && !origin_is_object_literal_method(origin)
                || data.completion_kind == COMPLETION_KIND_GLOBAL
                    && !should_include_symbol(
                        *symbol,
                        data,
                        closest_symbol_declaration,
                        file,
                        type_checker,
                        compiler_options,
                    )
            {
                continue;
            }

            // When in a value location in a JS file, ignore symbols that definitely seem to be type-only.
            if !data.is_type_only_location
                && ast::is_source_file_js(file)
                && symbol_appears_to_be_type_only(*symbol, type_checker)
            {
                continue;
            }

            let mut original_sort_text = data
                .symbol_to_sort_text_map
                .get(symbol)
                .cloned()
                .unwrap_or_default();
            if original_sort_text.is_empty() {
                original_sort_text = SORT_TEXT_LOCATION_PRIORITY.to_string();
            }

            let sort_text = if is_deprecated(*symbol, type_checker) {
                deprecate_sort_text(original_sort_text)
            } else {
                original_sort_text
            };
            let entry = self.create_completion_item(
                ctx,
                type_checker,
                *symbol,
                sort_text,
                replacement_token,
                data,
                position,
                file,
                name.clone(),
                needs_convert_property_access,
                origin,
                use_semicolons,
                compiler_options,
                is_member_completion,
            );
            let Some(entry) = entry else {
                continue;
            };

            // True for locals; false for globals, module exports from other files, `this.` completions.
            let should_shadow_later_symbols = (origin.is_none()
                || origin_is_type_only_alias(origin))
                && !(type_checker.symbol_parent_public(*symbol).is_none()
                    && !completion_symbol_declarations(type_checker, *symbol)
                        .iter()
                        .any(|d| {
                            d.store_id() == file.store().store_id()
                                && ast::get_source_file_of_node(file.store(), Some(*d))
                                    .is_some_and(|source_file| source_file == file.as_node())
                        }));
            uniques.insert(name, should_shadow_later_symbols);
            sorted_entries.push(entry);
        }

        for auto_import in &data.auto_imports {
            let Some(export) = auto_import.export.as_ref() else {
                continue;
            };
            let export_name = export.name().to_string();
            // !!! check for type-only in JS
            // !!! deprecation

            if data.import_statement_completion.is_some() {
                // !!!
                continue;
            }

            // Non-contextual keywords (e.g., `function`, `class`, `const`) cannot be used as identifiers,
            // so auto-imports with these names should not shadow keyword completions.
            let token = scanner::string_to_token(&export_name);
            if token != ast::Kind::Unknown && ast::is_non_contextual_keyword(token) {
                continue;
            }

            if !export.is_unresolved_alias() {
                if data.is_type_only_location {
                    if export.flags & ast::SYMBOL_FLAGS_TYPE == ast::SYMBOL_FLAGS_NONE
                        && export.flags & ast::SYMBOL_FLAGS_MODULE == ast::SYMBOL_FLAGS_NONE
                    {
                        continue;
                    }
                } else if export.flags & ast::SYMBOL_FLAGS_VALUE == ast::SYMBOL_FLAGS_NONE {
                    continue;
                }
            }

            let entry = self.create_lsp_completion_item(
                ctx,
                export_name.clone(),
                String::new(),
                String::new(),
                SORT_TEXT_AUTO_IMPORT_SUGGESTIONS.to_string(),
                export.script_element_kind,
                export.script_element_kind_modifiers,
                None,
                None,
                Some(lsproto::CompletionItemLabelDetails {
                    description: Some(auto_import.fix.module_specifier().to_string()),
                    ..Default::default()
                }),
                file,
                position,
                false, /*isMemberCompletion*/
                false, /*isSnippet*/
                true,  /*hasAction*/
                false, /*preselect*/
                auto_import.fix.module_specifier().to_string(),
                auto_import.fix.auto_import_fix.clone(),
                None, /*detail*/
            );

            if !uniques.get(&export_name).copied().unwrap_or(false) {
                uniques.insert(export_name, false);
                sorted_entries.push(entry);
            }
        }

        let mut unique_set = collections::Set::new();
        for name in uniques.keys() {
            unique_set.add(name.clone());
        }
        (unique_set, sorted_entries)
    }

    pub(crate) fn create_completion_item<'a>(
        &self,
        ctx: &core::Context,
        type_checker: &mut checker::Checker<'a, '_>,
        symbol: CompletionSymbol,
        mut sort_text: SortText,
        replacement_token: Option<&ast::Node>,
        data: &CompletionDataData<'a>,
        position: i32,
        file: &'a ast::SourceFile,
        mut name: String,
        needs_convert_property_access: bool,
        origin: Option<&SymbolOriginInfo>,
        _use_semicolons: bool,
        _compiler_options: &core::CompilerOptions,
        is_member_completion: bool,
    ) -> Option<lsproto::CompletionItem> {
        let store = file.store();
        let context_token = data.context_token;
        let mut insert_text = String::new();
        let filter_text = String::new();
        let mut replacement_span =
            self.get_replacement_range_for_context_token(file, replacement_token, position);
        let mut is_snippet = false;
        let mut has_action = false;
        let mut source = get_source_from_origin(origin);
        let mut label_details = None;
        let preferences = self.user_preferences();
        let insert_question_dot = origin_is_nullable_member(origin);
        let use_braces = origin_is_symbol_member(origin) || needs_convert_property_access;
        if origin_is_this_type_node(origin) {
            if needs_convert_property_access {
                insert_text = format!(
                    "this{}[{}]",
                    if insert_question_dot { "?." } else { "" },
                    quote_property_name(file, preferences.clone(), &name),
                );
            } else {
                insert_text = format!(
                    "this{}{}",
                    if insert_question_dot { "?." } else { "." },
                    name,
                );
            }
        } else if data.property_access_to_convert.is_some() && (use_braces || insert_question_dot) {
            // We should only have needsConvertPropertyAccess if there's a property access to convert. But see microsoft/TypeScript#21790.
            // Somehow there was a global with a non-identifier name. Hopefully someone will complain about getting a "foo bar" global completion and provide a repro.
            let property_access_to_convert = data.property_access_to_convert.unwrap();
            if use_braces {
                if needs_convert_property_access {
                    insert_text = format!(
                        "[{}]",
                        quote_property_name(file, preferences.clone(), &name)
                    );
                } else {
                    insert_text = format!("[{}]", name);
                }
            } else {
                insert_text = name.clone();
            }

            if insert_question_dot
                || node_question_dot_token(store, property_access_to_convert).is_some()
            {
                insert_text = "?.".to_string() + &insert_text;
            }

            let mut dot = astnav::find_child_of_kind_info(
                property_access_to_convert,
                ast::Kind::DotToken,
                file,
            );
            if dot.is_none() {
                dot = astnav::find_child_of_kind_info(
                    property_access_to_convert,
                    ast::Kind::QuestionDotToken,
                    file,
                );
            }

            let dot = dot?;

            // If the text after the '.' starts with this name, write over it. Else, add new text.
            let property_access_name = node_name(store, property_access_to_convert).unwrap();
            let end = if name.starts_with(&node_text(store, property_access_name)) {
                store.loc(property_access_to_convert).end()
            } else {
                dot.loc.end()
            };
            replacement_span = Some(self.create_lsp_range_from_bounds(
                astnav::get_start_of_token_info(dot, file),
                end,
                file,
            ));
        }

        if data.jsx_initializer.is_initializer {
            if insert_text.is_empty() {
                insert_text = name.clone();
            }
            insert_text = format!("{{{}}}", insert_text);
            if let Some(initializer) = data.jsx_initializer.initializer {
                replacement_span = Some(self.create_lsp_range_from_node(initializer, file));
            }
        }

        if origin_is_promise(origin) && data.property_access_to_convert.is_some() {
            if insert_text.is_empty() {
                insert_text = name.clone();
            }
            let property_access_to_convert = data.property_access_to_convert.unwrap();
            let preceding_token =
                astnav::find_preceding_token(file, store.loc(property_access_to_convert).pos());
            let mut await_text = String::new();
            if preceding_token.is_some_and(|preceding_token| {
                lsutil::position_is_asi_candidate(
                    store.loc(preceding_token).end(),
                    node_parent(store, preceding_token).unwrap(),
                    file,
                )
            }) {
                await_text = ";".to_string();
            }

            await_text += &format!(
                "(await {})",
                scanner::get_text_of_node(
                    file,
                    &node_expression(store, property_access_to_convert).unwrap()
                )
            );
            if needs_convert_property_access {
                insert_text = await_text + &insert_text;
            } else {
                let dot_str = if insert_question_dot { "?." } else { "." };
                insert_text = await_text + dot_str + &insert_text;
            }
            let is_in_await_expression = node_parent(store, property_access_to_convert)
                .as_ref()
                .is_some_and(|parent| ast::is_await_expression(store, *parent));
            let expression_storage;
            let wrap_node = if is_in_await_expression {
                node_parent(store, property_access_to_convert).unwrap()
            } else {
                expression_storage = node_expression(store, property_access_to_convert).unwrap();
                expression_storage
            };
            replacement_span = Some(self.create_lsp_range_from_bounds(
                astnav::get_start_of_node(wrap_node, file),
                store.loc(property_access_to_convert).end(),
                file,
            ));
        }

        if origin_is_type_only_alias(origin) {
            has_action = true;
        }

        // Provide object member completions when missing commas, and insert missing commas.
        // For example:
        //
        //    interface I {
        //        a: string;
        //        b: number
        //     }
        //
        //     const cc: I = { a: "red" | }
        //
        // Completion should add a comma after "red" and provide completions for b
        if data.completion_kind == COMPLETION_KIND_OBJECT_PROPERTY_DECLARATION {
            if let Some(context_token) = context_token {
                let preceding_token = astnav::find_preceding_token_ex(
                    file,
                    store.loc(context_token).pos(),
                    Some(context_token),
                );
                if !ast::node_has_kind(store, preceding_token, ast::Kind::CommaToken) {
                    let context_parent = node_parent(store, &context_token);
                    let context_grandparent = context_parent
                        .as_ref()
                        .and_then(|parent| node_parent(store, parent));
                    let property_assignment =
                        ast::find_ancestor(store, context_parent, ast::is_property_assignment);
                    let is_last_token_of_property_assignment = property_assignment
                        .as_ref()
                        .is_some_and(|property_assignment| {
                            lsutil::get_last_token_info(Some(*property_assignment), file)
                                .is_some_and(|last| last.matches_node(store, context_token))
                        });
                    if context_grandparent
                        .as_ref()
                        .is_some_and(|parent| ast::is_method_declaration(store, *parent))
                        || context_grandparent
                            .as_ref()
                            .is_some_and(|parent| ast::is_get_accessor_declaration(store, *parent))
                        || context_grandparent
                            .as_ref()
                            .is_some_and(|parent| ast::is_set_accessor_declaration(store, *parent))
                        || context_parent
                            .as_ref()
                            .is_some_and(|parent| ast::is_spread_assignment(store, *parent))
                        || is_last_token_of_property_assignment
                        || context_parent.as_ref().is_some_and(|parent| {
                            ast::is_shorthand_property_assignment(store, *parent)
                        }) && get_line_of_position(file, store.loc(context_token).end())
                            != get_line_of_position(file, position)
                    {
                        source = COMPLETION_SOURCE_OBJECT_LITERAL_MEMBER_WITH_COMMA.to_string();
                        has_action = true;
                    }
                }
            }
        }

        if preferences
            .clone()
            .include_completions_with_class_member_snippets
            .is_true()
            && data.completion_kind == COMPLETION_KIND_MEMBER_LIKE
            && is_class_like_member_completion(symbol, data.location.as_ref().unwrap(), file)
        {
            // !!! class member completions
        }

        if origin_is_object_literal_method(origin) {
            let object_literal_method = origin.unwrap().as_object_literal_method();
            insert_text = object_literal_method.insert_text.clone();
            is_snippet = object_literal_method.is_snippet;
            label_details = object_literal_method.label_details.clone(); // !!! check if this can conflict with case above where we set label details
            if !client_supports_item_label_details(ctx) {
                if let Some(details) = &object_literal_method.label_details {
                    if let Some(detail) = &details.detail {
                        name += detail;
                    }
                }
                label_details = None;
            }
            source = COMPLETION_SOURCE_OBJECT_LITERAL_METHOD_SNIPPET.to_string();
            sort_text = sort_below(sort_text);
        }

        if data.is_jsx_identifier_expected
            && !data.is_right_of_open_tag
            && client_supports_item_snippet(ctx)
            && preferences.clone().jsx_attribute_completion_style
                != lsutil::JsxAttributeCompletionStyle::None
            && !(node_parent(store, data.location.as_ref().unwrap()).is_some_and(|parent| {
                ast::is_jsx_attribute(store, parent) && node_initializer(store, parent).is_some()
            }))
        {
            let mut use_braces = preferences.clone().jsx_attribute_completion_style
                == lsutil::JsxAttributeCompletionStyle::Braces;
            let t = type_checker
                .get_type_of_symbol_identity_at_location_public(symbol, data.location)
                .unwrap_or_else(|| type_checker.get_error_type());

            // If is boolean like or undefined, don't return a snippet, we want to return just the completion.
            if preferences.clone().jsx_attribute_completion_style
                == lsutil::JsxAttributeCompletionStyle::Auto
                && !type_checker.is_boolean_like_type_public(t)
                && !(type_checker.is_union_type_public(t)
                    && type_checker
                        .type_types_public(t)
                        .iter()
                        .any(|t| type_checker.is_boolean_like_type_public(*t)))
            {
                if type_checker.is_string_like_type_public(t)
                    || type_checker.is_union_type_public(t)
                        && type_checker.type_types_public(t).iter().all(|t| {
                            type_checker.type_flags_public(*t)
                                & (checker::TYPE_FLAGS_STRING_LIKE | checker::TYPE_FLAGS_UNDEFINED)
                                != checker::TYPE_FLAGS_NONE
                                || is_string_and_empty_anonymous_object_intersection(
                                    type_checker,
                                    *t,
                                )
                        })
                {
                    // If type is string-like or undefined, use quotes.
                    insert_text = format!(
                        "{}={}",
                        escape_snippet_text(&name),
                        quote(file, preferences.clone(), "$1")
                    );
                    is_snippet = true;
                } else {
                    // Use braces for everything else.
                    use_braces = true;
                }
            }

            if use_braces {
                insert_text = escape_snippet_text(&name) + "={$1}";
                is_snippet = true;
            }
        }

        let parent_named_import_or_export = data.location.as_ref().and_then(|location| {
            ast::find_ancestor(store, Some(*location), is_named_imports_or_exports)
        });
        if let Some(parent_named_import_or_export) = parent_named_import_or_export {
            if !scanner::is_identifier_text(&name, core::LanguageVariant::Standard) {
                insert_text = quote_property_name(file, preferences.clone(), &name);

                if store.kind(parent_named_import_or_export) == ast::Kind::NamedImports {
                    // Check if it is `import { ^here as name } from '...'``.
                    // We have to access the scanner here to check if it is `{ ^here as name }`` or `{ ^here, as, name }`.
                    let mut scanner = scanner::Scanner::new(
                        file.text().to_string(),
                        core::ScriptTarget::default(),
                    );
                    scanner.reset_pos(position);
                    if !(scanner.scan() == ast::Kind::AsKeyword
                        && scanner.scan() == ast::Kind::Identifier)
                    {
                        insert_text +=
                            &format!(" as {}", generate_identifier_for_arbitrary_string(&name));
                    }
                }
            } else if store.kind(parent_named_import_or_export) == ast::Kind::NamedImports {
                let possible_token = scanner::string_to_token(&name);
                if possible_token != ast::Kind::Unknown
                    && (possible_token == ast::Kind::AwaitKeyword
                        || lsutil::is_non_contextual_keyword(possible_token))
                {
                    insert_text = format!("{} as {}_", name, name);
                }
            }
        }

        // Commit characters

        let element_kind = get_completion_symbol_kind(
            store,
            type_checker,
            symbol,
            data.location.as_ref().unwrap(),
        );
        let mut commit_characters = None;
        if client_supports_item_commit_characters(ctx) {
            if element_kind == lsutil::ScriptElementKind::Warning
                || element_kind == lsutil::ScriptElementKind::String
            {
                commit_characters = Some(Vec::new());
            } else if !client_supports_default_commit_characters(ctx) {
                commit_characters = Some(data.default_commit_characters.clone());
            }
            // Otherwise use the completion list default.
        }

        let preselect = is_recommended_completion_match(
            Some(symbol),
            data.recommended_completion.clone(),
            type_checker,
        );
        let kind_modifiers = get_completion_symbol_modifiers(type_checker, symbol);

        Some(self.create_lsp_completion_item(
            ctx,
            name,
            insert_text,
            filter_text,
            sort_text,
            element_kind,
            kind_modifiers,
            replacement_span,
            commit_characters,
            label_details,
            file,
            position,
            is_member_completion,
            is_snippet,
            has_action,
            preselect,
            source,
            None, /*autoImportFix*/
            None, /*detail*/
        ))
    }

    pub fn get_completions_at_position<'a>(
        &self,
        ctx: core::Context,
        file: &'a ast::SourceFile,
        position: i32,
        trigger_character: Option<&String>,
    ) -> Result<Option<lsproto::CompletionList>, core::Error> {
        let (_, previous_token) = get_relevant_tokens(position, file);
        if let Some(trigger_character) = trigger_character {
            if !is_in_string(file, position, previous_token.as_ref())
                && !is_valid_trigger(file, trigger_character, previous_token.as_ref(), position)
            {
                return Ok(None);
            }
        }

        if trigger_character.is_some_and(|trigger_character| trigger_character == " ") {
            // `isValidTrigger` ensures we are at `import |`
            if self
                .user_preferences()
                .include_completions_for_import_statements
                .is_true()
            {
                return Ok(Some(lsproto::CompletionList {
                    is_incomplete: true,
                    ..Default::default()
                }));
            }
            return Ok(None);
        }

        let compiler_options = self.get_program().options();

        // !!! see if incomplete completion list and continue or clean

        self.get_program().with_type_checker_for_file_using(
            compiler::CheckerAccess::context(&ctx),
            file,
            |checker| {
                if let Some(previous_token) = previous_token.as_ref() {
                    // lookup is sequential with the later completion-data lookup.
                    let string_completions = self.get_string_literal_completions(
                        &ctx,
                        file,
                        position,
                        *previous_token,
                        checker,
                        compiler_options,
                    );
                    if let Some(string_completions) = string_completions {
                        return Ok(Some(string_completions));
                    }
                }

                if let Some(previous_token) = previous_token {
                    let previous_parent = node_parent(file.store(), &previous_token);
                    if (file.store().kind(previous_token) == ast::Kind::BreakKeyword
                        || file.store().kind(previous_token) == ast::Kind::ContinueKeyword
                        || file.store().kind(previous_token) == ast::Kind::Identifier)
                        && previous_parent.as_ref().is_some_and(|parent| {
                            ast::is_break_or_continue_statement(file.store(), parent)
                        })
                    {
                        let result = self.get_label_completions_at_position(
                            &ctx,
                            previous_parent.as_ref().unwrap(),
                            file,
                            position,
                            self.get_optional_replacement_span(Some(&previous_token), file),
                        );
                        return Ok(result);
                    }
                }

                let preferences = self.user_preferences();
                let data = self.get_completion_data(
                    &ctx,
                    checker,
                    file,
                    position,
                    preferences.clone(),
                    false, /*forItemResolve*/
                )?;
                let Some(data) = data else {
                    return Ok(None);
                };

                let result = match data {
                    CompletionData::Data(data) => {
                        let optional_replacement_span =
                            self.get_optional_replacement_span(data.location.as_ref(), file);
                        self.completion_info_from_data(
                            &ctx,
                            checker,
                            file,
                            compiler_options,
                            data,
                            position,
                            optional_replacement_span,
                        )?
                    }
                    CompletionData::Keyword(data) => {
                        let optional_replacement_span = previous_token.as_ref().and_then(|token| {
                            self.get_optional_replacement_span(Some(token), file)
                        });
                        self.specific_keyword_completion_info(
                            &ctx,
                            position,
                            file,
                            data.keyword_completions,
                            data.is_new_identifier_location,
                            optional_replacement_span,
                        )
                    }
                };
                Ok(Some(result))
            },
        )
    }

    pub fn provide_completion(
        &self,
        ctx: core::Context,
        document_uri: lsproto::DocumentUri,
        lsp_position: lsproto::Position,
        context: Option<&lsproto::CompletionContext>,
    ) -> Result<lsproto::CompletionResponse, core::Error> {
        let (_, file) = self.get_program_and_file(document_uri);
        let mut trigger_character = None;
        if let Some(context) = context {
            trigger_character = context.trigger_character.as_ref();
        }
        let position = self
            .converters
            .line_and_character_to_position(file, lsp_position) as i32;
        let completion_list =
            self.get_completions_at_position(ctx, file, position, trigger_character)?;
        let completion_list = ensure_item_data(&file.file_name(), position, completion_list);
        Ok(lsproto::CompletionItemsOrListOrNull {
            list: completion_list,
            ..Default::default()
        })
    }
}

pub fn get_contextual_keywords(
    file: &ast::SourceFile,
    context_token: Option<&ast::Node>,
    position: i32,
) -> Vec<lsproto::CompletionItem> {
    let store = file.store();
    let mut entries = Vec::new();
    // An `AssertClause` can come after an import declaration:
    //  import * from "foo" |
    //  import "foo" |
    // or after a re-export declaration that has a module specifier:
    //  export { foo } from "foo" |
    // Source: https://tc39.es/proposal-import-assertions/
    if let Some(context_token) = context_token {
        let parent = node_parent(store, context_token);
        let token_line = scanner::get_ecma_line_of_position(file, store.loc(*context_token).end());
        let current_line = scanner::get_ecma_line_of_position(file, position);
        if (parent
            .as_ref()
            .is_some_and(|parent| ast::is_import_declaration(store, *parent))
            || parent
                .as_ref()
                .is_some_and(|parent| ast::is_export_declaration(store, *parent))
                && parent
                    .as_ref()
                    .and_then(|parent| node_module_specifier(store, parent))
                    .is_some())
            && node_module_specifier(store, parent.unwrap())
                .as_ref()
                .is_some_and(|module_specifier| *context_token == *module_specifier)
            && token_line == current_line
        {
            entries.push(lsproto::CompletionItem {
                label: scanner::token_to_string(ast::Kind::AssertKeyword),
                kind: Some(lsproto::CompletionItemKind::KEYWORD),
                sort_text: Some(SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string()),
                ..Default::default()
            });
        }
    }
    entries
}

fn completion_literal_is_name(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_declaration_name(store, &node)
        || store
            .parent(node)
            .is_some_and(|parent| store.kind(parent) == ast::Kind::ExternalModuleReference)
        || ast::is_argument_expression_of_element_access(store, node)
        || ast::is_literal_computed_property_declaration_name(store, node)
}

fn compute_name_table(file: &ast::SourceFile) -> HashMap<String, i32> {
    let store = file.store();
    let mut name_table = HashMap::new();

    fn walk(
        store: &ast::AstStore,
        file: &ast::SourceFile,
        node: ast::Node,
        name_table: &mut HashMap<String, i32>,
    ) {
        if (ast::is_identifier(store, node)
            && !ast::is_jsx_tag_name(store, node)
            && !store.text(node).is_empty())
            || (ast::is_string_or_numeric_literal_like(store, node)
                && completion_literal_is_name(store, node))
            || ast::is_private_identifier(store, node)
        {
            let text = store.text(node);
            if name_table.contains_key(&text) {
                name_table.insert(text, -1);
            } else {
                name_table.insert(text, store.loc(node).pos());
            }
        }

        let _ = store.for_each_present_child(node, |child| {
            walk(store, file, child, name_table);
            std::ops::ControlFlow::Continue(())
        });
    }

    let _ = store.for_each_present_child(file.as_node(), |child| {
        walk(store, file, child, &mut name_table);
        std::ops::ControlFlow::Continue(())
    });

    name_table
}

impl LanguageService<'_> {
    pub fn get_js_completion_entries(
        &self,
        _ctx: &core::Context,
        file: &ast::SourceFile,
        position: i32,
        unique_names: &mut collections::Set<String>,
        mut sorted_entries: Vec<lsproto::CompletionItem>,
    ) -> Vec<lsproto::CompletionItem> {
        let name_table = compute_name_table(file);
        for (name, pos) in name_table {
            // Skip identifiers produced only from the current location
            if pos == position {
                continue;
            }
            if !unique_names.has(&name)
                && scanner::is_identifier_text(&name, core::LanguageVariant::Standard)
            {
                unique_names.add(name.clone());
                sorted_entries.push(lsproto::CompletionItem {
                    label: name,
                    kind: Some(lsproto::CompletionItemKind::TEXT),
                    sort_text: Some(SORT_TEXT_JAVASCRIPT_IDENTIFIERS.to_string()),
                    commit_characters: Some(Vec::new()),
                    ..Default::default()
                });
            }
        }
        sorted_entries
    }
}

pub fn try_get_containing_jsx_element(
    context_token: Option<&ast::Node>,
    file: &ast::SourceFile,
) -> Option<ast::Node> {
    let context_token = context_token?;
    let store = file.store();

    let parent = node_parent(store, context_token);
    match store.kind(*context_token) {
        ast::Kind::GreaterThanToken
        | ast::Kind::LessThanSlashToken
        | ast::Kind::SlashToken
        | ast::Kind::Identifier
        | ast::Kind::PropertyAccessExpression
        | ast::Kind::JsxAttributes
        | ast::Kind::JsxAttribute
        | ast::Kind::JsxSpreadAttribute => {
            if parent
                .as_ref()
                .is_some_and(|parent| store.kind(*parent) == ast::Kind::JsxSelfClosingElement)
                || parent
                    .as_ref()
                    .is_some_and(|parent| store.kind(*parent) == ast::Kind::JsxOpeningElement)
            {
                if store.kind(*context_token) == ast::Kind::GreaterThanToken {
                    let preceding_token =
                        astnav::find_preceding_token(file, store.loc(*context_token).pos());
                    if node_type_arguments(store, parent.as_ref().unwrap()).is_empty()
                        || preceding_token
                            .is_some_and(|token| store.kind(token) == ast::Kind::SlashToken)
                    {
                        return None;
                    }
                }
                return parent;
            } else if parent
                .as_ref()
                .is_some_and(|parent| store.kind(*parent) == ast::Kind::JsxAttribute)
            {
                // Currently we parse JsxOpeningLikeElement as:
                //      JsxOpeningLikeElement
                //          attributes: JsxAttributes
                //             properties: NodeArray<JsxAttributeLike>
                return parent
                    .and_then(|parent| node_parent(store, parent))
                    .and_then(|parent| node_parent(store, &parent));
            }
        }
        // The context token is the closing } or " of an attribute, which means
        // its parent is a JsxExpression, whose parent is a JsxAttribute,
        // whose parent is a JsxOpeningLikeElement
        ast::Kind::StringLiteral => {
            if parent
                .as_ref()
                .is_some_and(|parent| store.kind(*parent) == ast::Kind::JsxAttribute)
                || parent
                    .as_ref()
                    .is_some_and(|parent| store.kind(*parent) == ast::Kind::JsxSpreadAttribute)
            {
                // Currently we parse JsxOpeningLikeElement as:
                //      JsxOpeningLikeElement
                //          attributes: JsxAttributes
                //             properties: NodeArray<JsxAttributeLike>
                return parent
                    .and_then(|parent| node_parent(store, parent))
                    .and_then(|parent| node_parent(store, &parent));
            }
        }
        ast::Kind::CloseBraceToken => {
            if parent
                .as_ref()
                .is_some_and(|parent| store.kind(*parent) == ast::Kind::JsxExpression)
                && parent
                    .as_ref()
                    .and_then(|parent| node_parent(store, parent))
                    .is_some()
                && parent
                    .as_ref()
                    .and_then(|parent| node_parent(store, parent))
                    .as_ref()
                    .is_some_and(|parent| store.kind(*parent) == ast::Kind::JsxAttribute)
            {
                // Currently we parse JsxOpeningLikeElement as:
                //      JsxOpeningLikeElement
                //          attributes: JsxAttributes
                //             properties: NodeArray<JsxAttributeLike>
                //                  each JsxAttribute can have initializer as JsxExpression
                return parent
                    .and_then(|parent| node_parent(store, parent))
                    .and_then(|parent| node_parent(store, &parent))
                    .and_then(|parent| node_parent(store, &parent));
            }
            if parent
                .as_ref()
                .is_some_and(|parent| store.kind(*parent) == ast::Kind::JsxSpreadAttribute)
            {
                // Currently we parse JsxOpeningLikeElement as:
                //      JsxOpeningLikeElement
                //          attributes: JsxAttributes
                //             properties: NodeArray<JsxAttributeLike>
                return parent
                    .and_then(|parent| node_parent(store, parent))
                    .and_then(|parent| node_parent(store, &parent));
            }
        }
        _ => {}
    }

    None
}

// Filters out completion suggestions from 'symbols' according to existing JSX attributes.
// @returns Symbols to be suggested in a JSX element, barring those whose attributes
// do not occur at the current position and have not otherwise been typed.
pub(crate) fn filter_jsx_attributes<'a>(
    symbols: Vec<CompletionSymbol>,
    attributes: Vec<&ast::JsxAttributeLike>,
    store: &ast::AstStore,
    file: &ast::SourceFile,
    position: i32,
    type_checker: &mut checker::Checker,
) -> (Vec<CompletionSymbol>, collections::Set<String>) {
    let mut existing_names = collections::Set::new();
    let mut members_declared_by_spread_assignment = collections::Set::new();
    for attr in attributes {
        // If this is the item we are editing right now, do not filter it out.
        if is_currently_editing_node(attr, file, position) {
            continue;
        }

        if store.kind(*attr) == ast::Kind::JsxAttribute {
            if let Some(name) = node_name(store, attr) {
                existing_names.add(node_text(store, name));
            }
        } else if ast::is_jsx_spread_attribute(store, *attr) {
            set_member_declared_by_spread_assignment(
                store,
                attr,
                &mut members_declared_by_spread_assignment,
                type_checker,
            );
        }
    }

    (
        symbols
            .into_iter()
            .filter(|a| !existing_names.has(&completion_symbol_name(type_checker, *a)))
            .collect(),
        members_declared_by_spread_assignment,
    )
}

pub fn ensure_item_data(
    file_name: &str,
    pos: i32,
    list: Option<lsproto::CompletionList>,
) -> Option<lsproto::CompletionList> {
    let mut list = list?;
    for item in &mut list.items {
        if item.data.is_none() {
            item.data = Some(lsproto::CompletionItemData {
                file_name: file_name.to_string(),
                position: pos,
                name: item.label.clone(),
                ..Default::default()
            });
        }
    }
    Some(list)
}

pub enum CompletionData<'a> {
    Data(CompletionDataData<'a>),
    Keyword(CompletionDataKeyword),
}

pub struct CompletionDataData<'a> {
    pub(crate) _marker: std::marker::PhantomData<&'a ()>,
    pub(crate) symbols: Vec<CompletionSymbol>,
    pub auto_imports: Vec<autoimport::FixAndExport>,
    pub completion_kind: CompletionKind,
    pub is_in_snippet_scope: bool,
    // Note that the presence of this alone doesn't mean that we need a conversion. Only do that if the completion is not an ordinary identifier.
    pub property_access_to_convert: Option<ast::Node>,
    pub is_new_identifier_location: bool,
    pub location: Option<ast::Node>,
    pub keyword_filters: KeywordCompletionFilters,
    pub literals: Vec<LiteralValue>,
    pub symbol_to_origin_info_map: HashMap<usize, SymbolOriginInfo>,
    pub symbol_to_sort_text_map: HashMap<CompletionSymbol, SortText>,
    pub(crate) recommended_completion: Option<CompletionSymbol>,
    pub previous_token: Option<ast::Node>,
    pub context_token: Option<ast::Node>,
    pub jsx_initializer: JsxInitializer,
    pub is_type_only_location: bool,
    // In JSX tag name and attribute names, identifiers like "my-tag" or "aria-name" is valid identifier.
    pub is_jsx_identifier_expected: bool,
    pub is_right_of_open_tag: bool,
    pub is_right_of_dot_or_question_dot: bool,
    pub import_statement_completion: Option<ImportStatementCompletionInfo>,
    pub has_unresolved_auto_imports: bool,
    // flags CompletionInfoFlags
    pub default_commit_characters: Vec<String>,
}

impl<'a> Default for CompletionDataData<'a> {
    fn default() -> Self {
        Self {
            symbols: Vec::new(),
            _marker: std::marker::PhantomData,
            auto_imports: Vec::new(),
            completion_kind: COMPLETION_KIND_NONE,
            is_in_snippet_scope: false,
            property_access_to_convert: None,
            is_new_identifier_location: false,
            location: None,
            keyword_filters: 0,
            literals: Vec::new(),
            symbol_to_origin_info_map: HashMap::new(),
            symbol_to_sort_text_map: HashMap::new(),
            recommended_completion: None,
            previous_token: None,
            context_token: None,
            jsx_initializer: JsxInitializer::default(),
            is_type_only_location: false,
            is_jsx_identifier_expected: false,
            is_right_of_open_tag: false,
            is_right_of_dot_or_question_dot: false,
            import_statement_completion: None,
            has_unresolved_auto_imports: false,
            default_commit_characters: Vec::new(),
        }
    }
}

pub struct CompletionDataKeyword {
    pub keyword_completions: Vec<lsproto::CompletionItem>,
    pub is_new_identifier_location: bool,
}

pub struct ImportStatementCompletionInfo {
    pub is_keyword_only_completion: bool,
    pub keyword_completion: ast::Kind, // TokenKind
    pub is_new_identifier_location: bool,
    pub is_top_level_type_only: bool,
    pub could_be_type_only_import_specifier: bool,
    pub replacement_span: Option<lsproto::Range>,
}

// If we're after the `=` sign but no identifier has been typed yet,
// value will be `true` but initializer will be `nil`.
#[derive(Clone, Copy, Default)]
pub struct JsxInitializer {
    pub is_initializer: bool,
    pub initializer: Option<ast::IdentifierNode>,
}

pub type KeywordCompletionFilters = i32;

pub const KEYWORD_COMPLETION_FILTERS_NONE: KeywordCompletionFilters = 0; // No keywords
pub const KEYWORD_COMPLETION_FILTERS_ALL: KeywordCompletionFilters = 1; // Every possible kewyord
pub const KEYWORD_COMPLETION_FILTERS_CLASS_ELEMENT_KEYWORDS: KeywordCompletionFilters = 2; // Keywords inside class body
pub const KEYWORD_COMPLETION_FILTERS_INTERFACE_ELEMENT_KEYWORDS: KeywordCompletionFilters = 3; // Keywords inside interface body
pub const KEYWORD_COMPLETION_FILTERS_CONSTRUCTOR_PARAMETER_KEYWORDS: KeywordCompletionFilters = 4; // Keywords at constructor parameter
pub const KEYWORD_COMPLETION_FILTERS_FUNCTION_LIKE_BODY_KEYWORDS: KeywordCompletionFilters = 5; // Keywords at function like body
pub const KEYWORD_COMPLETION_FILTERS_TYPE_ASSERTION_KEYWORDS: KeywordCompletionFilters = 6;
pub const KEYWORD_COMPLETION_FILTERS_TYPE_KEYWORDS: KeywordCompletionFilters = 7;
pub const KEYWORD_COMPLETION_FILTERS_TYPE_KEYWORD: KeywordCompletionFilters = 8; // Literally just `type`
pub const KEYWORD_COMPLETION_FILTERS_LAST: KeywordCompletionFilters =
    KEYWORD_COMPLETION_FILTERS_TYPE_KEYWORD;

pub fn keyword_filters_from_syntax_kind(keyword_completion: ast::Kind) -> KeywordCompletionFilters {
    match keyword_completion {
        ast::Kind::TypeKeyword => KEYWORD_COMPLETION_FILTERS_TYPE_KEYWORD,
        _ => panic!(
            "Unknown mapping from ast.Kind `{}` to KeywordCompletionFilters",
            keyword_completion.to_string()
        ),
    }
}

pub type CompletionKind = i32;

pub const COMPLETION_KIND_NONE: CompletionKind = 0;
pub const COMPLETION_KIND_OBJECT_PROPERTY_DECLARATION: CompletionKind = 1;
pub const COMPLETION_KIND_GLOBAL: CompletionKind = 2;
pub const COMPLETION_KIND_PROPERTY_ACCESS: CompletionKind = 3;
pub const COMPLETION_KIND_MEMBER_LIKE: CompletionKind = 4;
pub const COMPLETION_KIND_STRING: CompletionKind = 5;

pub static TRIGGER_CHARACTERS: &[&str] = &[".", "\"", "'", "`", "/", "@", "<", "#", " "];

// All commit characters, valid when `isNewIdentifierLocation` is false.
pub static ALL_COMMIT_CHARACTERS: &[&str] = &[".", ",", ";"];

// Commit characters valid at expression positions where we could be inside a parameter list.
pub static NO_COMMA_COMMIT_CHARACTERS: &[&str] = &[".", ";"];

pub static EMPTY_COMMIT_CHARACTERS: &[&str] = &[];

pub type SortText = String;

pub const SORT_TEXT_LOCAL_DECLARATION_PRIORITY: &str = "10";
pub const SORT_TEXT_LOCATION_PRIORITY: &str = "11";
#[allow(non_upper_case_globals)]
pub const SortTextLocationPriority: &str = SORT_TEXT_LOCATION_PRIORITY;
pub const SORT_TEXT_OPTIONAL_MEMBER: &str = "12";
pub const SORT_TEXT_MEMBER_DECLARED_BY_SPREAD_ASSIGNMENT: &str = "13";
pub const SORT_TEXT_SUGGESTED_CLASS_MEMBERS: &str = "14";
pub const SORT_TEXT_GLOBALS_OR_KEYWORDS: &str = "15";
pub const SORT_TEXT_AUTO_IMPORT_SUGGESTIONS: &str = "16";
pub const SORT_TEXT_CLASS_MEMBER_SNIPPETS: &str = "17";
pub const SORT_TEXT_JAVASCRIPT_IDENTIFIERS: &str = "18";

pub fn deprecate_sort_text(original: SortText) -> SortText {
    "z".to_string() + &original
}

pub fn sort_below(original: SortText) -> SortText {
    original + "1"
}

pub type SymbolOriginInfoKind = i32;

pub const SYMBOL_ORIGIN_INFO_KIND_THIS_TYPE: SymbolOriginInfoKind = 1 << 0;
pub const SYMBOL_ORIGIN_INFO_KIND_SYMBOL_MEMBER: SymbolOriginInfoKind = 1 << 1;
pub const SYMBOL_ORIGIN_INFO_KIND_PROMISE: SymbolOriginInfoKind = 1 << 2;
pub const SYMBOL_ORIGIN_INFO_KIND_NULLABLE: SymbolOriginInfoKind = 1 << 3;
pub const SYMBOL_ORIGIN_INFO_KIND_TYPE_ONLY_ALIAS: SymbolOriginInfoKind = 1 << 4;
pub const SYMBOL_ORIGIN_INFO_KIND_OBJECT_LITERAL_METHOD: SymbolOriginInfoKind = 1 << 5;
pub const SYMBOL_ORIGIN_INFO_KIND_IGNORE: SymbolOriginInfoKind = 1 << 6;
pub const SYMBOL_ORIGIN_INFO_KIND_COMPUTED_PROPERTY_NAME: SymbolOriginInfoKind = 1 << 7;

#[derive(Clone)]
pub enum SymbolOriginInfoData {
    ObjectLiteralMethod(SymbolOriginInfoObjectLiteralMethod),
    TypeOnlyAlias(SymbolOriginInfoTypeOnlyAlias),
    ComputedPropertyName(SymbolOriginInfoComputedPropertyName),
}

#[derive(Clone)]
pub struct SymbolOriginInfo {
    pub kind: SymbolOriginInfoKind,
    pub is_default_export: bool,
    pub is_from_package_json: bool,
    pub file_name: String,
    pub data: Option<SymbolOriginInfoData>,
}

impl Default for SymbolOriginInfo {
    fn default() -> Self {
        Self {
            kind: SYMBOL_ORIGIN_INFO_KIND_THIS_TYPE,
            is_default_export: false,
            is_from_package_json: false,
            file_name: String::new(),
            data: None,
        }
    }
}

impl SymbolOriginInfo {
    pub fn symbol_name(&self) -> String {
        match self.data.as_ref() {
            Some(SymbolOriginInfoData::ComputedPropertyName(data)) => data.symbol_name.clone(),
            other => panic!(
                "symbolOriginInfo: unknown data type for symbolName(): {:?}",
                other.map(|_| ())
            ),
        }
    }

    pub fn as_object_literal_method(&self) -> &SymbolOriginInfoObjectLiteralMethod {
        match self.data.as_ref() {
            Some(SymbolOriginInfoData::ObjectLiteralMethod(data)) => data,
            _ => panic!("symbolOriginInfo: expected object literal method data"),
        }
    }
}

#[derive(Clone)]
pub struct SymbolOriginInfoObjectLiteralMethod {
    pub insert_text: String,
    pub label_details: Option<lsproto::CompletionItemLabelDetails>,
    pub is_snippet: bool,
}

#[derive(Clone)]
pub struct SymbolOriginInfoTypeOnlyAlias {
    pub declaration: ast::Node,
}

#[derive(Clone)]
pub struct SymbolOriginInfoComputedPropertyName {
    pub symbol_name: String,
}

// Special values for `CompletionInfo['source']` used to disambiguate
// completion items with the same `name`. (Each completion item must
// have a unique name/source combination, because those two fields
// comprise `CompletionEntryIdentifier` in `getCompletionEntryDetails`.
//
// When the completion item is an auto-import suggestion, the source
// is the module specifier of the suggestion. To avoid collisions,
// the values here should not be a module specifier we would ever
// generate for an auto-import.
pub type CompletionSource = String;

// Completions that require `this.` insertion text.
pub const COMPLETION_SOURCE_THIS_PROPERTY: &str = "ThisProperty/";
// Auto-import that comes attached to a class member snippet.
pub const COMPLETION_SOURCE_CLASS_MEMBER_SNIPPET: &str = "ClassMemberSnippet/";
// A type-only import that needs to be promoted in order to be used at the completion location.
pub const COMPLETION_SOURCE_TYPE_ONLY_ALIAS: &str = "TypeOnlyAlias/";
// Auto-import that comes attached to an object literal method snippet.
pub const COMPLETION_SOURCE_OBJECT_LITERAL_METHOD_SNIPPET: &str = "ObjectLiteralMethodSnippet/";
// Case completions for switch statements.
pub const COMPLETION_SOURCE_SWITCH_CASES: &str = "SwitchCases/";
// Completions for an object literal expression.
pub const COMPLETION_SOURCE_OBJECT_LITERAL_MEMBER_WITH_COMMA: &str =
    "ObjectLiteralMemberWithComma/";

// Value is set to false for global variables or completions from external module exports,
// true otherwise.
pub type UniqueNamesMap = HashMap<String, bool>;

// string | jsnum.Number | PseudoBigInt
#[derive(Clone)]
pub enum LiteralValue {
    String(String),
    Number(jsnum::Number),
    PseudoBigInt(jsnum::PseudoBigInt),
}

pub type GlobalsSearch = i32;

pub const GLOBALS_SEARCH_CONTINUE: GlobalsSearch = 0;
pub const GLOBALS_SEARCH_SUCCESS: GlobalsSearch = 1;
pub const GLOBALS_SEARCH_FAIL: GlobalsSearch = 2;

pub fn keyword_completion_data(
    keyword_filters: KeywordCompletionFilters,
    filter_out_ts_only_keywords: bool,
    is_new_identifier_location: bool,
) -> CompletionDataKeyword {
    CompletionDataKeyword {
        keyword_completions: get_keyword_completions(keyword_filters, filter_out_ts_only_keywords),
        is_new_identifier_location,
    }
}

pub fn get_default_commit_characters(is_new_identifier_location: bool) -> Vec<String> {
    if is_new_identifier_location {
        return Vec::new();
    }
    ALL_COMMIT_CHARACTERS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

pub fn completion_name_for_literal(
    file: &ast::SourceFile,
    preferences: lsutil::UserPreferences,
    literal: &LiteralValue,
) -> String {
    match literal {
        LiteralValue::String(value) => quote(file, preferences, value),
        LiteralValue::Number(value) => value.to_string(),
        LiteralValue::PseudoBigInt(value) => value.to_string() + "n",
    }
}

pub fn create_completion_item_for_literal(
    file: &ast::SourceFile,
    preferences: lsutil::UserPreferences,
    literal: &LiteralValue,
) -> lsproto::CompletionItem {
    lsproto::CompletionItem {
        label: completion_name_for_literal(file, preferences, literal),
        kind: Some(lsproto::CompletionItemKind::CONSTANT),
        sort_text: Some(SORT_TEXT_LOCATION_PRIORITY.to_string()),
        commit_characters: Some(Vec::new()),
        ..Default::default()
    }
}

pub(crate) fn is_recommended_completion_match(
    local_symbol: Option<CompletionSymbol>,
    recommended_completion: Option<CompletionSymbol>,
    type_checker: &mut checker::Checker,
) -> bool {
    local_symbol == recommended_completion
        || local_symbol.is_some_and(|local_symbol| {
            if completion_symbol_flags(type_checker, local_symbol) & ast::SYMBOL_FLAGS_EXPORT_VALUE
                == ast::SYMBOL_FLAGS_NONE
            {
                return false;
            }
            type_checker.get_export_symbol_public(local_symbol) == recommended_completion
        })
}

// Ported from vscode.
pub fn word_separators() -> collections::Set<char> {
    collections::new_set_from_items(
        [
            '`', '~', '!', '@', '%', '^', '&', '*', '(', ')', '-', '=', '+', '[', '{', ']', '}',
            '\\', '|', ';', ':', '\'', '"', ',', '.', '<', '>', '/', '?',
        ]
        .into_iter(),
    )
}

// Finds the length and first rune of the word that ends at the given position.
// e.g. for "abc def.ghi|jkl", the word length is 3 and the word start is 'g'.
pub fn get_word_length_and_start(source_file: &ast::SourceFile, position: usize) -> (usize, char) {
    // !!! Port other case of vscode's `DEFAULT_WORD_REGEXP` that covers words that start like numbers, e.g. -123.456abcd.
    let text = &source_file.text()[..position];
    let separators = word_separators();
    let mut total_size = 0;
    let mut first_rune = '\0';
    while total_size < text.len() {
        let remaining = &text[..text.len() - total_size];
        let Some(ch) = remaining.chars().next_back() else {
            break;
        };
        if separators.has(&ch) || ch.is_whitespace() {
            break;
        }
        total_size += ch.len_utf8();
        first_rune = ch;
    }
    // If word starts with `@`, disregard this first character.
    if first_rune == '@' {
        total_size -= 1;
        first_rune = text[text.len() - total_size..]
            .chars()
            .next()
            .unwrap_or('\0');
    }
    (total_size, first_rune)
}

// `["ab c"]` -> `ab c`
// `['ab c']` -> `ab c`
// `[123]` -> `123`
pub fn trim_element_access(text: &str) -> String {
    let mut text = text.strip_prefix('[').unwrap_or(text);
    text = text.strip_suffix(']').unwrap_or(text);
    if text.starts_with('\'') && text.ends_with('\'') {
        text = text.strip_prefix('\'').unwrap_or(text);
        text = text.strip_suffix('\'').unwrap_or(text);
    }
    if text.starts_with('"') && text.ends_with('"') {
        text = text.strip_prefix('"').unwrap_or(text);
        text = text.strip_suffix('"').unwrap_or(text);
    }
    text.to_string()
}

// Ported from vscode ts extension: `getFilterText`.
pub fn get_filter_text(
    _file: &ast::SourceFile,
    _position: i32,
    insert_text: &str,
    label: &str,
    word_start: char,
    dot_accessor: &str,
) -> String {
    // Private field completion, e.g. label `#bar`.
    if let Some(after) = label.strip_prefix('#') {
        if !insert_text.is_empty() {
            if let Some(after) = insert_text.strip_prefix("this.#") {
                if word_start == '#' {
                    // `method() { this.#| }`
                    // `method() { #| }`
                    return String::new();
                } else {
                    // `method() { this.| }`
                    // `method() { | }`
                    return after.to_string();
                }
            }
        } else if word_start == '#' {
            // `method() { this.#| }`
            return String::new();
        } else {
            // `method() { this.| }`
            // `method() { | }`
            return after.to_string();
        }
    }

    // For `this.` completions, generally don't set the filter text since we don't want them to be overly deprioritized. microsoft/vscode#74164
    if insert_text.starts_with("this.") {
        return String::new();
    }

    // Handle the case:
    // ```
    // const xyz = { 'ab c': 1 };
    // xyz.ab|
    // ```
    // In which case we want to insert a bracket accessor but should use `.abc` as the filter text instead of
    // the bracketed insert text.
    if insert_text.starts_with('[') {
        return dot_accessor.to_string() + &trim_element_access(insert_text);
    }

    if let Some(after_question_dot) = insert_text.strip_prefix("?.") {
        // Handle this case like the case above:
        // ```
        // const xyz = { 'ab c': 1 } | undefined;
        // xyz.ab|
        // ```
        // filterText should be `.ab c` instead of `?.['ab c']`.
        if after_question_dot.starts_with('[') {
            return dot_accessor.to_string() + &trim_element_access(after_question_dot);
        } else {
            // ```
            // const xyz = { abc: 1 } | undefined;
            // xyz.ab|
            // ```
            // filterText should be `.abc` instead of `?.abc.
            return dot_accessor.to_string() + after_question_dot;
        }
    }

    // In all other cases, fall back to using the insertText.
    insert_text.to_string()
}

// Ported from vscode's `provideCompletionItems`.
pub fn get_dot_accessor(file: &ast::SourceFile, position: usize) -> String {
    let text = &file.text()[..position];
    if text.ends_with("?.") {
        return file.text()[position - 2..position].to_string();
    }
    if text.ends_with('.') {
        return file.text()[position - 1..position].to_string();
    }
    String::new()
}

pub fn str_ptr_is_empty(ptr: Option<&String>) -> bool {
    ptr.is_none_or(|ptr| ptr.is_empty())
}

pub fn str_ptr_to(v: String) -> Option<String> {
    if v.is_empty() {
        return None;
    }
    Some(v)
}

pub fn bool_to_ptr(v: bool) -> Option<bool> {
    if v {
        return Some(true);
    }
    None
}

pub fn get_line_of_position(file: &ast::SourceFile, pos: i32) -> i32 {
    scanner::get_ecma_line_of_position(file, pos) as i32
}

pub fn get_line_end_of_position(file: &ast::SourceFile, pos: i32) -> i32 {
    let line = get_line_of_position(file, pos);
    let line_starts = scanner::get_ecma_line_starts(file);
    let last_char_pos = if line as usize + 1 >= line_starts.len() {
        file.end()
    } else {
        line_starts[line as usize + 1] as i32 - 1
    };
    let full_text = file.text();
    if last_char_pos > 0
        && (last_char_pos as usize) < full_text.len()
        && full_text.as_bytes()[last_char_pos as usize] == b'\n'
        && full_text.as_bytes()[last_char_pos as usize - 1] == b'\r'
    {
        return last_char_pos - 1;
    }
    last_char_pos
}

pub fn is_class_like_member_completion(
    _symbol: CompletionSymbol,
    _location: &ast::Node,
    _file: &ast::SourceFile,
) -> bool {
    // !!! class member completions
    false
}

pub(crate) fn symbol_appears_to_be_type_only(
    symbol: CompletionSymbol,
    type_checker: &mut checker::Checker,
) -> bool {
    let symbol = type_checker.skip_alias_public(symbol).unwrap_or(symbol);
    let flags = completion_symbol_combined_flags(type_checker, symbol);
    let declarations = completion_symbol_declarations(type_checker, symbol);
    flags & ast::SYMBOL_FLAGS_VALUE == ast::SYMBOL_FLAGS_NONE
        && (declarations.is_empty()
            || type_checker
                .source_file_store(declarations[0])
                .is_none_or(|store| !ast::is_in_js_file(store, declarations[0]))
            || flags & ast::SYMBOL_FLAGS_TYPE != ast::SYMBOL_FLAGS_NONE)
}

pub(crate) fn should_include_symbol<'a>(
    symbol: CompletionSymbol,
    data: &CompletionDataData<'a>,
    closest_symbol_declaration: Option<ast::Declaration>,
    file: &'a ast::SourceFile,
    type_checker: &mut checker::Checker<'a, '_>,
    compiler_options: &core::CompilerOptions,
) -> bool {
    let store = file.store();
    let mut all_flags = completion_symbol_flags(type_checker, symbol);
    let Some(location) = data.location else {
        return false;
    };
    // export = /**/ here we want to get all meanings, so any symbol is ok
    if node_parent(store, location)
        .as_ref()
        .is_some_and(|parent| ast::is_export_assignment(store, *parent))
    {
        return true;
    }

    // Filter out variables from their own initializers
    // `const a = /* no 'a' here */`
    if closest_symbol_declaration.is_some_and(|closest_symbol_declaration| {
        ast::is_variable_declaration(store, closest_symbol_declaration)
            && completion_symbol_value_declaration(type_checker, symbol)
                .is_some_and(|value_declaration| value_declaration == closest_symbol_declaration)
    }) {
        return false;
    }

    // Filter out current and latter parameters from defaults
    // `function f(a = /* no 'a' and 'b' here */, b) { }` or
    // `function f<T = /* no 'T' and 'T2' here */>(a: T, b: T2) { }`
    let symbol_declaration =
        completion_symbol_value_declaration(type_checker, symbol).or_else(|| {
            completion_symbol_declarations(type_checker, symbol)
                .first()
                .copied()
        });

    if let (Some(closest_symbol_declaration), Some(symbol_declaration)) =
        (closest_symbol_declaration, symbol_declaration)
    {
        if ast::is_parameter_declaration(store, closest_symbol_declaration)
            && ast::is_parameter_declaration(store, symbol_declaration)
        {
            let parameters = store
                .parameters(node_parent(store, &closest_symbol_declaration).unwrap())
                .unwrap();
            if store.loc(symbol_declaration).pos() >= store.loc(closest_symbol_declaration).pos()
                && store.loc(symbol_declaration).pos() < parameters.end()
            {
                return false;
            }
        } else if ast::is_type_parameter_declaration(store, closest_symbol_declaration)
            && ast::is_type_parameter_declaration(store, symbol_declaration)
        {
            if closest_symbol_declaration == symbol_declaration
                && data.context_token.is_some_and(|context_token| {
                    store.kind(context_token) == ast::Kind::ExtendsKeyword
                })
            {
                // filter out the directly self-recursive type parameters
                // `type A<K extends /* no 'K' here*/> = K`
                return false;
            }
            if is_in_type_parameter_default(store, data.context_token.as_ref())
                && !ast::is_infer_type_node(
                    store,
                    node_parent(store, &closest_symbol_declaration).unwrap(),
                )
            {
                let type_parameters =
                    store.type_parameters(node_parent(store, &closest_symbol_declaration).unwrap());
                if type_parameters.is_some_and(|type_parameters| {
                    store.loc(symbol_declaration).pos()
                        >= store.loc(closest_symbol_declaration).pos()
                        && store.loc(symbol_declaration).pos() < type_parameters.end()
                }) {
                    return false;
                }
            }
        }
    }

    // External modules can have global export declarations that will be
    // available as global keywords in all scopes. But if the external module
    // already has an explicit export and user only wants to use explicit
    // module imports then the global keywords will be filtered out so auto
    // import suggestions will win in the completion.
    let symbol_origin = type_checker.skip_alias_public(symbol).unwrap_or(symbol);
    // We only want to filter out the global keywords.
    // Auto Imports are not available for scripts so this conditional is always false.
    if file.external_module_indicator().is_some()
        && compiler_options.allow_umd_global_access != core::TSTrue
        && symbol != symbol_origin
        && data
            .symbol_to_sort_text_map
            .get(&symbol)
            .is_some_and(|sort_text| sort_text == SORT_TEXT_GLOBALS_OR_KEYWORDS)
        && type_checker
            .symbol_parent_public(symbol)
            .is_some_and(|parent| is_external_module_symbol_identity(store, type_checker, parent))
    {
        return false;
    }

    all_flags |= completion_symbol_combined_flags(type_checker, symbol_origin);
    if completion_symbol_flags(type_checker, symbol) & ast::SYMBOL_FLAGS_ALIAS
        != ast::SYMBOL_FLAGS_NONE
    {
        all_flags |= completion_symbol_flags(type_checker, symbol);
    }

    // import m = /**/ <-- It can only access namespace (if typing import = x. this would get member symbols and not namespace)
    if is_in_right_side_of_internal_import_equals_declaration(store, location) {
        return all_flags & ast::SYMBOL_FLAGS_NAMESPACE != ast::SYMBOL_FLAGS_NONE;
    }

    if data.is_type_only_location {
        // It's a type, but you can reach it by namespace.type as well.
        return symbol_can_be_referenced_at_type_location(
            symbol,
            type_checker,
            collections::Set::new(),
        );
    }

    // expressions are value space (which includes the value namespaces)
    all_flags & ast::SYMBOL_FLAGS_VALUE != ast::SYMBOL_FLAGS_NONE
}

pub fn get_completion_entry_display_name_for_symbol(
    _store: &ast::AstStore,
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
    origin: Option<&SymbolOriginInfo>,
    completion_kind: CompletionKind,
    is_jsx_identifier_expected: bool,
) -> (String, bool) {
    if origin_is_ignore(origin) {
        return (String::new(), false);
    }

    let name = if origin_includes_symbol_name(origin) {
        origin.unwrap().symbol_name()
    } else {
        completion_symbol_name(type_checker, symbol)
    };
    if name.is_empty()
        ||
        // If the symbol is external module, don't show it in the completion list
        // (i.e declare module "http" { const x; } | // <= request completion here, "http" should not be there)
        completion_symbol_flags(type_checker, symbol) & ast::SYMBOL_FLAGS_MODULE != ast::SYMBOL_FLAGS_NONE && starts_with_quote(&name)
        ||
        // If the symbol is the internal name of an ES symbol, it is not a valid entry. Internal names for ES symbols start with "__@"
        is_known_symbol_name(&name)
    {
        return (String::new(), false);
    }

    let variant = if is_jsx_identifier_expected {
        core::LanguageVariant::JSX
    } else {
        core::LanguageVariant::Standard
    };
    // name is a valid identifier or private identifier text
    if scanner::is_identifier_text(&name, variant)
        || completion_symbol_value_declaration(type_checker, symbol).is_some_and(
            |value_declaration| {
                type_checker
                    .source_file_store(value_declaration)
                    .is_some_and(|declaration_store| {
                        ast::is_private_identifier_class_element_declaration(
                            declaration_store,
                            value_declaration,
                        )
                    })
            },
        )
    {
        return (name, false);
    }
    if completion_symbol_flags(type_checker, symbol) & ast::SYMBOL_FLAGS_ALIAS
        != ast::SYMBOL_FLAGS_NONE
    {
        // Allow non-identifier import/export aliases since we can insert them as string literals
        return (name, true);
    }

    match completion_kind {
        COMPLETION_KIND_MEMBER_LIKE => {
            if origin_is_computed_property_name(origin) {
                return (origin.unwrap().symbol_name(), false);
            }
            (String::new(), false)
        }
        COMPLETION_KIND_OBJECT_PROPERTY_DECLARATION => {
            // TODO: microsoft/TypeScript#18169
            (
                core::stringify_json(&name, "", "").unwrap_or_else(|_| name.clone()),
                false,
            )
        }
        COMPLETION_KIND_PROPERTY_ACCESS | COMPLETION_KIND_GLOBAL => {
            // For a 'this.' completion it will be in a global context, but may have a non-identifier name.
            // Don't add a completion for a name starting with a space. See https://github.com/Microsoft/TypeScript/pull/20547
            if name.starts_with(' ') {
                return (String::new(), false);
            }
            (name, true)
        }
        COMPLETION_KIND_NONE | COMPLETION_KIND_STRING => (name, false),
        _ => panic!("Unexpected completion kind: {}", completion_kind),
    }
}

pub fn origin_is_ignore(origin: Option<&SymbolOriginInfo>) -> bool {
    origin.is_some_and(|origin| origin.kind & SYMBOL_ORIGIN_INFO_KIND_IGNORE != 0)
}

pub fn origin_includes_symbol_name(origin: Option<&SymbolOriginInfo>) -> bool {
    origin_is_computed_property_name(origin)
}

pub fn origin_is_computed_property_name(origin: Option<&SymbolOriginInfo>) -> bool {
    origin.is_some_and(|origin| origin.kind & SYMBOL_ORIGIN_INFO_KIND_COMPUTED_PROPERTY_NAME != 0)
}

pub fn origin_is_object_literal_method(origin: Option<&SymbolOriginInfo>) -> bool {
    origin.is_some_and(|origin| origin.kind & SYMBOL_ORIGIN_INFO_KIND_OBJECT_LITERAL_METHOD != 0)
}

pub fn origin_is_this_type_node(origin: Option<&SymbolOriginInfo>) -> bool {
    origin.is_some_and(|origin| origin.kind & SYMBOL_ORIGIN_INFO_KIND_THIS_TYPE != 0)
}

pub fn origin_is_type_only_alias(origin: Option<&SymbolOriginInfo>) -> bool {
    origin.is_some_and(|origin| origin.kind & SYMBOL_ORIGIN_INFO_KIND_TYPE_ONLY_ALIAS != 0)
}

pub fn origin_is_symbol_member(origin: Option<&SymbolOriginInfo>) -> bool {
    origin.is_some_and(|origin| origin.kind & SYMBOL_ORIGIN_INFO_KIND_SYMBOL_MEMBER != 0)
}

pub fn origin_is_nullable_member(origin: Option<&SymbolOriginInfo>) -> bool {
    origin.is_some_and(|origin| origin.kind & SYMBOL_ORIGIN_INFO_KIND_NULLABLE != 0)
}

pub fn origin_is_promise(origin: Option<&SymbolOriginInfo>) -> bool {
    origin.is_some_and(|origin| origin.kind & SYMBOL_ORIGIN_INFO_KIND_PROMISE != 0)
}

pub fn get_source_from_origin(origin: Option<&SymbolOriginInfo>) -> String {
    if origin_is_this_type_node(origin) {
        return COMPLETION_SOURCE_THIS_PROPERTY.to_string();
    }

    if origin_is_type_only_alias(origin) {
        return COMPLETION_SOURCE_TYPE_ONLY_ALIAS.to_string();
    }

    String::new()
}

// In a scenarion such as `const x = 1 * |`, the context and previous tokens are both `*`.
// In `const x = 1 * o|`, the context token is *, and the previous token is `o`.
// `contextToken` and `previousToken` can both be nil if we are at the beginning of the file.
pub fn get_relevant_tokens(
    position: i32,
    file: &ast::SourceFile,
) -> (Option<ast::Node>, Option<ast::Node>) {
    let store = file.store();
    let previous_token = astnav::find_preceding_token(file, position);
    if previous_token.is_some_and(|previous_token| {
        position <= store.loc(previous_token).end()
            && (ast::is_member_name(store, previous_token)
                || ast::is_keyword_kind(store.kind(previous_token)))
    }) {
        let previous_token = previous_token.unwrap();
        let context_token = astnav::find_preceding_token(file, store.loc(previous_token).pos());
        return (context_token, Some(previous_token));
    }
    (previous_token, previous_token)
}

fn get_relevant_token_infos(
    position: i32,
    file: &ast::SourceFile,
) -> (Option<astnav::TokenInfo>, Option<astnav::TokenInfo>) {
    let store = file.store();
    let previous_token = astnav::find_preceding_token_info(file, position);
    if previous_token.is_some_and(|previous_token| {
        position <= previous_token.loc.end()
            && (is_member_name_token_info(store, &previous_token)
                || ast::is_keyword_kind(previous_token.kind))
    }) {
        let previous_token = previous_token.unwrap();
        let context_token = astnav::find_preceding_token_info(file, previous_token.loc.pos());
        return (context_token, Some(previous_token));
    }
    (previous_token, previous_token)
}

fn is_member_name_token_info(store: &ast::AstStore, token: &astnav::TokenInfo) -> bool {
    token
        .node
        .is_some_and(|node| ast::is_member_name(store, node))
        || matches!(
            token.kind,
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier
        )
}

fn is_scanned_completion_context_token(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::OpenBraceToken
            | ast::Kind::CommaToken
            | ast::Kind::DotToken
            | ast::Kind::QuestionDotToken
            | ast::Kind::ColonToken
            | ast::Kind::AsKeyword
            | ast::Kind::SatisfiesKeyword
    )
}

fn is_scanned_type_context_token(
    store: &ast::AstStore,
    kind: ast::Kind,
    parent: Option<ast::Node>,
    location: ast::Node,
) -> bool {
    match kind {
        ast::Kind::AsKeyword => {
            is_scanned_context_token_parent(store, parent, location, ast::Kind::AsExpression)
        }
        ast::Kind::SatisfiesKeyword => {
            is_scanned_context_token_parent(store, parent, location, ast::Kind::SatisfiesExpression)
        }
        _ => false,
    }
}

fn is_scanned_context_token_parent(
    store: &ast::AstStore,
    parent: Option<ast::Node>,
    location: ast::Node,
    expected_kind: ast::Kind,
) -> bool {
    parent.is_some_and(|parent| store.kind(parent) == expected_kind)
        || store.kind(location) == expected_kind
        || node_parent(store, &location).is_some_and(|parent| store.kind(parent) == expected_kind)
}

// "." | '"' | "'" | "`" | "/" | "@" | "<" | "#" | " "
pub type CompletionsTriggerCharacter = String;

pub fn is_valid_trigger(
    file: &ast::SourceFile,
    trigger_character: &str,
    context_token: Option<&ast::Node>,
    position: i32,
) -> bool {
    let store = file.store();
    match trigger_character {
        "." | "@" => true,
        "\"" | "'" | "`" => {
            // Only automatically bring up completions if this is an opening quote.
            context_token.is_some_and(|context_token| {
                is_string_literal_or_template(file.store(), *context_token)
                    && position == astnav::get_start_of_node(*context_token, file) + 1
            })
        }
        "#" => context_token.is_some_and(|context_token| {
            ast::is_private_identifier(store, *context_token)
                && ast::get_containing_class(file.store(), *context_token).is_some()
        }),
        "<" => {
            // Opening JSX tag
            context_token.is_some_and(|context_token| {
                store.kind(*context_token) == ast::Kind::LessThanToken
                    && (!node_parent(file.store(), context_token)
                        .as_ref()
                        .is_some_and(|parent| ast::is_binary_expression(store, *parent))
                        || binary_expression_may_be_open_tag(
                            store,
                            node_parent(file.store(), context_token).unwrap(),
                        ))
            })
        }
        "/" => {
            let Some(context_token) = context_token else {
                return false;
            };
            if ast::is_string_literal_like(store, *context_token) {
                return ast::try_get_import_from_module_specifier(file.store(), context_token)
                    .is_some();
            }
            store.kind(*context_token) == ast::Kind::LessThanSlashToken
                && node_parent(file.store(), context_token)
                    .as_ref()
                    .is_some_and(|parent| ast::is_jsx_closing_element(store, *parent))
        }
        " " => context_token.is_some_and(|context_token| {
            store.kind(*context_token) == ast::Kind::ImportKeyword
                && node_parent(file.store(), context_token)
                    .is_some_and(|parent| store.kind(parent) == ast::Kind::SourceFile)
        }),
        _ => panic!("Unknown trigger character: {}", trigger_character),
    }
}

pub(crate) fn is_string_literal_or_template(store: &ast::AstStore, node: ast::Node) -> bool {
    matches!(
        store.kind(node),
        ast::Kind::StringLiteral
            | ast::Kind::NoSubstitutionTemplateLiteral
            | ast::Kind::TemplateExpression
            | ast::Kind::TaggedTemplateExpression
    )
}

pub fn binary_expression_may_be_open_tag(
    store: &ast::AstStore,
    binary_expression: ast::Node,
) -> bool {
    let left = store.left(binary_expression);
    ast::node_is_missing(store, left)
}

pub fn is_checked_file(file: &ast::SourceFile, compiler_options: &core::CompilerOptions) -> bool {
    !ast::is_source_file_js(file) || ast::is_check_jsenabled_for_file(file, compiler_options)
}

pub fn is_context_token_value_location(
    store: &ast::AstStore,
    context_token: Option<&ast::Node>,
) -> bool {
    context_token.is_some_and(|context_token| {
        (store.kind(*context_token) == ast::Kind::TypeOfKeyword
            && node_parent(store, context_token)
                .as_ref()
                .is_some_and(|parent| {
                    store.kind(*parent) == ast::Kind::TypeQuery
                        || ast::is_type_of_expression(store, *parent)
                }))
            || (store.kind(*context_token) == ast::Kind::AssertsKeyword
                && node_parent(store, context_token)
                    .as_ref()
                    .is_some_and(|parent| store.kind(*parent) == ast::Kind::TypePredicate))
    })
}

pub fn is_context_token_type_location(
    store: &ast::AstStore,
    context_token: Option<&ast::Node>,
) -> bool {
    if let Some(context_token) = context_token {
        let Some(parent) = node_parent(store, context_token) else {
            return false;
        };
        let parent_kind = store.kind(parent);
        match store.kind(*context_token) {
            ast::Kind::ColonToken => {
                return parent_kind == ast::Kind::PropertyDeclaration
                    || parent_kind == ast::Kind::PropertySignature
                    || parent_kind == ast::Kind::Parameter
                    || parent_kind == ast::Kind::VariableDeclaration
                    || ast::is_function_like_kind(parent_kind);
            }
            ast::Kind::EqualsToken => {
                return parent_kind == ast::Kind::TypeAliasDeclaration
                    || parent_kind == ast::Kind::TypeParameter;
            }
            ast::Kind::AsKeyword => return parent_kind == ast::Kind::AsExpression,
            ast::Kind::LessThanToken => {
                return parent_kind == ast::Kind::TypeReference
                    || parent_kind == ast::Kind::TypeAssertionExpression;
            }
            ast::Kind::ExtendsKeyword => return parent_kind == ast::Kind::TypeParameter,
            ast::Kind::SatisfiesKeyword => return parent_kind == ast::Kind::SatisfiesExpression,
            _ => {}
        }
    }
    false
}

fn is_context_token_info_type_location(
    store: &ast::AstStore,
    context_token: Option<&astnav::TokenInfo>,
    location: ast::Node,
) -> bool {
    context_token.is_some_and(|context_token| {
        context_token.node.is_none()
            && is_scanned_type_context_token(
                store,
                context_token.kind,
                context_token.parent,
                location,
            )
    })
}

pub fn get_nullable_symbol_origin_info_kind(
    mut kind: SymbolOriginInfoKind,
    insert_question_dot: bool,
) -> SymbolOriginInfoKind {
    if insert_question_dot {
        kind |= SYMBOL_ORIGIN_INFO_KIND_NULLABLE;
    }
    kind
}

pub fn is_equality_operator_kind(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::EqualsEqualsEqualsToken
            | ast::Kind::EqualsEqualsToken
            | ast::Kind::ExclamationEqualsEqualsToken
            | ast::Kind::ExclamationEqualsToken
    )
}

// We disregard boolean literals for completion purposes.
fn is_literal(checker: &checker::Checker<'_, '_>, t: checker::TypeHandle) -> bool {
    checker.is_string_literal_type_public(t)
        || checker.is_number_literal_type_public(t)
        || checker.is_big_int_literal_type_public(t)
}

pub fn is_abstract_constructor_symbol(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: ast::SymbolIdentity,
) -> bool {
    if completion_symbol_flags(type_checker, symbol) & ast::SYMBOL_FLAGS_CLASS
        != ast::SYMBOL_FLAGS_NONE
    {
        let declaration = completion_symbol_declarations(type_checker, symbol)
            .iter()
            .copied()
            .find(|declaration| {
                declaration.store_id() == store.store_id()
                    && ast::is_class_like(store, *declaration)
            });
        return declaration.is_some_and(|declaration| {
            ast::has_syntactic_modifier(store, declaration, ast::MODIFIER_FLAGS_ABSTRACT)
        });
    }
    false
}

pub fn starts_with_quote(s: &str) -> bool {
    s.starts_with('"') || s.starts_with('\'')
}

pub(crate) fn is_arrow_function_body(store: &ast::AstStore, node: ast::Node) -> bool {
    node_parent(store, &node)
        .as_ref()
        .is_some_and(|parent| ast::is_arrow_function(store, *parent))
        && (node_body(store, node_parent(store, &node).unwrap())
            // PORT NOTE: reshaped for Rust AST value handles; Node::body returns an owned
            // handle, so compare the same observable source range and kind.
            .is_some_and(|body| {
                store.kind(body) == store.kind(node)
                    && store.loc(body).pos() == store.loc(node).pos()
                    && store.loc(body).end() == store.loc(node).end()
            })
            ||
            // const a = () => /**/;
            store.kind(node) == ast::Kind::EqualsGreaterThanToken)
}

pub(crate) fn is_deprecated(symbol: CompletionSymbol, type_checker: &mut checker::Checker) -> bool {
    let symbol = type_checker.skip_alias_public(symbol).unwrap_or(symbol);
    let declarations = completion_symbol_declarations(type_checker, symbol);
    !declarations.is_empty()
        && declarations
            .iter()
            .all(|decl| type_checker.is_deprecated_declaration(*decl))
}

pub fn quote_property_name(
    file: &ast::SourceFile,
    preferences: lsutil::UserPreferences,
    name: &str,
) -> String {
    if name.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        return name.to_string();
    }
    quote(file, preferences, name)
}

pub fn escape_snippet_text(text: &str) -> String {
    text.replace('$', "\\$")
}

pub fn is_named_imports_or_exports(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_named_imports(store, node) || ast::is_named_exports(store, node)
}

pub fn generate_identifier_for_arbitrary_string(text: &str) -> String {
    let mut needs_underscore = false;
    let mut identifier = String::new();

    // Convert "(example, text)" into "_example_text_"
    for (pos, ch) in text.char_indices() {
        let valid_char = if pos == 0 {
            scanner::is_identifier_start(ch)
        } else {
            scanner::is_identifier_part(ch)
        };
        if valid_char {
            if needs_underscore {
                identifier.push('_');
            }
            identifier.push(ch);
            needs_underscore = false;
        } else {
            needs_underscore = true;
        }
    }

    if needs_underscore {
        identifier.push('_');
    }

    // Default to "_" if the provided text was empty
    if identifier.is_empty() {
        return "_".to_string();
    }

    identifier
}

// Copied from vscode TS extension.
pub fn get_completions_symbol_kind(kind: lsutil::ScriptElementKind) -> lsproto::CompletionItemKind {
    match kind {
        lsutil::ScriptElementKind::PrimitiveType | lsutil::ScriptElementKind::Keyword => {
            lsproto::CompletionItemKind::KEYWORD
        }
        lsutil::ScriptElementKind::ConstElement
        | lsutil::ScriptElementKind::LetElement
        | lsutil::ScriptElementKind::VariableElement
        | lsutil::ScriptElementKind::LocalVariableElement
        | lsutil::ScriptElementKind::Alias
        | lsutil::ScriptElementKind::ParameterElement => lsproto::CompletionItemKind::VARIABLE,

        lsutil::ScriptElementKind::MemberVariableElement
        | lsutil::ScriptElementKind::MemberGetAccessorElement
        | lsutil::ScriptElementKind::MemberSetAccessorElement => lsproto::CompletionItemKind::FIELD,

        lsutil::ScriptElementKind::FunctionElement
        | lsutil::ScriptElementKind::LocalFunctionElement => lsproto::CompletionItemKind::FUNCTION,

        lsutil::ScriptElementKind::MemberFunctionElement
        | lsutil::ScriptElementKind::ConstructSignatureElement
        | lsutil::ScriptElementKind::CallSignatureElement
        | lsutil::ScriptElementKind::IndexSignatureElement => lsproto::CompletionItemKind::METHOD,

        lsutil::ScriptElementKind::EnumElement => lsproto::CompletionItemKind::ENUM,

        lsutil::ScriptElementKind::EnumMemberElement => lsproto::CompletionItemKind::ENUM_MEMBER,

        lsutil::ScriptElementKind::ModuleElement
        | lsutil::ScriptElementKind::ExternalModuleName => lsproto::CompletionItemKind::MODULE,

        lsutil::ScriptElementKind::ClassElement | lsutil::ScriptElementKind::TypeElement => {
            lsproto::CompletionItemKind::CLASS
        }

        lsutil::ScriptElementKind::InterfaceElement => lsproto::CompletionItemKind::INTERFACE,

        lsutil::ScriptElementKind::Warning => lsproto::CompletionItemKind::TEXT,

        lsutil::ScriptElementKind::ScriptElement => lsproto::CompletionItemKind::FILE,

        lsutil::ScriptElementKind::Directory => lsproto::CompletionItemKind::FOLDER,

        lsutil::ScriptElementKind::String => lsproto::CompletionItemKind::CONSTANT,

        _ => lsproto::CompletionItemKind::PROPERTY,
    }
}

// Editors will use the `sortText` and then fall back to `name` for sorting, but leave ties in response order.
// So, it's important that we sort those ties in the order we want them displayed if it matters. We don't
// strictly need to sort by name or SortText here since clients are going to do it anyway, but we have to
// do the work of comparing them so we can sort those ties appropriately.
pub fn compare_completion_entries(a: &lsproto::CompletionItem, b: &lsproto::CompletionItem) -> i32 {
    let compare_strings = stringutil::compare_strings_case_insensitive_then_sensitive;
    let mut result = compare_strings(
        a.sort_text.as_deref().unwrap_or_default(),
        b.sort_text.as_deref().unwrap_or_default(),
    );
    if result == stringutil::COMPARISON_EQUAL {
        result = compare_strings(&a.label, &b.label);
    }
    result as i32
}

pub fn all_keyword_completions() -> &'static Vec<lsproto::CompletionItem> {
    static ALL_KEYWORD_COMPLETIONS: OnceLock<Vec<lsproto::CompletionItem>> = OnceLock::new();
    ALL_KEYWORD_COMPLETIONS.get_or_init(|| {
        let mut result = Vec::with_capacity(
            (ast::Kind::LastKeyword as usize) - (ast::Kind::FirstKeyword as usize) + 1,
        );
        let mut kind = ast::Kind::FirstKeyword;
        loop {
            result.push(lsproto::CompletionItem {
                label: scanner::token_to_string(kind),
                kind: Some(lsproto::CompletionItemKind::KEYWORD),
                sort_text: Some(SORT_TEXT_GLOBALS_OR_KEYWORDS.to_string()),
                ..Default::default()
            });
            if kind == ast::Kind::LastKeyword {
                break;
            }
            kind = kind.next();
        }
        result
    })
}

pub fn clone_items(items: &[lsproto::CompletionItem]) -> Vec<lsproto::CompletionItem> {
    items.to_vec()
}

pub fn get_keyword_completions(
    keyword_filter: KeywordCompletionFilters,
    filter_out_ts_only_keywords: bool,
) -> Vec<lsproto::CompletionItem> {
    if !filter_out_ts_only_keywords {
        return clone_items(&get_typescript_keyword_completions(keyword_filter));
    }

    get_typescript_keyword_completions(keyword_filter)
        .into_iter()
        .filter(|ci| !is_type_script_only_keyword(scanner::string_to_token(&ci.label)))
        .collect()
}

pub fn get_typescript_keyword_completions(
    keyword_filter: KeywordCompletionFilters,
) -> Vec<lsproto::CompletionItem> {
    all_keyword_completions()
        .iter()
        .filter(|entry| {
            let kind = scanner::string_to_token(&entry.label);
            match keyword_filter {
                KEYWORD_COMPLETION_FILTERS_NONE => false,
                KEYWORD_COMPLETION_FILTERS_ALL => {
                    is_function_like_body_keyword(kind)
                        || kind == ast::Kind::DeclareKeyword
                        || kind == ast::Kind::ModuleKeyword
                        || kind == ast::Kind::TypeKeyword
                        || kind == ast::Kind::NamespaceKeyword
                        || kind == ast::Kind::AbstractKeyword
                        || is_type_keyword(kind) && kind != ast::Kind::UndefinedKeyword
                }
                KEYWORD_COMPLETION_FILTERS_FUNCTION_LIKE_BODY_KEYWORDS => {
                    is_function_like_body_keyword(kind)
                }
                KEYWORD_COMPLETION_FILTERS_CLASS_ELEMENT_KEYWORDS => {
                    is_class_member_completion_keyword(kind)
                }
                KEYWORD_COMPLETION_FILTERS_INTERFACE_ELEMENT_KEYWORDS => {
                    is_interface_or_type_literal_completion_keyword(kind)
                }
                KEYWORD_COMPLETION_FILTERS_CONSTRUCTOR_PARAMETER_KEYWORDS => {
                    ast::is_parameter_property_modifier(kind)
                }
                KEYWORD_COMPLETION_FILTERS_TYPE_ASSERTION_KEYWORDS => {
                    is_type_keyword(kind) || kind == ast::Kind::ConstKeyword
                }
                KEYWORD_COMPLETION_FILTERS_TYPE_KEYWORDS => is_type_keyword(kind),
                KEYWORD_COMPLETION_FILTERS_TYPE_KEYWORD => kind == ast::Kind::TypeKeyword,
                _ => panic!("Unknown keyword filter: {}", keyword_filter),
            }
        })
        .cloned()
        .collect()
}

pub fn is_type_script_only_keyword(kind: ast::Kind) -> bool {
    matches!(
        kind,
        ast::Kind::AbstractKeyword
            | ast::Kind::AnyKeyword
            | ast::Kind::BigIntKeyword
            | ast::Kind::BooleanKeyword
            | ast::Kind::DeclareKeyword
            | ast::Kind::EnumKeyword
            | ast::Kind::GlobalKeyword
            | ast::Kind::ImplementsKeyword
            | ast::Kind::InferKeyword
            | ast::Kind::InterfaceKeyword
            | ast::Kind::IsKeyword
            | ast::Kind::KeyOfKeyword
            | ast::Kind::ModuleKeyword
            | ast::Kind::NamespaceKeyword
            | ast::Kind::NeverKeyword
            | ast::Kind::NumberKeyword
            | ast::Kind::ObjectKeyword
            | ast::Kind::OverrideKeyword
            | ast::Kind::PrivateKeyword
            | ast::Kind::ProtectedKeyword
            | ast::Kind::PublicKeyword
            | ast::Kind::ReadonlyKeyword
            | ast::Kind::StringKeyword
            | ast::Kind::SymbolKeyword
            | ast::Kind::TypeKeyword
            | ast::Kind::UniqueKeyword
            | ast::Kind::UnknownKeyword
    )
}

pub fn is_function_like_body_keyword(kind: ast::Kind) -> bool {
    kind == ast::Kind::AsyncKeyword
        || kind == ast::Kind::AwaitKeyword
        || kind == ast::Kind::UsingKeyword
        || kind == ast::Kind::AsKeyword
        || kind == ast::Kind::SatisfiesKeyword
        || kind == ast::Kind::TypeKeyword
        || !ast::is_contextual_keyword(kind) && !is_class_member_completion_keyword(kind)
}

pub fn is_class_member_completion_keyword(kind: ast::Kind) -> bool {
    match kind {
        ast::Kind::AbstractKeyword
        | ast::Kind::AccessorKeyword
        | ast::Kind::ConstructorKeyword
        | ast::Kind::GetKeyword
        | ast::Kind::SetKeyword
        | ast::Kind::AsyncKeyword
        | ast::Kind::DeclareKeyword
        | ast::Kind::OverrideKeyword => true,
        _ => ast::is_class_member_modifier(kind),
    }
}

pub fn is_interface_or_type_literal_completion_keyword(kind: ast::Kind) -> bool {
    kind == ast::Kind::ReadonlyKeyword
}

pub fn is_contextual_keyword_in_auto_importable_expression_space(keyword: &str) -> bool {
    keyword == "abstract"
        || keyword == "async"
        || keyword == "await"
        || keyword == "declare"
        || keyword == "module"
        || keyword == "namespace"
        || keyword == "type"
        || keyword == "satisfies"
        || keyword == "as"
}

pub fn is_member_completion_kind(kind: CompletionKind) -> bool {
    kind == COMPLETION_KIND_OBJECT_PROPERTY_DECLARATION
        || kind == COMPLETION_KIND_MEMBER_LIKE
        || kind == COMPLETION_KIND_PROPERTY_ACCESS
}

pub(crate) fn keyword_for_node(store: &ast::AstStore, node: ast::Node) -> ast::Kind {
    if ast::is_identifier(store, node) {
        return scanner::identifier_to_keyword_kind(store, node);
    }
    store.kind(node)
}

fn get_type_at_location_for_member_completion<'a>(
    type_checker: &mut checker::Checker<'a, '_>,
    store: &ast::AstStore,
    node: ast::Node,
) -> checker::TypeHandle {
    // TypeScript-Go's GetTypeAtLocation reaches checkExpression for calls. In
    // incomplete member-completion trees, use the resolved signature directly so
    // `foo().` exposes members of the call return type.
    if ast::is_call_expression(store, node) || ast::is_new_expression(store, node) {
        if let Some(signature) = type_checker.get_resolved_signature_public(node) {
            return type_checker.get_return_type_of_signature_public(signature);
        }
    }
    type_checker.get_type_at_location(node)
}

pub fn compute_commit_characters_and_is_new_identifier(
    context_token: Option<&ast::Node>,
    file: &ast::SourceFile,
    position: i32,
) -> (bool, Vec<String>) {
    let all_commit = || {
        ALL_COMMIT_CHARACTERS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    };
    let no_comma = || {
        NO_COMMA_COMMIT_CHARACTERS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    };
    let empty = || Vec::<String>::new();

    let Some(context_token) = context_token else {
        return (false, all_commit());
    };
    let store = file.store();
    let containing_node_kind = node_parent(store, context_token)
        .map(|parent| store.kind(parent))
        .unwrap_or(ast::Kind::Unknown);
    let token_kind = keyword_for_node(store, *context_token);
    // Previous token may have been a keyword that was converted to an identifier.
    match token_kind {
        ast::Kind::CommaToken => match containing_node_kind {
            // func( a, |
            // new C(a, |
            ast::Kind::CallExpression | ast::Kind::NewExpression => {
                let expression =
                    node_expression(store, node_parent(store, context_token).unwrap()).unwrap();
                // func\n(a, |
                if get_line_of_position(file, store.loc(expression).end())
                    != get_line_of_position(file, position)
                {
                    return (true, no_comma());
                }
                return (true, all_commit());
            }
            // const x = (a, |
            ast::Kind::BinaryExpression => return (true, no_comma()),
            // constructor( a, | /* public, protected, private keywords are allowed here, so show completion */
            // var x: (s: string, list|
            // const obj = { x, |
            ast::Kind::Constructor
            | ast::Kind::FunctionType
            | ast::Kind::ObjectLiteralExpression => {
                return (true, empty());
            }
            // [a, |
            ast::Kind::ArrayLiteralExpression => return (true, all_commit()),
            _ => return (false, all_commit()),
        },
        ast::Kind::OpenParenToken => match containing_node_kind {
            // func( |
            // new C(a|
            ast::Kind::CallExpression | ast::Kind::NewExpression => {
                let expression =
                    node_expression(store, node_parent(store, context_token).unwrap()).unwrap();
                // func\n( |
                if get_line_of_position(file, store.loc(expression).end())
                    != get_line_of_position(file, position)
                {
                    return (true, no_comma());
                }
                return (true, all_commit());
            }
            // const x = (a|
            ast::Kind::ParenthesizedExpression => return (true, no_comma()),
            // constructor( |
            // function F(pred: (a| /* this can become an arrow function, where 'a' is the argument */
            ast::Kind::Constructor | ast::Kind::ParenthesizedType => return (true, empty()),
            _ => return (false, all_commit()),
        },
        ast::Kind::OpenBracketToken => match containing_node_kind {
            // [ |
            // [ | : string ]
            // [ | : string ]
            // [ |    /* this can become an index signature */
            ast::Kind::ArrayLiteralExpression
            | ast::Kind::IndexSignature
            | ast::Kind::TupleType
            | ast::Kind::ComputedPropertyName => return (true, all_commit()),
            _ => return (false, all_commit()),
        },
        // module |
        // namespace |
        // import |
        ast::Kind::ModuleKeyword | ast::Kind::NamespaceKeyword | ast::Kind::ImportKeyword => {
            return (true, empty());
        }
        ast::Kind::DotToken => match containing_node_kind {
            // module A.|
            ast::Kind::ModuleDeclaration => return (true, empty()),
            _ => return (false, all_commit()),
        },
        ast::Kind::OpenBraceToken => match containing_node_kind {
            // class A { |
            // const obj = { |
            ast::Kind::ClassDeclaration | ast::Kind::ObjectLiteralExpression => {
                return (true, empty());
            }
            _ => return (false, all_commit()),
        },
        ast::Kind::EqualsToken => match containing_node_kind {
            // const x = a|
            // x = a|
            ast::Kind::VariableDeclaration | ast::Kind::BinaryExpression => {
                return (true, all_commit());
            }
            _ => return (false, all_commit()),
        },
        ast::Kind::TemplateHead => {
            // `aa ${|
            return (
                containing_node_kind == ast::Kind::TemplateExpression,
                all_commit(),
            );
        }
        ast::Kind::TemplateMiddle => {
            // `aa ${10} dd ${|
            return (
                containing_node_kind == ast::Kind::TemplateSpan,
                all_commit(),
            );
        }
        ast::Kind::AsyncKeyword => {
            // const obj = { async c|()
            // const obj = { async c|
            if containing_node_kind == ast::Kind::MethodDeclaration
                || containing_node_kind == ast::Kind::ShorthandPropertyAssignment
            {
                return (true, empty());
            }
            return (false, all_commit());
        }
        ast::Kind::AsteriskToken => {
            // const obj = { * c|
            if containing_node_kind == ast::Kind::MethodDeclaration {
                return (true, empty());
            }
            return (false, all_commit());
        }
        _ => {}
    }

    if is_class_member_completion_keyword(token_kind) {
        return (true, empty());
    }

    (false, all_commit())
}

pub fn get_scope_node(
    initial_token: Option<&ast::Node>,
    position: i32,
    file: &ast::SourceFile,
) -> Option<ast::Node> {
    let mut scope = initial_token.copied();
    while scope.is_some_and(|scope| !position_belongs_to_node(scope, position, file)) {
        scope = node_parent(file.store(), &scope.unwrap());
    }
    scope
}

pub(crate) fn is_snippet_scope(store: &ast::AstStore, scope_node: ast::Node) -> bool {
    match store.kind(scope_node) {
        ast::Kind::SourceFile
        | ast::Kind::TemplateExpression
        | ast::Kind::JsxExpression
        | ast::Kind::Block => true,
        _ => ast::is_statement(store, scope_node),
    }
}

pub(crate) fn get_properties_for_completion<'a>(
    t: checker::TypeHandle,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Vec<CompletionSymbol> {
    if type_checker.is_union_type_public(t) {
        return type_checker
            .get_all_possible_properties_of_types(type_checker.type_types_public(t));
    }
    type_checker.get_apparent_properties(t)
}

// Given 'a.b.c', returns 'a'.
pub fn get_left_most_name(
    store: &ast::AstStore,
    e: &ast::Expression,
) -> Option<ast::IdentifierNode> {
    if ast::is_identifier(store, *e) {
        return Some(*e);
    } else if ast::is_property_access_expression(store, *e) {
        let expression = node_expression(store, e)?;
        return get_left_most_name(store, &expression);
    }
    None
}

pub(crate) fn get_first_symbol_in_chain<'a>(
    symbol: CompletionSymbol,
    enclosing_declaration: &ast::Node,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<CompletionSymbol> {
    let _ = enclosing_declaration;
    // Missing checker facade: identity-based get_accessible_symbol_chain_public.
    let parent = type_checker.symbol_parent_public(symbol);
    if let Some(parent) = parent {
        if is_module_symbol_identity(type_checker, parent) {
            return Some(symbol);
        }
        return get_first_symbol_in_chain(parent, enclosing_declaration, type_checker);
    }
    None
}

pub fn is_module_symbol_identity(
    type_checker: &mut checker::Checker<'_, '_>,
    symbol: CompletionSymbol,
) -> bool {
    completion_symbol_declarations(type_checker, symbol)
        .iter()
        .any(|decl| {
            type_checker
                .source_file_store(*decl)
                .is_some_and(|store| store.kind(*decl) == ast::Kind::SourceFile)
        })
}

// Determines if a type is exactly the same type resolved by the global 'self', 'global', or 'globalThis'.
pub fn is_probably_global_type<'a>(
    t: checker::TypeHandle,
    file: &'a ast::SourceFile,
    type_checker: &mut checker::Checker<'a, '_>,
) -> bool {
    // The type of `self` and `window` is the same in lib.dom.d.ts, but `window` does not exist in
    // lib.webworker.d.ts, so checking against `self` is also a check against `window` when it exists.
    let self_symbol = type_checker.get_global_symbol_public(
        "self",
        ast::SYMBOL_FLAGS_VALUE,
        None, /*diagnostic*/
    );
    if let Some(symbol) = self_symbol {
        if type_checker
            .get_type_of_symbol_at_source_file_public(symbol, file)
            .is_some_and(|symbol_type| symbol_type == t)
        {
            return true;
        }
    }
    let global_symbol = type_checker.get_global_symbol_public(
        "global",
        ast::SYMBOL_FLAGS_VALUE,
        None, /*diagnostic*/
    );
    if let Some(symbol) = global_symbol {
        if type_checker
            .get_type_of_symbol_at_source_file_public(symbol, file)
            .is_some_and(|symbol_type| symbol_type == t)
        {
            return true;
        }
    }
    let global_this_symbol = type_checker.get_global_symbol_public(
        "globalThis",
        ast::SYMBOL_FLAGS_VALUE,
        None, /*diagnostic*/
    );
    if let Some(symbol) = global_this_symbol {
        if type_checker
            .get_type_of_symbol_at_source_file_public(symbol, file)
            .is_some_and(|symbol_type| symbol_type == t)
        {
            return true;
        }
    }
    false
}

pub fn get_apparent_properties<'a>(
    store: &ast::AstStore,
    t: checker::TypeHandle,
    node: &ast::Node,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Vec<CompletionSymbol> {
    if !type_checker.is_union_type_public(t) {
        return type_checker.get_apparent_properties(t);
    }
    let mut member_types = Vec::new();
    for member_type in type_checker.type_types_public(t) {
        if type_checker.type_flags_public(member_type) & checker::TYPE_FLAGS_PRIMITIVE
            != checker::TYPE_FLAGS_NONE
            || type_checker.is_array_like_type_public(member_type)
            || type_checker.is_type_invalid_due_to_union_discriminant(member_type, *node)
            || type_checker.type_has_call_or_construct_signatures_public(member_type)
        {
            continue;
        }
        if type_checker.is_class_type_public(member_type)
            && contains_non_public_properties(
                store,
                &type_checker.get_apparent_properties(member_type),
                type_checker,
            )
        {
            continue;
        }
        member_types.push(member_type);
    }
    type_checker.get_all_possible_properties_of_types(member_types)
}

pub(crate) fn contains_non_public_properties(
    store: &ast::AstStore,
    props: &[CompletionSymbol],
    type_checker: &mut checker::Checker<'_, '_>,
) -> bool {
    props.iter().any(|p| {
        completion_symbol_declarations(type_checker, *p)
            .iter()
            .any(|decl| {
                node_modifier_flags(store, decl)
                    & ast::MODIFIER_FLAGS_NON_PUBLIC_ACCESSIBILITY_MODIFIER
                    != ast::MODIFIER_FLAGS_NONE
            })
    })
}

pub(crate) fn is_currently_editing_node(
    node: &ast::Node,
    file: &ast::SourceFile,
    position: i32,
) -> bool {
    let start = astnav::get_start_of_node(*node, file);
    start <= position && position <= file.store().loc(*node).end()
}

pub fn set_member_declared_by_spread_assignment(
    store: &ast::AstStore,
    declaration: &ast::Node,
    members: &mut collections::Set<String>,
    type_checker: &mut checker::Checker,
) {
    let Some(expression) = node_expression(store, declaration) else {
        return;
    };
    let symbol = type_checker.get_symbol_at_location_public(expression);
    let mut t = None;
    if let Some(symbol) = symbol {
        t = type_checker.get_type_of_symbol_identity_at_location_public(symbol, Some(expression));
    }
    let mut properties = Vec::new();
    if t.is_some_and(|t| {
        type_checker.type_flags_public(t) & checker::TYPE_FLAGS_STRUCTURED_TYPE
            != checker::TYPE_FLAGS_NONE
    }) {
        properties = type_checker.structured_properties_public(t.unwrap());
    }
    for property in properties {
        members.add(completion_symbol_name(type_checker, property));
    }
}

pub(crate) fn is_constructor_parameter_completion(store: &ast::AstStore, node: ast::Node) -> bool {
    let parent = node_parent(store, &node);
    parent.is_some()
        && parent
            .as_ref()
            .is_some_and(|parent| ast::is_parameter_declaration(store, *parent))
        && parent
            .as_ref()
            .and_then(|parent| node_parent(store, parent))
            .as_ref()
            .is_some_and(|parent| ast::is_constructor_declaration(store, *parent))
        && (ast::is_parameter_property_modifier(store.kind(node))
            || ast::is_declaration_name(store, &node))
}

pub(crate) fn is_from_object_type_declaration(store: &ast::AstStore, node: ast::Node) -> bool {
    let parent = node_parent(store, &node);
    parent.is_some()
        && parent
            .as_ref()
            .is_some_and(|parent| ast::is_class_or_type_element(store, parent))
        && parent
            .as_ref()
            .and_then(|parent| node_parent(store, parent))
            .as_ref()
            .is_some_and(|parent| ast::is_object_type_declaration(store, parent))
}

// Returns the immediate owning class declaration of a context token,
// on the condition that one exists and that the context implies completion should be given.
pub fn try_get_object_type_declaration_completion_container<'a>(
    file: &ast::SourceFile,
    context_token: Option<&'a ast::Node>,
    location: &'a ast::Node,
    position: i32,
) -> Option<ast::Node> {
    let store = file.store();
    // class c { method() { } | method2() { } }
    match store.kind(*location) {
        ast::Kind::SyntaxList => {
            if let Some(parent) = node_parent(store, location)
                .filter(|parent| ast::is_object_type_declaration(store, parent))
            {
                return Some(parent);
            }
            return None;
        }
        ast::Kind::EndOfFile => {
            if let Some(stmt_list) =
                node_parent(store, location).and_then(|parent| node_statement_list(store, parent))
            {
                if !stmt_list.is_empty() {
                    let cls = stmt_list.last().expect("non-empty statement list");
                    if ast::is_object_type_declaration(store, cls)
                        && !astnav::has_child_of_kind(cls, ast::Kind::CloseBraceToken, file)
                    {
                        return Some(cls);
                    }
                }
            }
        }
        ast::Kind::PrivateIdentifier => {
            if node_parent(store, location)
                .as_ref()
                .is_some_and(|parent| ast::is_property_declaration(store, *parent))
            {
                return ast::find_ancestor(store, Some(*location), |store, node| {
                    ast::is_class_like(store, node)
                });
            }
        }
        ast::Kind::Identifier => {
            let original_keyword_kind = scanner::identifier_to_keyword_kind(store, *location);
            if original_keyword_kind != ast::Kind::Unknown {
                return None;
            }
            // class c { public prop = c| }
            if node_parent(store, location)
                .as_ref()
                .is_some_and(|parent| ast::is_property_declaration(store, *parent))
                && node_parent(store, location)
                    .and_then(|parent| node_initializer(store, parent))
                    .is_some_and(|initializer| {
                        store.kind(initializer) == store.kind(*location)
                            && store.loc(initializer).pos() == store.loc(*location).pos()
                            && store.loc(initializer).end() == store.loc(*location).end()
                    })
            {
                return None;
            }
            // class c extends React.Component { a: () => 1\n compon| }
            if is_from_object_type_declaration(store, *location) {
                return ast::find_ancestor(store, Some(*location), |store, node| {
                    ast::is_object_type_declaration(store, node)
                });
            }
        }
        _ => {}
    }

    let Some(context_token) = context_token else {
        return None;
    };

    // class C { blah; constructor/**/ }
    // or
    // class C { blah \n constructor/**/ }
    if store.kind(*location) == ast::Kind::ConstructorKeyword
        || (ast::is_identifier(store, *context_token)
            && node_parent(store, context_token)
                .as_ref()
                .is_some_and(|parent| ast::is_property_declaration(store, *parent))
            && ast::is_class_like(store, *location))
    {
        return ast::find_ancestor(store, Some(*context_token), |store, node| {
            ast::is_class_like(store, node)
        });
    }

    match store.kind(*context_token) {
        // class c { public prop = | /* global completions */ }
        ast::Kind::EqualsToken => None,
        // class c {getValue(): number; | }
        // class c { method() { } | }
        ast::Kind::SemicolonToken | ast::Kind::CloseBraceToken => {
            // class c { method() { } b| }
            if is_from_object_type_declaration(store, *location)
                && node_parent(store, location)
                    .and_then(|parent| node_name(store, parent))
                    .is_some_and(|name| {
                        store.kind(name) == store.kind(*location)
                            && store.loc(name).pos() == store.loc(*location).pos()
                            && store.loc(name).end() == store.loc(*location).end()
                    })
            {
                return node_parent(store, location)
                    .and_then(|parent| node_parent(store, parent))
                    .map(|parent| parent);
            }
            if ast::is_object_type_declaration(store, location) {
                return Some(*location);
            }
            None
        }
        // class c { |
        // class c {getValue(): number, | }
        ast::Kind::OpenBraceToken | ast::Kind::CommaToken => {
            if let Some(parent) = node_parent(store, context_token)
                .filter(|parent| ast::is_object_type_declaration(store, parent))
            {
                return Some(parent);
            }
            None
        }
        _ => {
            if ast::is_object_type_declaration(store, location) {
                // class C extends React.Component { a: () => 1\n| }
                // class C { prop = ""\n | }
                if get_line_of_position(file, store.loc(*context_token).end())
                    != get_line_of_position(file, position)
                {
                    return Some(*location);
                }
                let is_valid_keyword = if node_parent(store, context_token)
                    .and_then(|parent| node_parent(store, parent))
                    .as_ref()
                    .is_some_and(|parent| ast::is_class_like(store, *parent))
                {
                    is_class_member_completion_keyword
                } else {
                    is_interface_or_type_literal_completion_keyword
                };

                if is_valid_keyword(store.kind(*context_token))
                    || store.kind(*context_token) == ast::Kind::AsteriskToken
                    || ast::is_identifier(store, *context_token)
                        && is_valid_keyword(scanner::identifier_to_keyword_kind(
                            store,
                            *context_token,
                        ))
                {
                    return node_parent(store, context_token)
                        .and_then(|parent| node_parent(store, parent))
                        .map(|parent| parent);
                }
            }
            None
        }
    }
}

pub(crate) fn is_type_keyword_token_or_identifier(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_type_keyword_token(store, &node)
        || ast::is_identifier(store, node)
            && scanner::identifier_to_keyword_kind(store, node) == ast::Kind::TypeKeyword
}

pub fn is_completion_list_blocker<'a>(
    context_token: &ast::Node,
    previous_token: Option<&ast::Node>,
    location: &ast::Node,
    file: &ast::SourceFile,
    position: i32,
    type_checker: &mut checker::Checker<'a, '_>,
) -> bool {
    is_in_string_or_regular_expression_or_template_literal(file.store(), context_token, position)
        || is_solely_identifier_definition_location(
            context_token,
            previous_token,
            file,
            position,
            type_checker,
        )
        || is_dot_of_numeric_literal(*context_token, file)
        || is_in_jsx_text(file.store(), context_token, location)
        || ast::is_big_int_literal(file.store(), *context_token)
}

pub fn is_in_string_or_regular_expression_or_template_literal(
    store: &ast::AstStore,
    context_token: &ast::Node,
    position: i32,
) -> bool {
    // To be "in" one of these literals, the position has to be:
    //   1. entirely within the token text.
    //   2. at the end position of an unterminated token.
    //   3. at the end of a regular expression (due to trailing flags like '/foo/g').
    (ast::is_regular_expression_literal(store, *context_token)
        || ast::is_string_text_containing_node(store, context_token))
        && store.loc(*context_token).contains_exclusive(position)
        || position == store.loc(*context_token).end()
            && (ast::is_unterminated_literal(store, *context_token)
                || ast::is_regular_expression_literal(store, *context_token))
}

// true if we are certain that the currently edited location must define a new location; false otherwise.
pub fn is_solely_identifier_definition_location<'a>(
    context_token: &ast::Node,
    previous_token: Option<&ast::Node>,
    file: &ast::SourceFile,
    position: i32,
    type_checker: &mut checker::Checker<'a, '_>,
) -> bool {
    let store = file.store();
    let parent = node_parent(store, context_token);
    let containing_node_kind = parent
        .as_ref()
        .map(|parent| store.kind(*parent))
        .unwrap_or(ast::Kind::Unknown);
    match store.kind(*context_token) {
        ast::Kind::CommaToken => {
            return containing_node_kind == ast::Kind::VariableDeclaration
                || is_variable_declaration_list_but_not_type_argument(
                    context_token,
                    file,
                    type_checker,
                )
                || containing_node_kind == ast::Kind::VariableStatement
                || containing_node_kind == ast::Kind::EnumDeclaration
                || is_function_like_but_not_constructor(containing_node_kind)
                || containing_node_kind == ast::Kind::InterfaceDeclaration
                || containing_node_kind == ast::Kind::ArrayBindingPattern
                || containing_node_kind == ast::Kind::TypeAliasDeclaration
                || (parent
                    .as_ref()
                    .is_some_and(|parent| ast::is_class_like(store, *parent))
                    && parent
                        .as_ref()
                        .and_then(|parent| store.type_parameters(*parent))
                        .is_some()
                    && parent
                        .as_ref()
                        .and_then(|parent| store.type_parameters(*parent))
                        .unwrap()
                        .end()
                        >= store.loc(*context_token).pos());
        }
        ast::Kind::DotToken => return containing_node_kind == ast::Kind::ArrayBindingPattern,
        ast::Kind::ColonToken => return containing_node_kind == ast::Kind::BindingElement,
        ast::Kind::OpenBracketToken => {
            return containing_node_kind == ast::Kind::ArrayBindingPattern;
        }
        ast::Kind::OpenParenToken => {
            return containing_node_kind == ast::Kind::CatchClause
                || is_function_like_but_not_constructor(containing_node_kind);
        }
        ast::Kind::OpenBraceToken => return containing_node_kind == ast::Kind::EnumDeclaration,
        ast::Kind::LessThanToken => {
            return containing_node_kind == ast::Kind::ClassDeclaration
                || containing_node_kind == ast::Kind::ClassExpression
                || containing_node_kind == ast::Kind::InterfaceDeclaration
                || containing_node_kind == ast::Kind::TypeAliasDeclaration
                || ast::is_function_like_kind(containing_node_kind);
        }
        ast::Kind::StaticKeyword => {
            return containing_node_kind == ast::Kind::PropertyDeclaration
                && !parent
                    .as_ref()
                    .and_then(|parent| node_parent(store, parent))
                    .as_ref()
                    .is_some_and(|parent| ast::is_class_like(store, *parent));
        }
        ast::Kind::DotDotDotToken => {
            return containing_node_kind == ast::Kind::Parameter
                || parent
                    .as_ref()
                    .and_then(|parent| node_parent(store, parent))
                    .as_ref()
                    .is_some_and(|parent| store.kind(*parent) == ast::Kind::ArrayBindingPattern);
        }
        ast::Kind::PublicKeyword | ast::Kind::PrivateKeyword | ast::Kind::ProtectedKeyword => {
            return containing_node_kind == ast::Kind::Parameter
                && !parent
                    .as_ref()
                    .and_then(|parent| node_parent(store, parent))
                    .as_ref()
                    .is_some_and(|parent| ast::is_constructor_declaration(store, *parent));
        }
        ast::Kind::AsKeyword => {
            return containing_node_kind == ast::Kind::ImportSpecifier
                || containing_node_kind == ast::Kind::ExportSpecifier
                || containing_node_kind == ast::Kind::NamespaceImport;
        }
        ast::Kind::GetKeyword | ast::Kind::SetKeyword => {
            return !is_from_object_type_declaration(file.store(), *context_token);
        }
        ast::Kind::Identifier => {
            if (containing_node_kind == ast::Kind::ImportSpecifier
                || containing_node_kind == ast::Kind::ExportSpecifier)
                && parent
                    .as_ref()
                    .and_then(|parent| node_name(file.store(), parent))
                    .is_some_and(|name| {
                        store.kind(name) == store.kind(*context_token)
                            && store.loc(name).pos() == store.loc(*context_token).pos()
                            && store.loc(name).end() == store.loc(*context_token).end()
                    })
                && node_text(file.store(), context_token) == "type"
            {
                // import { type | }
                return false;
            }
            let ancestor_variable_declaration =
                ast::find_ancestor(file.store(), parent, ast::is_variable_declaration);
            if ancestor_variable_declaration.is_some()
                && get_line_end_of_position(file, store.loc(*context_token).end()) < position
            {
                // let a
                // |
                return false;
            }
        }
        ast::Kind::ClassKeyword
        | ast::Kind::EnumKeyword
        | ast::Kind::InterfaceKeyword
        | ast::Kind::FunctionKeyword
        | ast::Kind::VarKeyword
        | ast::Kind::ImportKeyword
        | ast::Kind::LetKeyword
        | ast::Kind::ConstKeyword
        | ast::Kind::InferKeyword => return true,
        ast::Kind::TypeKeyword => return containing_node_kind != ast::Kind::ImportSpecifier,
        ast::Kind::AsteriskToken => {
            return parent
                .as_ref()
                .is_some_and(|parent| ast::is_function_like(store, Some(*parent)))
                && !parent
                    .as_ref()
                    .is_some_and(|parent| ast::is_method_declaration(store, *parent));
        }
        _ => {}
    }

    let token_kind = keyword_for_node(file.store(), *context_token);
    // If the previous token is keyword corresponding to class member completion keyword
    // there will be completion available here
    if is_class_member_completion_keyword(token_kind)
        && is_from_object_type_declaration(file.store(), *context_token)
    {
        return false;
    }

    if is_constructor_parameter_completion(file.store(), *context_token) {
        // constructor parameter completion is available only if
        // - its modifier of the constructor parameter or
        // - its name of the parameter and not being edited
        // eg. constructor(a |<- this shouldnt show completion
        if !ast::is_identifier(store, *context_token)
            || ast::is_parameter_property_modifier(token_kind)
            || is_currently_editing_node(context_token, file, position)
        {
            return false;
        }
    }

    // Previous token may have been a keyword that was converted to an identifier.
    match keyword_for_node(file.store(), *context_token) {
        ast::Kind::AbstractKeyword
        | ast::Kind::ClassKeyword
        | ast::Kind::DeclareKeyword
        | ast::Kind::EnumKeyword
        | ast::Kind::FunctionKeyword
        | ast::Kind::InterfaceKeyword
        | ast::Kind::LetKeyword
        | ast::Kind::PrivateKeyword
        | ast::Kind::ProtectedKeyword
        | ast::Kind::PublicKeyword
        | ast::Kind::StaticKeyword
        | ast::Kind::VarKeyword => return true,
        ast::Kind::AsyncKeyword => {
            return node_parent(store, context_token)
                .as_ref()
                .is_some_and(|parent| ast::is_property_declaration(store, *parent));
        }
        _ => {}
    }

    let previous_token = previous_token.unwrap_or(context_token);
    // If we are inside a class declaration, and `constructor` is totally not present,
    // but we request a completion manually at a whitespace...
    let ancestor_class_like =
        ast::find_ancestor(store, parent, |store, node| ast::is_class_like(store, node));
    if ancestor_class_like.is_some()
        && *context_token == *previous_token
        && is_previous_property_declaration_terminated(context_token, file, position)
    {
        // Don't block completions.
        return false;
    }

    let ancestor_property_declaration =
        ast::find_ancestor(store, parent, ast::is_property_declaration);
    // If we are inside a class declaration and typing `constructor` after property declaration...
    if let Some(ancestor_property_declaration) = ancestor_property_declaration {
        if *context_token != *previous_token
            && node_parent(store, previous_token)
                .and_then(|parent| node_parent(store, parent))
                .as_ref()
                .is_some_and(|parent| ast::is_class_like(store, *parent))
            && position <= store.loc(*previous_token).end()
        {
            // If we are sure that the previous property declaration is terminated according to newline or semicolon...
            if is_previous_property_declaration_terminated(
                context_token,
                file,
                store.loc(*previous_token).end(),
            ) {
                // Don't block completions.
                return false;
            } else if store.kind(*context_token) != ast::Kind::EqualsToken
                && (ast::is_initialized_property(store, &ancestor_property_declaration)
                    || node_type(store, ancestor_property_declaration).is_some())
            {
                return true;
            }
        }
    }
    if token_kind == ast::Kind::ConstKeyword {
        return true;
    }
    ast::is_declaration_name(store, context_token)
        && !parent
            .as_ref()
            .is_some_and(|parent| ast::is_shorthand_property_assignment(store, *parent))
        && !parent
            .as_ref()
            .is_some_and(|parent| ast::is_jsx_attribute(store, *parent))
        // Don't block completions if we're in `class C /**/`, `interface I /**/` or `<T /**/>` ,
        // because we're *past* the end of the identifier and might want to complete `extends`.
        // If `contextToken !== previousToken`, this is `class C ex/**/`, `interface I ex/**/` or `<T ex/**/>`.
        && !((parent.as_ref().is_some_and(|parent| ast::is_class_like(store, *parent))
            || parent.as_ref().is_some_and(|parent| ast::is_interface_declaration(store, *parent))
            || parent.as_ref().is_some_and(|parent| ast::is_type_parameter_declaration(store, *parent)))
            && (*context_token != *previous_token || position > store.loc(*previous_token).end()))
}

pub fn is_variable_declaration_list_but_not_type_argument<'a>(
    node: &ast::Node,
    file: &ast::SourceFile,
    type_checker: &mut checker::Checker<'a, '_>,
) -> bool {
    node_parent(file.store(), node)
        .as_ref()
        .is_some_and(|parent| file.store().kind(*parent) == ast::Kind::VariableDeclarationList)
        && !is_possibly_type_argument_position(Some(node), file, type_checker)
}

pub fn is_function_like_but_not_constructor(kind: ast::Kind) -> bool {
    ast::is_function_like_kind(kind) && kind != ast::Kind::Constructor
}

pub fn is_previous_property_declaration_terminated(
    context_token: &ast::Node,
    file: &ast::SourceFile,
    position: i32,
) -> bool {
    let store = file.store();
    store.kind(*context_token) != ast::Kind::EqualsToken
        && (store.kind(*context_token) == ast::Kind::SemicolonToken
            || get_line_of_position(file, store.loc(*context_token).end())
                != get_line_of_position(file, position))
}

pub(crate) fn is_dot_of_numeric_literal(context_token: ast::Node, file: &ast::SourceFile) -> bool {
    let store = file.store();
    if store.kind(context_token) == ast::Kind::NumericLiteral {
        let loc = store.loc(context_token);
        let text = &file.text()[loc.pos() as usize..loc.end() as usize];
        return text.chars().next_back() == Some('.');
    }

    false
}

pub fn is_in_jsx_text(
    store: &ast::AstStore,
    context_token: &ast::Node,
    location: &ast::Node,
) -> bool {
    if store.kind(*context_token) == ast::Kind::JsxText {
        return true;
    }

    if store.kind(*context_token) == ast::Kind::GreaterThanToken
        && node_parent(store, context_token).is_some()
    {
        // <Component<string> /**/ />
        // <Component<string> /**/ ><Component>
        // - contextToken: GreaterThanToken (before cursor)
        // - location: JsxSelfClosingElement or JsxOpeningElement
        // - contextToken.parent === location
        if node_parent(store, context_token)
            .as_ref()
            .is_some_and(|parent| *location == *parent)
            && ast::is_jsx_opening_like_element(store, location)
        {
            return false;
        }

        if node_parent(store, context_token)
            .is_some_and(|parent| store.kind(parent) == ast::Kind::JsxOpeningElement)
        {
            // <div>/**/
            // - contextToken: GreaterThanToken (before cursor)
            // - location: JSXElement
            // - different parents (JSXOpeningElement, JSXElement)
            return node_parent(store, location)
                .is_none_or(|parent| store.kind(parent) != ast::Kind::JsxOpeningElement);
        }

        if node_parent(store, context_token).is_some_and(|parent| {
            store.kind(parent) == ast::Kind::JsxClosingElement
                || store.kind(parent) == ast::Kind::JsxSelfClosingElement
        }) {
            return node_parent(store, context_token)
                .and_then(|parent| node_parent(store, parent))
                .is_some_and(|parent| store.kind(parent) == ast::Kind::JsxElement);
        }
    }

    false
}

pub fn client_supports_item_label_details(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .text_document
        .completion
        .completion_item
        .label_details_support
}

pub fn client_supports_item_snippet(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .text_document
        .completion
        .completion_item
        .snippet_support
}

pub fn client_supports_item_commit_characters(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .text_document
        .completion
        .completion_item
        .commit_characters_support
}

pub fn client_supports_item_insert_replace(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .text_document
        .completion
        .completion_item
        .insert_replace_support
}

pub fn client_supports_default_commit_characters(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .text_document
        .completion
        .completion_list
        .item_defaults
        .contains(&"commitCharacters".to_string())
}

pub fn client_supports_default_edit_range(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .text_document
        .completion
        .completion_list
        .item_defaults
        .contains(&"editRange".to_string())
}

pub struct ArgumentInfoForCompletions {
    pub invocation: ast::CallLikeExpression,
    pub argument_index: i32,
    pub argument_count: i32,
}

pub fn get_argument_info_for_completions<'a>(
    node: &ast::Node,
    position: i32,
    file: &'a ast::SourceFile,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<ArgumentInfoForCompletions> {
    let info = get_immediately_containing_argument_info(*node, position, file, type_checker);
    if info.is_none()
        || info.as_ref().unwrap().is_type_parameter_list
        || info.as_ref().unwrap().invocation.call_invocation.is_none()
    {
        return None;
    }
    let info = info.unwrap();
    Some(ArgumentInfoForCompletions {
        invocation: info.invocation.call_invocation.unwrap().node,
        argument_index: info.argument_index,
        argument_count: info.argument_count,
    })
}

pub fn get_completion_documentation_format(ctx: &core::Context) -> lsproto::MarkupKind {
    lsproto::preferred_markup_kind(
        &lsproto::get_client_capabilities(ctx)
            .text_document
            .completion
            .completion_item
            .documentation_format,
    )
}

pub fn create_simple_details<'a>(
    item: &'a mut lsproto::CompletionItem,
    name: &str,
    doc_format: lsproto::MarkupKind,
) -> &'a mut lsproto::CompletionItem {
    create_completion_details(item, name, "" /*documentation*/, doc_format)
}

pub fn create_completion_details<'a>(
    item: &'a mut lsproto::CompletionItem,
    detail: &str,
    documentation: &str,
    doc_format: lsproto::MarkupKind,
) -> &'a mut lsproto::CompletionItem {
    // !!! fill in additionalTextEdits from code actions
    if item.detail.is_none() && !detail.is_empty() {
        item.detail = Some(detail.to_string());
    }
    if !documentation.is_empty() {
        item.documentation = Some(lsproto::Documentation::MarkupContent(
            lsproto::MarkupContent {
                kind: doc_format,
                value: documentation.to_string(),
            },
        ));
    }
    item
}

pub struct CodeAction {
    // Description of the code action to display in the UI of the editor
    pub description: String,
    // Text changes to apply to each file as part of the code action
    pub changes: Vec<lsproto::TextEdit>,
}

pub struct DetailsData<'a> {
    pub symbol: Option<SymbolDetails<'a>>,
    pub request: Option<CompletionData<'a>>,
    pub literal: Option<LiteralValue>,
    pub cases: Option<()>,
}

pub struct SymbolDetails<'a> {
    pub(crate) _marker: std::marker::PhantomData<&'a ()>,
    pub(crate) symbol: CompletionSymbol,
    pub location: Option<ast::Node>,
    pub origin: Option<SymbolOriginInfo>,
    pub previous_token: Option<ast::Node>,
    pub context_token: Option<ast::Node>,
    pub jsx_initializer: JsxInitializer,
    pub is_type_only_location: bool,
}

impl LanguageService<'_> {
    pub fn resolve_completion_item(
        &self,
        ctx: &core::Context,
        item: &mut lsproto::CompletionItem,
        data: Option<&lsproto::CompletionItemData>,
    ) -> Result<lsproto::CompletionItem, core::Error> {
        let Some(data) = data else {
            return Err(core::Error::new("completion item data is nil"));
        };

        let (program, file) = self.try_get_program_and_file(&data.file_name);
        let Some(file) = file else {
            return Err(core::Error::new(format!(
                "file not found: {}",
                data.file_name
            )));
        };

        program.with_type_checker_for_file_using(
            compiler::CheckerAccess::context(ctx),
            file,
            |checker| {
                let result = self.get_completion_item_details(
                    ctx,
                    program,
                    checker,
                    data.position,
                    file,
                    item,
                    data,
                );
                Ok(result.clone())
            },
        )
    }

    pub fn get_completion_item_details<'a>(
        &self,
        ctx: &core::Context,
        program: &compiler::Program,
        checker: &mut checker::Checker<'a, '_>,
        position: i32,
        file: &'a ast::SourceFile,
        item: &'a mut lsproto::CompletionItem,
        data: &'a lsproto::CompletionItemData,
    ) -> &'a mut lsproto::CompletionItem {
        let doc_format = get_completion_documentation_format(ctx);
        let (context_token, previous_token) = get_relevant_tokens(position, file);
        if is_in_string(file, position, previous_token.as_ref()) {
            *item = self.get_string_literal_completion_details(
                ctx,
                checker,
                item.clone(),
                &data.name,
                file,
                position,
                context_token,
                doc_format,
            );
            return item;
        }

        if let Some(auto_import) = &data.auto_import {
            let (edits, description) = autoimport::Fix {
                auto_import_fix: Some(auto_import.clone()),
                ..Default::default()
            }
            .edits(
                ctx.clone(),
                file,
                program.options(),
                self.format_options(),
                &self.converters,
                self.user_preferences(),
            );
            item.additional_text_edits = Some(edits);
            item.detail = str_ptr_to(description);
            return item;
        }

        // Compute all the completion symbols again.
        let symbol_completion =
            self.get_symbol_completion_from_item_data(ctx, checker, file, position, data);
        let preferences = self.user_preferences();

        if let Some(request) = symbol_completion.request {
            match request {
                CompletionData::Keyword(request) => {
                    if request
                        .keyword_completions
                        .iter()
                        .any(|c| c.label == data.name)
                    {
                        return create_simple_details(item, &data.name, doc_format);
                    }
                    return item;
                }
                CompletionData::Data(_) => {
                    panic!("Unexpected completion data type: completionDataData");
                }
            }
        } else if let Some(symbol_details) = symbol_completion.symbol {
            return self.create_completion_details_for_symbol(
                item,
                symbol_details.symbol,
                checker,
                symbol_details.location.as_ref().unwrap(),
                position,
                doc_format,
            );
        } else if let Some(literal) = symbol_completion.literal {
            return create_simple_details(
                item,
                &completion_name_for_literal(file, preferences, &literal),
                doc_format,
            );
        } else if symbol_completion.cases.is_some() {
            return item;
        } else {
            // Didn't find a symbol with this name.  See if we can find a keyword instead.
            if all_keyword_completions()
                .iter()
                .any(|c| c.label == data.name)
            {
                return create_simple_details(item, &data.name, doc_format);
            }
            return item;
        }
    }

    pub fn get_symbol_completion_from_item_data<'a>(
        &self,
        ctx: &core::Context,
        ch: &mut checker::Checker<'a, '_>,
        file: &'a ast::SourceFile,
        position: i32,
        item_data: &lsproto::CompletionItemData,
    ) -> DetailsData<'a> {
        if item_data.source == COMPLETION_SOURCE_SWITCH_CASES {
            return DetailsData {
                cases: Some(()),
                symbol: None,
                request: None,
                literal: None,
            };
        }

        let completion_data = self.get_completion_data(
            ctx,
            ch,
            file,
            position,
            self.user_preferences(),
            true, /*forItemResolve*/
        );
        let Ok(completion_data) = completion_data else {
            panic!("getCompletionData failed during completion item resolve");
        };

        let Some(completion_data) = completion_data else {
            return DetailsData {
                symbol: None,
                request: None,
                literal: None,
                cases: None,
            };
        };

        let CompletionData::Data(data) = completion_data else {
            return DetailsData {
                request: Some(completion_data),
                symbol: None,
                literal: None,
                cases: None,
            };
        };

        let preferences = self.user_preferences();
        for literal in &data.literals {
            if completion_name_for_literal(file, preferences.clone(), literal) == item_data.name {
                return DetailsData {
                    literal: Some(literal.clone()),
                    symbol: None,
                    request: None,
                    cases: None,
                };
            }
        }

        // Find the symbol with the matching entry name.
        // We don't need to perform character checks here because we're only comparing the
        // name against 'entryName' (which is known to be good), not building a new
        // completion entry.
        for (index, symbol) in data.symbols.iter().enumerate() {
            let origin = data.symbol_to_origin_info_map.get(&index).cloned();
            let (display_name, _) = get_completion_entry_display_name_for_symbol(
                file.store(),
                ch,
                *symbol,
                origin.as_ref(),
                data.completion_kind,
                data.is_jsx_identifier_expected,
            );
            if display_name == item_data.name
                && (item_data.source == COMPLETION_SOURCE_CLASS_MEMBER_SNIPPET
                    && completion_symbol_flags(ch, *symbol) & ast::SYMBOL_FLAGS_CLASS_MEMBER
                        != ast::SYMBOL_FLAGS_NONE
                    || item_data.source == COMPLETION_SOURCE_OBJECT_LITERAL_METHOD_SNIPPET
                        && completion_symbol_flags(ch, *symbol)
                            & (ast::SYMBOL_FLAGS_PROPERTY | ast::SYMBOL_FLAGS_METHOD)
                            != ast::SYMBOL_FLAGS_NONE
                    || get_source_from_origin(origin.as_ref()) == item_data.source
                    || item_data.source == COMPLETION_SOURCE_OBJECT_LITERAL_MEMBER_WITH_COMMA)
            {
                return DetailsData {
                    symbol: Some(SymbolDetails {
                        _marker: std::marker::PhantomData,
                        symbol: *symbol,
                        location: data.location,
                        origin,
                        previous_token: data.previous_token,
                        context_token: data.context_token,
                        jsx_initializer: data.jsx_initializer,
                        is_type_only_location: data.is_type_only_location,
                    }),
                    request: None,
                    literal: None,
                    cases: None,
                };
            }
        }
        DetailsData {
            symbol: None,
            request: None,
            literal: None,
            cases: None,
        }
    }

    pub fn get_import_statement_completion_info(
        &self,
        context_token: &ast::Node,
        source_file: &ast::SourceFile,
    ) -> ImportStatementCompletionInfo {
        let store = source_file.store();
        let mut result = ImportStatementCompletionInfo {
            is_keyword_only_completion: false,
            keyword_completion: ast::Kind::Unknown,
            is_new_identifier_location: false,
            is_top_level_type_only: false,
            could_be_type_only_import_specifier: false,
            replacement_span: None,
        };
        let mut candidate: Option<ast::Node> = None;
        let parent = node_parent(store, context_token);
        if parent
            .as_ref()
            .is_some_and(|parent| ast::is_import_equals_declaration(store, *parent))
        {
            // import Foo |
            // import Foo f|
            let parent = parent.unwrap();
            let last_token = lsutil::get_last_token_info(Some(parent), source_file);
            if store.kind(*context_token) == ast::Kind::Identifier
                && last_token
                    .is_some_and(|last_token| !last_token.matches_node(store, *context_token))
            {
                result.keyword_completion = ast::Kind::FromKeyword;
                result.is_keyword_only_completion = true;
            } else {
                if store.kind(*context_token) != ast::Kind::TypeKeyword {
                    result.keyword_completion = ast::Kind::TypeKeyword;
                }
                if is_module_specifier_missing_or_empty(
                    store,
                    store.module_reference(parent).as_ref(),
                ) {
                    candidate = Some(parent);
                }
            }
        } else if parent
            .as_ref()
            .is_some_and(|parent| could_be_type_only_import_specifier(store, parent, context_token))
            && parent
                .as_ref()
                .and_then(|parent| node_parent(store, parent))
                .as_ref()
                .is_some_and(|parent| can_complete_from_named_bindings(store, source_file, parent))
        {
            candidate = parent;
        } else if parent
            .as_ref()
            .is_some_and(|parent| ast::is_named_imports(store, *parent))
            || parent
                .as_ref()
                .is_some_and(|parent| ast::is_namespace_import(store, *parent))
        {
            let parent = parent.unwrap();
            if !node_parent(store, parent)
                .as_ref()
                .is_some_and(|parent| store.is_type_only(*parent).unwrap_or(false))
                && (store.kind(*context_token) == ast::Kind::OpenBraceToken
                    || store.kind(*context_token) == ast::Kind::ImportKeyword
                    || store.kind(*context_token) == ast::Kind::CommaToken)
            {
                result.keyword_completion = ast::Kind::TypeKeyword;
            }
            if can_complete_from_named_bindings(store, source_file, &parent) {
                // At `import { ... } |` or `import * as Foo |`, the only possible completion is `from`
                if store.kind(*context_token) == ast::Kind::CloseBraceToken
                    || store.kind(*context_token) == ast::Kind::Identifier
                {
                    result.is_keyword_only_completion = true;
                    result.keyword_completion = ast::Kind::FromKeyword;
                } else {
                    candidate =
                        node_parent(store, &parent).and_then(|parent| node_parent(store, &parent));
                }
            }
        } else if (parent
            .as_ref()
            .is_some_and(|parent| ast::is_export_declaration(store, *parent))
            && store.kind(*context_token) == ast::Kind::AsteriskToken)
            || (parent
                .as_ref()
                .is_some_and(|parent| ast::is_named_exports(store, *parent))
                && store.kind(*context_token) == ast::Kind::CloseBraceToken)
        {
            result.is_keyword_only_completion = true;
            result.keyword_completion = ast::Kind::FromKeyword;
        } else if store.kind(*context_token) == ast::Kind::ImportKeyword {
            if parent
                .as_ref()
                .is_some_and(|parent| ast::is_source_file(store, *parent))
            {
                // A lone import keyword with nothing following it does not parse as a statement at all
                result.keyword_completion = ast::Kind::TypeKeyword;
                candidate = Some(*context_token);
            } else if parent
                .as_ref()
                .is_some_and(|parent| ast::is_import_declaration(store, *parent))
            {
                let parent = parent.unwrap();
                // `import s| from`
                result.keyword_completion = ast::Kind::TypeKeyword;
                if is_module_specifier_missing_or_empty(
                    store,
                    node_module_specifier(store, parent).as_ref(),
                ) {
                    candidate = Some(parent);
                }
            }
        }

        if let Some(candidate) = candidate {
            result.is_new_identifier_location = true;
            result.replacement_span = self
                .get_single_line_replacement_span_for_import_completion_node(
                    source_file,
                    &candidate,
                );
            result.could_be_type_only_import_specifier =
                could_be_type_only_import_specifier(store, &candidate, context_token);
            if ast::is_import_declaration(store, candidate) {
                if let Some(import_clause) = store.import_clause(candidate) {
                    result.is_top_level_type_only =
                        store.is_type_only(import_clause).unwrap_or(false);
                }
            } else if store.kind(candidate) == ast::Kind::ImportEqualsDeclaration {
                result.is_top_level_type_only = store.is_type_only(candidate).unwrap_or(false);
            }
        } else {
            result.is_new_identifier_location = result.keyword_completion == ast::Kind::TypeKeyword;
        }
        result
    }

    pub fn get_single_line_replacement_span_for_import_completion_node(
        &self,
        source_file: &ast::SourceFile,
        node: &ast::Node,
    ) -> Option<lsproto::Range> {
        let store = source_file.store();
        let node = ast::find_ancestor(store, Some(*node), |store, n| {
            ast::is_import_declaration(store, n) || ast::is_import_equals_declaration(store, n)
        })
        .unwrap_or(*node);
        // Use token position instead of node.Pos() to avoid including trivia.
        let token_pos = scanner::get_token_pos_of_node(&node, source_file, false);
        if printer::get_lines_between_positions(
            source_file,
            token_pos as i32,
            store.loc(node).end(),
        ) == 0
        {
            return Some(self.create_lsp_range_from_node(node, source_file));
        }

        if store.kind(node) == ast::Kind::ImportKeyword
            || store.kind(node) == ast::Kind::ImportSpecifier
        {
            panic!(
                "ImportKeyword was necessarily on one line; ImportSpecifier was necessarily parented in an ImportDeclaration"
            );
        }

        // Guess which point in the import might actually be a later statement parsed as part of the import
        // during parser recovery - either in the middle of named imports, or the module specifier.
        let potential_split_point = if store.kind(node) == ast::Kind::ImportDeclaration {
            let mut specifier: Option<ast::Node> = None;
            if let Some(import_clause) = store.import_clause(node) {
                if let Some(named_bindings) = store.named_bindings(import_clause) {
                    specifier = get_potentially_invalid_import_specifier(
                        store,
                        source_file,
                        &named_bindings,
                    );
                }
            }
            let module_specifier = node_module_specifier(store, &node);
            specifier.or(module_specifier)
        } else {
            store.module_reference(node)
        };

        let Some(potential_split_point) = potential_split_point else {
            return None;
        };
        let first_token = lsutil::get_first_token_info(Some(node), source_file)?;
        let without_module_specifier = core::new_text_range(
            first_token.loc.pos(),
            store.loc(potential_split_point).pos(),
        );
        // The module specifier/reference was previously found to be missing, empty, or
        // not a string literal - in this last case, it's likely that statement on a following
        // line was parsed as the module specifier of a partially-typed import, e.g.
        //   import Foo|
        //   interface Blah {}
        // This appears to be a multiline-import, and editors can't replace multiple lines.
        // But if everything but the "module specifier" is on one line, by this point we can
        // assume that the "module specifier" is actually just another statement, and return
        // the single-line range of the import excluding that probable statement.
        if printer::get_lines_between_positions(
            source_file,
            without_module_specifier.pos(),
            without_module_specifier.end(),
        ) == 0
        {
            return Some(self.create_lsp_range_from_bounds(
                without_module_specifier.pos(),
                without_module_specifier.end(),
                source_file,
            ));
        }
        None
    }
}

pub fn could_be_type_only_import_specifier(
    store: &ast::AstStore,
    import_specifier: &ast::Node,
    context_token: &ast::Node,
) -> bool {
    ast::is_import_specifier(store, *import_specifier)
        && (store.is_type_only(*import_specifier).unwrap_or(false)
            || node_name(store, import_specifier)
                .as_ref()
                .is_some_and(|name| *context_token == *name)
                && is_type_keyword_token_or_identifier(store, *context_token))
}

pub fn can_complete_from_named_bindings(
    store: &ast::AstStore,
    source_file: &ast::SourceFile,
    named_bindings: &ast::Node,
) -> bool {
    let import_clause = node_parent(store, named_bindings);
    if !is_module_specifier_missing_or_empty(
        store,
        import_clause
            .as_ref()
            .and_then(|parent| node_parent(store, parent))
            .and_then(|parent| node_module_specifier(store, parent))
            .as_ref(),
    ) || import_clause
        .as_ref()
        .and_then(|parent| node_name(store, parent))
        .is_some()
    {
        return false;
    }
    if ast::is_named_imports(store, *named_bindings) {
        // We can only complete on named imports if there are no other named imports already,
        // but parser recovery sometimes puts later statements in the named imports list, so
        // we try to only consider the probably-valid ones.
        let invalid_named_import =
            get_potentially_invalid_import_specifier(store, source_file, named_bindings);
        let elements = node_elements(store, named_bindings);
        let mut valid_imports = elements.len() as i32;
        if let Some(invalid_named_import) = invalid_named_import {
            valid_imports = elements
                .iter()
                .position(|element| *element == invalid_named_import)
                .map(|index| index as i32)
                .unwrap_or(-1);
        }

        return valid_imports < 2 && valid_imports > -1;
    }
    true
}

// Tries to identify the first named import that is not really a named import, but rather
// just parser recovery for a situation like:
//
//	import { Foo|
//	interface Bar {}
//
// in which `Foo`, `interface`, and `Bar` are all parsed as import specifiers. The caller
// will also check if this token is on a separate line from the rest of the import.
pub fn get_potentially_invalid_import_specifier(
    store: &ast::AstStore,
    source_file: &ast::SourceFile,
    named_bindings: &ast::Node,
) -> Option<ast::Node> {
    if store.kind(*named_bindings) != ast::Kind::NamedImports {
        return None;
    }
    let elements = node_elements(store, named_bindings);
    elements.into_iter().find_map(|e| {
        (node_property_name(store, e).is_none()
            && lsutil::is_non_contextual_keyword(scanner::string_to_token(&node_text(
                store,
                node_name(store, e).unwrap(),
            )))
            && astnav::find_preceding_token(
                source_file,
                store.loc(node_name(store, e).unwrap()).pos(),
            )
            .is_some_and(|token| store.kind(token) != ast::Kind::CommaToken))
        .then_some(e)
    })
}

pub fn is_module_specifier_missing_or_empty(
    store: &ast::AstStore,
    specifier: Option<&ast::Expression>,
) -> bool {
    let Some(node) = specifier else {
        return true;
    };
    if ast::node_is_missing(store, Some(*node)) {
        return true;
    }
    let expression_storage;
    let mut node = *node;
    if ast::is_external_module_reference(store, node) {
        let Some(expression) = node_expression(store, &node) else {
            return true;
        };
        expression_storage = expression;
        node = expression_storage;
    }
    if !ast::is_string_literal_like(store, node) {
        return true;
    }
    node_text(store, &node).is_empty()
}

pub fn is_possibly_type_argument_position<'a>(
    token: Option<&ast::Node>,
    source_file: &ast::SourceFile,
    type_checker: &mut checker::Checker<'a, '_>,
) -> bool {
    let Some(token) = token else {
        return false;
    };
    let info = get_possible_type_arguments_info(token, source_file);
    info.is_some_and(|info| {
        ast::is_part_of_type_node(source_file.store(), info.called)
            || !get_possible_generic_signatures(
                source_file.store(),
                &info.called,
                info.n_type_arguments,
                type_checker,
            )
            .is_empty()
            || is_possibly_type_argument_position(Some(&info.called), source_file, type_checker)
    })
}

// True if symbol is a type or a module containing at least one type.
pub(crate) fn symbol_can_be_referenced_at_type_location(
    symbol: CompletionSymbol,
    type_checker: &mut checker::Checker,
    seen_modules: collections::Set<CompletionSymbol>,
) -> bool {
    let alias_target_referenced =
        if let Some(export_symbol) = type_checker.symbol_export_symbol_public(symbol) {
            non_alias_can_be_referenced_at_type_location(
                type_checker
                    .skip_alias_public(export_symbol)
                    .unwrap_or(export_symbol),
                type_checker,
                seen_modules.clone(),
            )
        } else {
            non_alias_can_be_referenced_at_type_location(
                type_checker.skip_alias_public(symbol).unwrap_or(symbol),
                type_checker,
                seen_modules.clone(),
            )
        };
    non_alias_can_be_referenced_at_type_location(symbol, type_checker, seen_modules.clone())
        || alias_target_referenced
}

pub(crate) fn non_alias_can_be_referenced_at_type_location(
    symbol: CompletionSymbol,
    type_checker: &mut checker::Checker,
    mut seen_modules: collections::Set<CompletionSymbol>,
) -> bool {
    let flags = completion_symbol_flags(type_checker, symbol);
    flags & ast::SYMBOL_FLAGS_TYPE != ast::SYMBOL_FLAGS_NONE
        || type_checker.is_unknown_symbol(symbol)
        || flags & ast::SYMBOL_FLAGS_MODULE != ast::SYMBOL_FLAGS_NONE
            && seen_modules.add_if_absent(symbol)
            && type_checker
                .get_exports_of_module_public(symbol)
                .iter()
                .any(|e| {
                    symbol_can_be_referenced_at_type_location(
                        *e,
                        type_checker,
                        seen_modules.clone(),
                    )
                })
}

pub(crate) fn get_contextual_type_for_conditional_expression<'a>(
    conditional_expr: &ast::Node,
    position: i32,
    file: &'a ast::SourceFile,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<checker::TypeHandle> {
    let arg_info =
        get_argument_info_for_completions(conditional_expr, position, file, type_checker);
    if let Some(arg_info) = arg_info {
        return type_checker.get_contextual_type_for_argument_at_index_public(
            arg_info.invocation,
            arg_info.argument_index as isize,
        );
    }
    let contextual_type = type_checker.get_contextual_type_public(
        *conditional_expr,
        checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES,
    );
    if contextual_type.is_some() {
        return contextual_type;
    }
    type_checker.get_contextual_type_public(*conditional_expr, checker::CONTEXT_FLAGS_NONE)
}

pub(crate) fn get_contextual_type<'a>(
    previous_token: &ast::Node,
    position: i32,
    file: &'a ast::SourceFile,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<checker::TypeHandle> {
    // PORT NOTE: reshaped for borrowck; parent accessors may return owned handles in this port.
    let store = file.store();
    let parent = node_parent(store, previous_token);
    match store.kind(*previous_token) {
        ast::Kind::Identifier => {
            return get_contextual_type_from_parent(
                store,
                *previous_token,
                type_checker,
                checker::CONTEXT_FLAGS_NONE,
            );
        }
        ast::Kind::EqualsToken => match parent.map(|parent| store.kind(parent)) {
            Some(ast::Kind::VariableDeclaration) => {
                let parent = parent?;
                let initializer = node_initializer(store, parent)?;
                return type_checker
                    .get_contextual_type_public(initializer, checker::CONTEXT_FLAGS_NONE);
            }
            Some(ast::Kind::BinaryExpression) => {
                let parent = parent?;
                let left = store.left(parent)?;
                return Some(type_checker.get_type_at_location(left));
            }
            Some(ast::Kind::JsxAttribute) => {
                let parent = parent?;
                return type_checker.get_contextual_type_for_jsx_attribute_public(
                    parent,
                    checker::CONTEXT_FLAGS_NONE,
                );
            }
            _ => return None,
        },
        ast::Kind::NewKeyword => {
            return parent.and_then(|parent| {
                type_checker.get_contextual_type_public(parent, checker::CONTEXT_FLAGS_NONE)
            });
        }
        ast::Kind::CaseKeyword => {
            let case_clause = if parent.is_some_and(|parent| ast::is_case_clause(store, parent)) {
                parent
            } else {
                None
            };
            if let Some(case_clause) = case_clause {
                return get_switched_type(store, &case_clause, type_checker);
            }
            return None;
        }
        ast::Kind::OpenBraceToken => {
            if let Some(parent) = parent
                && ast::is_jsx_expression(store, parent)
                && !node_parent(store, parent)
                    .as_ref()
                    .is_some_and(|parent| ast::is_jsx_element(store, *parent))
                && !node_parent(store, parent)
                    .as_ref()
                    .is_some_and(|parent| ast::is_jsx_fragment(store, *parent))
            {
                return node_parent(store, parent).and_then(|parent| {
                    type_checker.get_contextual_type_for_jsx_attribute_public(
                        parent,
                        checker::CONTEXT_FLAGS_NONE,
                    )
                });
            }
            return None;
        }
        ast::Kind::OpenBracketToken => {
            // When completing after `[` in an array literal (e.g., `[/*here*/]`),
            // we should provide contextual type for the first element
            if let Some(parent) = parent
                && ast::is_array_literal_expression(store, parent)
            {
                let contextual_array_type =
                    type_checker.get_contextual_type_public(parent, checker::CONTEXT_FLAGS_NONE);
                if let Some(contextual_array_type) = contextual_array_type {
                    // Get the type for the first element (index 0)
                    return type_checker.get_contextual_type_for_array_literal_at_position(
                        Some(contextual_array_type),
                        parent,
                        position as usize,
                    );
                }
            }
            return None;
        }
        ast::Kind::CloseBracketToken => {
            // When completing after `]` (e.g., `[x]/*here*/`), we should not provide a contextual type
            // for the closing bracket token itself. Without this case, CloseBracketToken would fall through
            // to the default case, and if the parent is an array literal, GetContextualType would try to
            // find the token's index in the array elements (returning -1), leading to an out-of-bounds panic
            // in getContextualTypeForElementExpression.
            return None;
        }
        ast::Kind::QuestionToken => {
            // When completing after `?` in a ternary conditional (e.g., `foo(a ? /*here*/)`),
            // we need to look at the parent conditional expression to find the contextual type.
            if let Some(parent) = parent
                && ast::is_conditional_expression(store, parent)
            {
                return get_contextual_type_for_conditional_expression(
                    &parent,
                    position,
                    file,
                    type_checker,
                );
            }
            return None;
        }
        ast::Kind::ColonToken => {
            // When completing after `:` in a ternary conditional (e.g., `foo(a ? b : /*here*/)`),
            // we need to look at the parent conditional expression to find the contextual type.
            // Only handle this if parent is ConditionalExpression, otherwise fall through to default
            // (colons are used in other contexts like object literals, type annotations, etc.)
            if let Some(parent) = parent
                && ast::is_conditional_expression(store, parent)
            {
                return get_contextual_type_for_conditional_expression(
                    &parent,
                    position,
                    file,
                    type_checker,
                );
            }
        }
        ast::Kind::CommaToken => {
            // When completing after `,` in an array literal (e.g., `[x, /*here*/]`),
            // we should provide contextual type for the element after the comma.
            if let Some(parent) = parent
                && ast::is_array_literal_expression(store, parent)
            {
                let contextual_array_type =
                    type_checker.get_contextual_type_public(parent, checker::CONTEXT_FLAGS_NONE);
                if let Some(contextual_array_type) = contextual_array_type {
                    return type_checker.get_contextual_type_for_array_literal_at_position(
                        Some(contextual_array_type),
                        parent,
                        position as usize,
                    );
                }
                return None;
            }
        }
        _ => {}
    }
    // Default case: see if we're in an argument position.
    let arg_info = get_argument_info_for_completions(previous_token, position, file, type_checker);
    if let Some(arg_info) = arg_info {
        type_checker.get_contextual_type_for_argument_at_index_public(
            arg_info.invocation,
            arg_info.argument_index as isize,
        )
    } else if is_equality_operator_kind(store.kind(*previous_token))
        && parent.is_some_and(|parent| ast::is_binary_expression(store, parent))
        && parent.is_some_and(|parent| {
            store
                .operator_token(parent)
                .is_some_and(|operator_token| is_equality_operator_kind(store.kind(operator_token)))
        })
    {
        // completion at `x ===/**/`
        let left = store.left(parent.unwrap())?;
        Some(type_checker.get_type_at_location(left))
    } else {
        let contextual_type = type_checker.get_contextual_type_public(
            *previous_token,
            checker::CONTEXT_FLAGS_IGNORE_NODE_INFERENCES,
        );
        if contextual_type.is_some() {
            return contextual_type;
        }
        type_checker.get_contextual_type_public(*previous_token, checker::CONTEXT_FLAGS_NONE)
    }
}

pub(crate) fn get_switched_type<'a>(
    store: &ast::AstStore,
    case_clause: &ast::Node,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<checker::TypeHandle> {
    let switch_statement =
        node_parent(store, case_clause).and_then(|parent| node_parent(store, parent))?;
    let expression = node_expression(store, switch_statement)?;
    Some(type_checker.get_type_at_location(expression))
}

fn get_recommended_completion<'a>(
    store: &ast::AstStore,
    previous_token: &ast::Node,
    contextual_type: checker::TypeHandle,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<CompletionSymbol> {
    let types = if type_checker.is_union_type_public(contextual_type) {
        type_checker.type_types_public(contextual_type)
    } else {
        vec![contextual_type]
    };
    // For a union, return the first one with a recommended completion.
    types.into_iter().find_map(|t| {
        let symbol = type_checker.type_symbol_public(t);
        if let Some(symbol) = symbol {
            let can_recommend = completion_symbol_flags(type_checker, symbol)
                & (ast::SYMBOL_FLAGS_ENUM_MEMBER
                    | ast::SYMBOL_FLAGS_ENUM
                    | ast::SYMBOL_FLAGS_CLASS)
                != ast::SYMBOL_FLAGS_NONE
                && !is_abstract_constructor_symbol(store, type_checker, symbol);
            if can_recommend {
                return get_first_symbol_in_chain(symbol, previous_token, type_checker);
            }
        }
        None
    })
}

pub fn is_in_type_parameter_default(
    store: &ast::AstStore,
    context_token: Option<&ast::Node>,
) -> bool {
    let Some(context_token) = context_token else {
        return false;
    };

    let mut node = *context_token;
    let mut parent = node_parent(store, context_token);
    while let Some(parent_node) = parent {
        if ast::is_type_parameter_declaration(store, parent_node) {
            return store
                .default_type(parent_node)
                .as_ref()
                .is_some_and(|default_type| *default_type == node)
                || store.kind(node) == ast::Kind::EqualsToken;
        }
        node = parent_node;
        parent = node_parent(store, parent_node);
    }

    false
}

// Checks whether type is `string & {}`, which is semantically equivalent to string but
// is not reduced by the checker as a special case used for supporting string literal completions
// for string type.
fn is_string_and_empty_anonymous_object_intersection<'a>(
    type_checker: &mut checker::Checker<'a, '_>,
    t: checker::TypeHandle,
) -> bool {
    if !type_checker.is_intersection_type_public(t) {
        return false;
    }

    let types = type_checker.type_types_public(t);
    types.len() == 2
        && (are_intersected_types_avoiding_string_reduction(type_checker, types[0], types[1])
            || are_intersected_types_avoiding_string_reduction(type_checker, types[1], types[0]))
}

fn are_intersected_types_avoiding_string_reduction<'a>(
    type_checker: &mut checker::Checker<'a, '_>,
    t1: checker::TypeHandle,
    t2: checker::TypeHandle,
) -> bool {
    let t2 = t2;
    type_checker.is_string_type_public(t1) && type_checker.is_empty_anonymous_object_type_public(t2)
}

pub fn try_get_type_literal_node(
    store: &ast::AstStore,
    node: Option<&ast::Node>,
) -> Option<ast::Node> {
    let node = *node?;

    let parent = node_parent(store, node);
    match store.kind(node) {
        ast::Kind::OpenBraceToken => {
            if let Some(parent) = parent
                && ast::is_type_literal_node(store, parent)
            {
                return Some(parent);
            }
        }
        ast::Kind::SemicolonToken | ast::Kind::CommaToken | ast::Kind::Identifier => {
            if let Some(parent) = parent
                && store.kind(parent) == ast::Kind::PropertySignature
                && node_parent(store, parent)
                    .as_ref()
                    .is_some_and(|parent| ast::is_type_literal_node(store, *parent))
            {
                return node_parent(store, parent);
            }
        }
        _ => {}
    }

    None
}

pub(crate) fn get_constraint_of_type_argument_property<'a>(
    store: &ast::AstStore,
    node: Option<&ast::Node>,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<checker::TypeHandle> {
    let node = node?;

    if ast::is_type_node(store, *node) {
        let constraint = type_checker.get_type_argument_constraint_public(*node);
        if constraint.is_some() {
            return constraint;
        }
    }

    let parent = node_parent(store, node);
    let t = get_constraint_of_type_argument_property(store, parent.as_ref(), type_checker)?;

    match store.kind(*node) {
        ast::Kind::PropertySignature => {
            let reparsed = ast::get_reparsed_node_for_node(store, node);
            if let Some(symbol) = node_symbol_identity(type_checker, reparsed) {
                let name = completion_symbol_name(type_checker, symbol);
                return type_checker.get_type_of_property_of_contextual_type_public(t, &name);
            }

            // In some cases, we won't have a corresponding symbol, so use the
            // name as declared by the property as a best effort.
            if let Some(name) = node_name(store, reparsed) {
                let (name, ok) = ast::try_get_text_of_property_name(store, name);
                if ok {
                    return type_checker.get_type_of_property_of_contextual_type_public(t, &name);
                }
            }

            None
        }
        ast::Kind::ColonToken => {
            if node_parent(store, node)
                .as_ref()
                .is_some_and(|parent| store.kind(*parent) == ast::Kind::PropertySignature)
            {
                // The cursor is at a property value location like `Foo<{ x: | }`.
                // `t` already refers to the appropriate property type.
                return Some(t);
            }
            None
        }
        ast::Kind::IntersectionType | ast::Kind::TypeLiteral | ast::Kind::UnionType => Some(t),
        ast::Kind::OpenBracketToken => type_checker.get_element_type_of_array_type_public(t),
        _ => None,
    }
}

pub(crate) fn try_get_object_literal_contextual_type<'a>(
    store: &ast::AstStore,
    node: &ast::Node,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Option<checker::TypeHandle> {
    let t = (type_checker).get_contextual_type_public(*node, checker::CONTEXT_FLAGS_NONE);
    if t.is_some() {
        return t;
    }

    let node_parent = node_parent(store, node);
    let parent = ast::walk_up_parenthesized_expressions(store, node_parent);
    if parent
        .as_ref()
        .is_some_and(|parent| ast::is_binary_expression(store, *parent))
        && parent
            .as_ref()
            .and_then(|parent| store.operator_token(*parent))
            .is_some_and(|operator_token| store.kind(operator_token) == ast::Kind::EqualsToken)
        && parent
            .as_ref()
            .and_then(|parent| store.left(*parent))
            .as_ref()
            .is_some_and(|left| *node == *left)
    {
        // Object literal is assignment pattern: ({ | } = x)
        return Some(type_checker.get_type_at_location(parent?));
    }
    if parent
        .as_ref()
        .is_some_and(|parent| ast::is_expression(store, *parent))
    {
        // f(() => (({ | })));
        return type_checker.get_contextual_type_public(parent?, checker::CONTEXT_FLAGS_NONE);
    }

    None
}

pub(crate) fn get_properties_for_object_expression<'a>(
    store: &ast::AstStore,
    contextual_type: checker::TypeHandle,
    completions_type: Option<checker::TypeHandle>,
    obj: &ast::Node,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Vec<CompletionSymbol> {
    let has_completions_type =
        completions_type.is_some_and(|completions_type| completions_type != contextual_type);
    let types = if type_checker.is_union_type_public(contextual_type) {
        type_checker.type_types_public(contextual_type)
    } else {
        vec![contextual_type]
    };
    let mut non_promise_types = Vec::new();
    for t in types {
        if type_checker
            .get_promised_type_of_promise_public(t)
            .is_none()
        {
            non_promise_types.push(t);
        }
    }
    let promise_filtered_contextual_type = type_checker.get_union_type_public(non_promise_types);

    let t = if has_completions_type
        && type_checker.type_flags_public(completions_type.unwrap())
            & checker::TYPE_FLAGS_ANY_OR_UNKNOWN
            == checker::TYPE_FLAGS_NONE
    {
        type_checker.get_union_type_public(vec![
            promise_filtered_contextual_type,
            completions_type.unwrap(),
        ])
    } else {
        promise_filtered_contextual_type
    };

    // Filter out members whose only declaration is the object literal itself to avoid
    // self-fulfilling completions like:
    //
    // function f<T>(x: T) {}
    // f({ abc/**/: "" }) // `abc` is a member of `T` but only because it declares itself
    let properties = get_apparent_properties(store, t, obj, type_checker);
    if type_checker.is_class_type_public(t)
        && contains_non_public_properties(store, &properties, type_checker)
    {
        Vec::new()
    } else if has_completions_type {
        properties
            .into_iter()
            .filter(|member| {
                let declarations = completion_symbol_declarations(type_checker, *member);
                declarations.is_empty()
                    || declarations
                        .iter()
                        .any(|decl| node_parent(store, decl).is_none_or(|parent| parent != *obj))
            })
            .collect()
    } else {
        properties
    }
}

// Filters out members that are already declared in the object literal or binding pattern.
// Also computes the set of existing members declared by spread assignment.
pub(crate) fn filter_object_members_list<'a>(
    contextual_member_symbols: Vec<CompletionSymbol>,
    existing_members: Vec<&ast::Declaration>,
    file: &ast::SourceFile,
    position: i32,
    type_checker: &mut checker::Checker,
) -> (Vec<CompletionSymbol>, collections::Set<String>) {
    let store = file.store();
    if existing_members.is_empty() {
        return (contextual_member_symbols, collections::Set::new());
    }

    let mut members_declared_by_spread_assignment = collections::Set::new();
    let mut existing_member_names = collections::Set::new();
    for member in existing_members {
        // Ignore omitted expressions for missing members.
        if store.kind(*member) != ast::Kind::PropertyAssignment
            && store.kind(*member) != ast::Kind::ShorthandPropertyAssignment
            && store.kind(*member) != ast::Kind::BindingElement
            && store.kind(*member) != ast::Kind::MethodDeclaration
            && store.kind(*member) != ast::Kind::GetAccessor
            && store.kind(*member) != ast::Kind::SetAccessor
            && store.kind(*member) != ast::Kind::SpreadAssignment
        {
            continue;
        }

        // If this is the current item we are editing right now, do not filter it out.
        if is_currently_editing_node(member, file, position) {
            continue;
        }

        let mut existing_name = String::new();

        if ast::is_spread_assignment(store, *member) {
            set_member_declared_by_spread_assignment(
                store,
                member,
                &mut members_declared_by_spread_assignment,
                type_checker,
            );
        } else if ast::is_binding_element(store, *member)
            && node_property_name(store, member).is_some()
        {
            // include only identifiers in completion list
            let property_name = node_property_name(store, member).unwrap();
            if store.kind(property_name) == ast::Kind::Identifier {
                existing_name = node_text(store, property_name).to_string();
            }
        } else {
            // TODO: Account for computed property name
            // NOTE: if one only performs this step when m.name is an identifier,
            // things like '__proto__' are not filtered out.
            let name = ast::get_name_of_declaration(store, Some(*member));
            if name
                .as_ref()
                .is_some_and(|name| ast::is_property_name_literal(store, *name))
            {
                existing_name = node_text(store, name.unwrap()).to_string();
            }
        }

        if !existing_name.is_empty() {
            existing_member_names.add(existing_name);
        }
    }

    let filtered_symbols = contextual_member_symbols
        .into_iter()
        .filter(|m| !existing_member_names.has(&completion_symbol_name(type_checker, *m)))
        .collect();

    (filtered_symbols, members_declared_by_spread_assignment)
}

// Returns the immediate owning class declaration of a context token,
// on the condition that one exists and that the context implies completion should be given.
pub fn try_get_constructor_like_completion_container(
    store: &ast::AstStore,
    context_token: Option<&ast::Node>,
) -> Option<ast::Node> {
    let context_token = context_token?;

    let parent = node_parent(store, context_token);
    match store.kind(*context_token) {
        ast::Kind::OpenParenToken | ast::Kind::CommaToken => {
            if parent
                .as_ref()
                .is_some_and(|parent| ast::is_constructor_declaration(store, *parent))
            {
                return parent;
            }
            None
        }
        _ => {
            if is_constructor_parameter_completion(store, *context_token) {
                return parent.and_then(|parent| node_parent(store, &parent));
            }
            None
        }
    }
}

pub fn try_get_function_like_body_completion_container(
    store: &ast::AstStore,
    context_token: Option<&ast::Node>,
) -> Option<ast::Node> {
    let context_token = context_token?;

    // PORT NOTE: reshaped for borrowck; track node identity by stable syntax coordinates
    // instead of carrying a borrowed node out of the ancestor callback.
    let mut prev = None;
    ast::find_ancestor_or_quit(store, Some(*context_token), |store, node| {
        if ast::is_class_like(store, node) {
            return ast::FindAncestorResult::Quit;
        }
        if ast::is_function_like_declaration(store, Some(node))
            && prev.is_some_and(|(kind, pos, end)| {
                node_body(store, node).is_some_and(|body| {
                    store.kind(body) == kind
                        && store.loc(body).pos() == pos
                        && store.loc(body).end() == end
                })
            })
        {
            return ast::FindAncestorResult::True;
        }
        prev = Some((
            store.kind(node),
            store.loc(node).pos(),
            store.loc(node).end(),
        ));
        ast::FindAncestorResult::False
    })
}

pub fn try_get_object_like_completion_container(
    store: &ast::AstStore,
    context_token: Option<&astnav::TokenInfo>,
    position: i32,
    file: &ast::SourceFile,
) -> Option<ast::Node> {
    let context_token = context_token?;

    let parent = context_token.parent;
    match context_token.kind {
        // const x = { |
        // const x = { a: 0, |
        ast::Kind::OpenBraceToken | ast::Kind::CommaToken => {
            if parent
                .as_ref()
                .is_some_and(|parent| ast::is_object_literal_expression(store, *parent))
                || parent
                    .as_ref()
                    .is_some_and(|parent| ast::is_object_binding_pattern(store, *parent))
            {
                return parent;
            }
        }
        ast::Kind::AsteriskToken => {
            if let Some(parent) = parent
                && ast::is_method_declaration(store, parent)
                && node_parent(store, parent)
                    .as_ref()
                    .is_some_and(|parent| ast::is_object_literal_expression(store, *parent))
            {
                return node_parent(store, &parent);
            }
        }
        ast::Kind::AsyncKeyword => {
            if let Some(parent) = parent
                && node_parent(store, parent)
                    .as_ref()
                    .is_some_and(|parent| ast::is_object_literal_expression(store, *parent))
            {
                return node_parent(store, &parent);
            }
        }
        ast::Kind::Identifier => {
            if token_info_text_matches(store, context_token, file, "async")
                && parent
                    .as_ref()
                    .is_some_and(|parent| ast::is_shorthand_property_assignment(store, *parent))
            {
                return parent
                    .as_ref()
                    .and_then(|parent| node_parent(store, parent));
            } else {
                if let Some(parent) = parent.as_ref()
                    && node_parent(store, parent)
                        .as_ref()
                        .is_some_and(|parent| ast::is_object_literal_expression(store, *parent))
                    && (ast::is_spread_assignment(store, *parent)
                        || ast::is_shorthand_property_assignment(store, *parent)
                            && get_line_of_position(file, context_token.loc.end())
                                != get_line_of_position(file, position))
                {
                    return node_parent(store, parent);
                }
                let mut ancestor_node = parent;
                while let Some(ancestor) = ancestor_node {
                    if ast::is_property_assignment(store, ancestor) {
                        if lsutil::get_last_token_info(Some(ancestor), file).is_some_and(|token| {
                            context_token
                                .node
                                .is_some_and(|node| token.matches_node(store, node))
                                || token.kind == context_token.kind
                                    && token.loc == context_token.loc
                        }) && node_parent(store, ancestor)
                            .as_ref()
                            .is_some_and(|parent| ast::is_object_literal_expression(store, *parent))
                        {
                            return node_parent(store, &ancestor);
                        }
                        break;
                    }
                    ancestor_node = node_parent(store, ancestor);
                }
            }
        }
        _ => {
            if let Some(parent) = parent.as_ref()
                && let Some(grandparent) = node_parent(store, parent)
                && let Some(great_grandparent) = node_parent(store, grandparent)
                && (ast::is_method_declaration(store, grandparent)
                    || ast::is_get_accessor_declaration(store, grandparent)
                    || ast::is_set_accessor_declaration(store, grandparent))
                && ast::is_object_literal_expression(store, great_grandparent)
            {
                return Some(great_grandparent);
            }
            if let Some(parent) = parent.as_ref()
                && ast::is_spread_assignment(store, *parent)
                && node_parent(store, parent)
                    .as_ref()
                    .is_some_and(|parent| ast::is_object_literal_expression(store, *parent))
            {
                return node_parent(store, parent);
            }
            let mut ancestor_node = parent;
            while let Some(ancestor) = ancestor_node {
                if ast::is_property_assignment(store, ancestor) {
                    if context_token.kind != ast::Kind::ColonToken
                        && lsutil::get_last_token_info(Some(ancestor), file).is_some_and(|token| {
                            context_token
                                .node
                                .is_some_and(|node| token.matches_node(store, node))
                                || token.kind == context_token.kind
                                    && token.loc == context_token.loc
                        })
                        && node_parent(store, ancestor)
                            .as_ref()
                            .is_some_and(|parent| ast::is_object_literal_expression(store, *parent))
                    {
                        return node_parent(store, &ancestor);
                    }
                    break;
                }
                ancestor_node = node_parent(store, ancestor);
            }
        }
    }

    None
}

fn token_info_text_matches(
    store: &ast::AstStore,
    token: &astnav::TokenInfo,
    file: &ast::SourceFile,
    text: &str,
) -> bool {
    if let Some(node) = token.node {
        return node_text(store, &node) == text;
    }
    let start = token.loc.pos().max(0) as usize;
    let end = token.loc.end().max(token.loc.pos()).max(0) as usize;
    file.text()
        .get(start..end)
        .is_some_and(|token_text| token_text.trim_start() == text)
}

// Filters out completion suggestions for class elements.
pub(crate) fn filter_class_members_list<'a>(
    base_symbols: Vec<CompletionSymbol>,
    existing_members: Vec<ast::Node>,
    class_element_modifier_flags: ast::ModifierFlags,
    file: &ast::SourceFile,
    position: i32,
    type_checker: &mut checker::Checker<'a, '_>,
) -> Vec<CompletionSymbol> {
    let store = file.store();
    let mut existing_member_names = collections::Set::new();
    for member in &existing_members {
        // Ignore omitted expressions for missing members.
        if store.kind(*member) != ast::Kind::PropertyDeclaration
            && store.kind(*member) != ast::Kind::MethodDeclaration
            && store.kind(*member) != ast::Kind::GetAccessor
            && store.kind(*member) != ast::Kind::SetAccessor
        {
            continue;
        }

        // If this is the current item we are editing right now, do not filter it out
        if is_currently_editing_node(member, file, position) {
            continue;
        }

        // Don't filter member even if the name matches if it is declared private in the list.
        if node_modifier_flags(store, member) & ast::ModifierFlags::Private
            != ast::ModifierFlags::None
        {
            continue;
        }

        // Do not filter it out if the static presence doesn't match.
        if ast::is_static(store, *member)
            != (class_element_modifier_flags & ast::ModifierFlags::Static
                != ast::ModifierFlags::None)
        {
            continue;
        }

        let existing_name = node_name(store, member)
            .as_ref()
            .map(|name| ast::get_property_name_for_property_name_node(store, name))
            .unwrap_or_default();
        if !existing_name.is_empty() {
            existing_member_names.add(existing_name);
        }
    }

    base_symbols
        .into_iter()
        .filter(|property_symbol| {
            let name = completion_symbol_name(type_checker, *property_symbol);
            let declarations = completion_symbol_declarations(type_checker, *property_symbol);
            let declaration_modifier_flags =
                declarations
                    .first()
                    .map_or(ast::MODIFIER_FLAGS_NONE, |declaration| {
                        type_checker.source_file_store(*declaration).map_or(
                            ast::MODIFIER_FLAGS_NONE,
                            |declaration_store| {
                                ast::get_combined_modifier_flags(declaration_store, *declaration)
                            },
                        )
                    });
            let value_declaration =
                completion_symbol_value_declaration(type_checker, *property_symbol);
            !existing_member_names.has(&name)
                && !declarations.is_empty()
                && declaration_modifier_flags & ast::ModifierFlags::Private
                    == ast::ModifierFlags::None
                && !value_declaration.is_some_and(|value_declaration| {
                    type_checker
                        .source_file_store(value_declaration)
                        .is_some_and(|declaration_store| {
                            ast::is_private_identifier_class_element_declaration(
                                declaration_store,
                                value_declaration,
                            )
                        })
                })
        })
        .collect()
}

impl LanguageService<'_> {
    pub fn get_optional_replacement_span(
        &self,
        location: Option<&ast::Node>,
        file: &ast::SourceFile,
    ) -> Option<lsproto::Range> {
        // StringLiteralLike locations are handled separately in stringCompletions.ts
        if location.is_some_and(|location| {
            file.store().kind(*location) == ast::Kind::Identifier
                || file.store().kind(*location) == ast::Kind::PrivateIdentifier
        }) {
            let location = location.unwrap();
            let start = astnav::get_start_of_node(*location, file);
            return Some(self.create_lsp_range_from_bounds(
                start,
                file.store().loc(*location).end(),
                file,
            ));
        }
        None
    }

    // Returns the item defaults for completion items, if that capability is supported.
    // Otherwise, if some item default is not supported by client, sets that property on each item.
    pub fn set_item_defaults(
        &self,
        ctx: &core::Context,
        position: i32,
        file: &ast::SourceFile,
        items: &mut [lsproto::CompletionItem],
        default_commit_characters: Option<&Vec<String>>,
        optional_replacement_span: Option<lsproto::Range>,
    ) -> Option<lsproto::CompletionItemDefaults> {
        let mut item_defaults = None;
        if let Some(default_commit_characters) = default_commit_characters {
            let supports_item_commit_characters = client_supports_item_commit_characters(ctx);
            if client_supports_default_commit_characters(ctx) && supports_item_commit_characters {
                item_defaults = Some(lsproto::CompletionItemDefaults {
                    commit_characters: Some(default_commit_characters.clone()),
                    ..Default::default()
                });
            } else if supports_item_commit_characters {
                for item in items.iter_mut() {
                    if item.commit_characters.is_none() {
                        item.commit_characters = Some(default_commit_characters.clone());
                    }
                }
            }
        }
        if let Some(optional_replacement_span) = optional_replacement_span {
            // Ported from vscode ts extension.
            let insert_range = lsproto::Range {
                start: optional_replacement_span.start,
                end: self.create_lsp_position(position, file),
            };
            if client_supports_default_edit_range(ctx) {
                let defaults =
                    item_defaults.get_or_insert_with(lsproto::CompletionItemDefaults::default);
                defaults.edit_range = Some(lsproto::RangeOrEditRangeWithInsertReplace {
                    edit_range_with_insert_replace: Some(lsproto::EditRangeWithInsertReplace {
                        insert: insert_range,
                        replace: optional_replacement_span,
                    }),
                    ..Default::default()
                });
                for item in items.iter_mut() {
                    // If `editRange` is set, `insertText` is ignored by the client, so we need to
                    // provide `textEdit` instead.
                    if item.insert_text.is_some() && item.text_edit.is_none() {
                        item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit {
                            insert_replace_edit: Some(lsproto::InsertReplaceEdit {
                                new_text: item.insert_text.clone().unwrap(),
                                insert: insert_range,
                                replace: optional_replacement_span,
                            }),
                            ..Default::default()
                        });
                        item.insert_text = None;
                    }
                }
            } else if client_supports_item_insert_replace(ctx) {
                for item in items.iter_mut() {
                    if item.text_edit.is_none() {
                        item.text_edit = Some(lsproto::TextEditOrInsertReplaceEdit {
                            insert_replace_edit: Some(lsproto::InsertReplaceEdit {
                                new_text: item
                                    .insert_text
                                    .clone()
                                    .unwrap_or_else(|| item.label.clone()),
                                insert: insert_range,
                                replace: optional_replacement_span,
                            }),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        item_defaults
    }

    pub fn specific_keyword_completion_info(
        &self,
        ctx: &core::Context,
        position: i32,
        file: &ast::SourceFile,
        mut items: Vec<lsproto::CompletionItem>,
        is_new_identifier_location: bool,
        optional_replacement_span: Option<lsproto::Range>,
    ) -> lsproto::CompletionList {
        let default_commit_characters = get_default_commit_characters(is_new_identifier_location);
        let item_defaults = self.set_item_defaults(
            ctx,
            position,
            file,
            &mut items,
            Some(&default_commit_characters),
            optional_replacement_span,
        );
        lsproto::CompletionList {
            is_incomplete: false,
            item_defaults,
            apply_kind: None,
            items,
        }
    }

    pub fn create_lsp_completion_item(
        &self,
        _ctx: &core::Context,
        mut name: String,
        mut insert_text: String,
        mut filter_text: String,
        sort_text: SortText,
        element_kind: lsutil::ScriptElementKind,
        kind_modifiers: lsutil::ScriptElementKindModifier,
        replacement_span: Option<lsproto::Range>,
        commit_characters: Option<Vec<String>>,
        label_details: Option<lsproto::CompletionItemLabelDetails>,
        file: &ast::SourceFile,
        position: i32,
        _is_member_completion: bool,
        is_snippet: bool,
        has_action: bool,
        preselect: bool,
        source: String,
        auto_import_fix: Option<lsproto::AutoImportFix>,
        detail: Option<String>,
    ) -> lsproto::CompletionItem {
        let kind = get_completions_symbol_kind(element_kind);
        let data = lsproto::CompletionItemData {
            file_name: file.file_name().to_string(),
            position,
            source: source.clone(),
            name: name.clone(),
            auto_import: auto_import_fix,
            ..Default::default()
        };

        // Text edit
        let mut text_edit = None;
        if let Some(replacement_span) = replacement_span {
            text_edit = Some(lsproto::TextEditOrInsertReplaceEdit {
                text_edit: Some(lsproto::TextEdit {
                    new_text: if insert_text.is_empty() {
                        name.clone()
                    } else {
                        insert_text.clone()
                    },
                    range: replacement_span,
                }),
                ..Default::default()
            });
        }

        // Filter text

        // Ported from vscode ts extension.
        let (word_size, word_start) = get_word_length_and_start(file, position as usize);
        let dot_accessor = get_dot_accessor(file, position as usize - word_size);
        if filter_text.is_empty() {
            filter_text = get_filter_text(
                file,
                position,
                &insert_text,
                &name,
                word_start,
                &dot_accessor,
            );
        }

        // Adjustements based on kind modifiers.
        let mut tags = None;
        // Copied from vscode ts extension: `MyCompletionItem.constructor`.
        if kind_modifiers & lsutil::ScriptElementKindModifier::OPTIONAL
            != lsutil::ScriptElementKindModifier::NONE
        {
            if insert_text.is_empty() {
                insert_text = name.clone();
            }
            if filter_text.is_empty() {
                filter_text = name.clone();
            }
            name.push('?');
        }
        if kind_modifiers & lsutil::ScriptElementKindModifier::DEPRECATED
            != lsutil::ScriptElementKindModifier::NONE
        {
            tags = Some(vec![lsproto::CompletionItemTag::DEPRECATED]);
        }

        if has_action && !source.is_empty() {
            // !!! adjust label like vscode does
        }

        // Client assumes plain text by default.
        let insert_text_format = if is_snippet {
            Some(lsproto::InsertTextFormat::Snippet)
        } else {
            None
        };

        lsproto::CompletionItem {
            label: name,
            label_details,
            kind: Some(kind),
            tags,
            detail,
            preselect: bool_to_ptr(preselect),
            sort_text: Some(sort_text),
            filter_text: str_ptr_to(filter_text),
            insert_text: str_ptr_to(insert_text),
            insert_text_format,
            text_edit,
            commit_characters,
            data: Some(data),
            ..Default::default()
        }
    }

    pub fn get_label_completions_at_position(
        &self,
        ctx: &core::Context,
        node: &ast::Node,
        file: &ast::SourceFile,
        position: i32,
        optional_replacement_span: Option<lsproto::Range>,
    ) -> Option<lsproto::CompletionList> {
        let mut items = self.get_label_statement_completions(ctx, node, file, position);
        if items.is_empty() {
            return None;
        }
        let default_commit_characters =
            get_default_commit_characters(false /*isNewIdentifierLocation*/);
        let item_defaults = self.set_item_defaults(
            ctx,
            position,
            file,
            &mut items,
            Some(&default_commit_characters),
            optional_replacement_span,
        );
        Some(lsproto::CompletionList {
            is_incomplete: false,
            item_defaults,
            apply_kind: None,
            items,
        })
    }

    pub fn get_label_statement_completions(
        &self,
        ctx: &core::Context,
        node: &ast::Node,
        file: &ast::SourceFile,
        position: i32,
    ) -> Vec<lsproto::CompletionItem> {
        let store = file.store();
        let mut uniques = collections::Set::new();
        let mut items = Vec::new();
        let mut current = Some(*node);
        while let Some(current_node) = current {
            if ast::is_function_like(store, Some(current_node)) {
                break;
            }
            if ast::is_labeled_statement(store, current_node) {
                let Some(label) = node_label(store, &current_node) else {
                    current = node_parent(store, &current_node);
                    continue;
                };
                let name = node_text(store, label).to_string();
                if !uniques.has(&name) {
                    uniques.add(name.clone());
                    items.push(self.create_lsp_completion_item(
                        ctx,
                        name,
                        String::new(), /*insertText*/
                        String::new(), /*filterText*/
                        SORT_TEXT_LOCATION_PRIORITY.to_string(),
                        lsutil::ScriptElementKind::Label,
                        lsutil::ScriptElementKindModifier::NONE, /*kindModifiers*/
                        None,
                        /*replacementSpan*/
                        None,
                        /*commitCharacters*/
                        None,
                        /*labelDetails*/
                        file,
                        position,
                        false,
                        /*isMemberCompletion*/
                        false,
                        /*isSnippet*/
                        false,
                        /*hasAction*/
                        false,
                        /*preselect*/
                        String::new(), /*source*/
                        None,
                        /*autoImportEntryData*/
                        None,
                        /*detail*/
                    ));
                }
            }
            current = node_parent(store, &current_node);
        }
        items
    }

    pub fn get_replacement_range_for_context_token(
        &self,
        file: &ast::SourceFile,
        context_token: Option<&ast::Node>,
        position: i32,
    ) -> Option<lsproto::Range> {
        let context_token = context_token?;

        // !!! ensure range is single line
        match file.store().kind(*context_token) {
            ast::Kind::StringLiteral | ast::Kind::NoSubstitutionTemplateLiteral => {
                self.create_range_from_string_literal_like_content(file, context_token, position)
            }
            _ => Some(self.create_lsp_range_from_node(*context_token, file)),
        }
    }

    pub fn create_range_from_string_literal_like_content(
        &self,
        file: &ast::SourceFile,
        node: &ast::StringLiteralLike,
        position: i32,
    ) -> Option<lsproto::Range> {
        let store = file.store();
        let mut replacement_end = store.loc(*node).end() - 1;
        let node_start = astnav::get_start_of_node(*node, file);
        if ast::is_unterminated_literal(store, *node) {
            // we return no replacement range only if unterminated string is empty
            if node_start == replacement_end {
                return None;
            }
            replacement_end = position.min(store.loc(*node).end());
        }
        Some(self.create_lsp_range_from_bounds(node_start + 1, replacement_end, file))
    }

    pub(crate) fn create_completion_details_for_symbol<'a, 'item>(
        &self,
        item: &'item mut lsproto::CompletionItem,
        symbol: CompletionSymbol,
        checker: &mut checker::Checker<'a, '_>,
        location: &ast::Node,
        position: i32,
        doc_format: lsproto::MarkupKind,
    ) -> &'item mut lsproto::CompletionItem {
        let mut vc = checker::VerbosityContext {
            level: 0,
            max_truncation_length: 0,
            can_increase_verbosity: false,
            truncated: false,
        };
        let (quick_info, documentation) = self.get_quick_info_and_documentation_for_symbol(
            checker,
            Some(symbol),
            *location,
            None,
            doc_format.clone(),
            &mut vc,
        );
        let _ = position;
        create_completion_details(item, &quick_info, &documentation, doc_format)
    }
}

pub fn get_closest_symbol_declaration(
    store: &ast::AstStore,
    context_token: Option<&ast::Node>,
    location: &ast::Node,
) -> Option<ast::Declaration> {
    let context_token = context_token?;

    let mut closest_declaration = None;
    let mut current = Some(*context_token);
    while let Some(node) = current {
        if ast::is_function_block(store, Some(node))
            || is_arrow_function_body(store, node)
            || ast::is_binding_pattern(store, node)
        {
            break;
        }

        if (ast::is_parameter_declaration(store, node)
            || ast::is_type_parameter_declaration(store, node))
            && !node_parent(store, &node)
                .as_ref()
                .is_some_and(|parent| ast::is_index_signature_declaration(store, *parent))
        {
            closest_declaration = Some(node);
            break;
        }
        current = node_parent(store, &node);
    }

    if closest_declaration.is_none() {
        let mut current = Some(*location);
        while let Some(node) = current {
            if ast::is_function_block(store, Some(node))
                || is_arrow_function_body(store, node)
                || ast::is_binding_pattern(store, node)
            {
                break;
            }

            if ast::is_variable_declaration(store, node) {
                closest_declaration = Some(node);
                break;
            }
            current = node_parent(store, &node);
        }
    }
    closest_declaration
}

impl LanguageService<'_> {
    pub fn get_jsx_closing_tag_completion(
        &self,
        ctx: &core::Context,
        location: &ast::Node,
        file: &ast::SourceFile,
        position: i32,
    ) -> Option<lsproto::CompletionList> {
        // We wanna walk up the tree till we find a JSX closing element.
        let jsx_closing_element =
            ast::find_ancestor_or_quit(file.store(), Some(*location), |store, node| {
                match store.kind(node) {
                    ast::Kind::JsxClosingElement => ast::FindAncestorResult::True,
                    ast::Kind::LessThanSlashToken
                    | ast::Kind::GreaterThanToken
                    | ast::Kind::Identifier
                    | ast::Kind::PropertyAccessExpression => ast::FindAncestorResult::False,
                    _ => ast::FindAncestorResult::Quit,
                }
            });

        let jsx_closing_element = jsx_closing_element?;

        // In the TypeScript JSX element, if such element is not defined. When users query for completion at closing tag,
        // instead of simply giving unknown value, the completion will return the tag-name of an associated opening-element.
        // For example:
        //     var x = <div> </ /*1*/
        // The completion list at "1" will contain "div>" with type any
        // And at `<div> </ /*1*/ >` (with a closing `>`), the completion list will contain "div".
        // And at property access expressions `<MainComponent.Child> </MainComponent. /*1*/ >` the completion will
        // return full closing tag with an optional replacement span
        // For example:
        //     var x = <MainComponent.Child> </     MainComponent /*1*/  >
        //     var y = <MainComponent.Child> </   /*2*/   MainComponent >
        // the completion list at "1" and "2" will contain "MainComponent.Child" with a replacement span of closing tag name
        let has_closing_angle_bracket =
            astnav::has_child_of_kind(jsx_closing_element, ast::Kind::GreaterThanToken, file);
        let store = file.store();
        let jsx_element = node_parent(store, jsx_closing_element)?;
        let opening_element = store.opening_element(jsx_element)?;
        let tag_name = node_tag_name(store, opening_element);
        let tag_name = tag_name?;
        let closing_tag = scanner::get_text_of_node(file, &tag_name);
        let full_closing_tag = closing_tag + if has_closing_angle_bracket { "" } else { ">" };
        let optional_replacement_span = node_tag_name(store, jsx_closing_element)
            .as_ref()
            .map(|tag_name| self.create_lsp_range_from_node(*tag_name, file));
        let default_commit_characters =
            get_default_commit_characters(false /*isNewIdentifierLocation*/);

        let item = self.create_lsp_completion_item(
            ctx,
            full_closing_tag.clone(),
            String::new(), /*insertText*/
            String::new(), /*filterText*/
            SORT_TEXT_LOCATION_PRIORITY.to_string(),
            lsutil::ScriptElementKind::ClassElement,
            lsutil::ScriptElementKindModifier::NONE, /*kindModifiers*/
            None,
            /*replacementSpan*/
            None,
            /*commitCharacters*/
            None,
            /*labelDetails*/
            file,
            position,
            true,  /*isMemberCompletion*/
            false, /*isSnippet*/
            false, /*hasAction*/
            false, /*preselect*/
            String::new(),
            /*source*/
            None, /*autoImportEntryData*/
            // !!! jsx autoimports
            None, /*detail*/
        );
        let mut items = vec![item];
        let item_defaults = self.set_item_defaults(
            ctx,
            position,
            file,
            &mut items,
            Some(&default_commit_characters),
            optional_replacement_span,
        );

        Some(lsproto::CompletionList {
            is_incomplete: false,
            item_defaults,
            apply_kind: None,
            items,
        })
    }
}

pub fn type_node_to_expression(
    type_node: &ast::TypeNode,
    target: core::ScriptTarget,
    quote_preference: lsutil::QuotePreference,
    factory: &mut ast::NodeFactory,
) -> Option<ast::Expression> {
    match factory.store().kind(*type_node) {
        ast::Kind::TypeReference => {
            let type_name = factory.store().type_name(*type_node)?;
            entity_name_to_expression(&type_name, target, quote_preference, factory)
        }
        ast::Kind::IndexedAccessType => {
            let (object_type, index_type) = {
                let store = factory.store();
                (
                    store.object_type(*type_node)?,
                    store.index_type(*type_node)?,
                )
            };
            let object_expression =
                type_node_to_expression(&object_type, target, quote_preference, factory);
            let index_expression =
                type_node_to_expression(&index_type, target, quote_preference, factory);
            if object_expression.is_some() && index_expression.is_some() {
                return Some(factory.new_element_access_expression(
                    object_expression.unwrap(),
                    None, /*questionDotToken*/
                    index_expression.unwrap(),
                    ast::NODE_FLAGS_NONE,
                ));
            }
            None
        }
        ast::Kind::LiteralType => {
            let literal = factory.store().literal(*type_node)?;
            match factory.store().kind(literal) {
                ast::Kind::StringLiteral => {
                    let text = factory.store().text(literal);
                    Some(factory.new_string_literal(
                        text,
                        if quote_preference == lsutil::QuotePreference::Single {
                            ast::TOKEN_FLAGS_SINGLE_QUOTE
                        } else {
                            ast::TOKEN_FLAGS_NONE
                        },
                    ))
                }
                ast::Kind::NumericLiteral => {
                    let (text, token_flags) = {
                        let store = factory.store();
                        (
                            store.text(literal),
                            store.token_flags(literal).unwrap_or(ast::TOKEN_FLAGS_NONE),
                        )
                    };
                    Some(factory.new_numeric_literal(text, token_flags))
                }
                _ => None,
            }
        }
        ast::Kind::ParenthesizedType => {
            let parenthesized_type = factory.store().r#type(*type_node)?;
            let expr =
                type_node_to_expression(&parenthesized_type, target, quote_preference, factory)?;
            if ast::is_identifier(factory.store(), expr) {
                return Some(expr);
            }
            Some(factory.new_parenthesized_expression(expr))
        }
        ast::Kind::TypeQuery => {
            let expr_name = factory.store().expr_name(*type_node)?;
            entity_name_to_expression(&expr_name, target, quote_preference, factory)
        }
        ast::Kind::ImportType => debug::fail(
            "We should not get an import type after calling 'typeToAutoImportableTypeNode'.",
        ),
        _ => None,
    }
}

pub fn entity_name_to_expression(
    entity_name: &ast::EntityName,
    target: core::ScriptTarget,
    quote_preference: lsutil::QuotePreference,
    factory: &mut ast::NodeFactory,
) -> Option<ast::Expression> {
    let _ = (target, quote_preference);
    if ast::is_identifier(factory.store(), *entity_name) {
        return Some(*entity_name);
    }
    let (left, right) = {
        let store = factory.store();
        (store.left(*entity_name)?, store.right(*entity_name)?)
    };
    let left_expression = entity_name_to_expression(&left, target, quote_preference, factory)?;
    Some(factory.new_property_access_expression(
        left_expression,
        None, /*questionDotToken*/
        right,
        ast::NODE_FLAGS_NONE,
    ))
}

pub struct SnippetPrinter {
    pub base_writer: printer::ChangeTrackerWriter,
    pub printer: printer::Printer,
    pub writer: SnippetEmitTextWriter,
    pub factory: ast::NodeFactory,
}

impl SnippetPrinter {
    /** Snippet-escaping version of `printer.printNode`. */
    pub(crate) fn print_node(&mut self, node: ast::Node) -> String {
        let unescaped = self.print_unescaped_node(node);
        escape_snippet_text(&unescaped)
    }

    pub(crate) fn print_unescaped_node(&mut self, node: ast::Node) -> String {
        self.writer.escapes.clear();
        self.writer.clear();
        self.printer.emit(&node, None /*sourceFile*/)
    }

    pub fn print_and_format_node(
        &mut self,
        ctx: core::Context,
        node: ast::Node,
        source_file: &ast::SourceFile,
    ) -> String {
        let _ = (ctx, source_file);
        self.print_node(node)
    }

    // Creates a source file containing `node` for formatting purposes.
    // `node` and descendants need to be synthetic nodes with positions assigned.
    // This function also assigns parent pointers.
    pub fn create_synthetic_file(
        &mut self,
        node: ast::Node,
        text: &str,
        target_file: &ast::SourceFile,
    ) -> ast::SourceFile {
        let eof = self.factory.new_token(ast::Kind::EndOfFile);
        self.factory.place_change_tracker_node(
            eof,
            core::new_text_range(text.len() as i32, text.len() as i32),
        );
        let node_loc = self.factory.loc(node);
        let statements = self.factory.new_node_list(node_loc, node_loc, vec![node]);
        let synthetic_file = self.factory.new_source_file(
            target_file.parse_options(),
            text.to_string(),
            statements,
            eof,
        );
        self.factory
            .place_change_tracker_node(synthetic_file, core::new_text_range(0, text.len() as i32));
        self.factory.adopt_change_tracker_children(synthetic_file);
        let factory = std::mem::take(&mut self.factory);
        factory.finish_parsed_source_file(synthetic_file, ast::ParsedSourceFileMetadata::default())
    }
}

pub fn create_snippet_printer(options: printer::PrinterOptions) -> SnippetPrinter {
    let mut base_writer = printer::new_change_tracker_writer(
        options.new_line.get_new_line_character().to_string(),
        -1,
    );
    let printer = printer::new_printer(
        options,
        base_writer.get_print_handlers(),
        None, /*emitContext*/
    );
    let writer = SnippetEmitTextWriter {
        change_tracker_writer: printer::new_change_tracker_writer(
            options.new_line.get_new_line_character().to_string(),
            -1,
        ),
        escapes: Vec::new(),
    };
    SnippetPrinter {
        base_writer,
        printer,
        writer,
        factory: ast::new_node_factory(ast::NodeFactoryHooks::default()),
    }
}

// Override base writer methods to perform snippet escaping.
pub struct SnippetEmitTextWriter {
    pub change_tracker_writer: printer::ChangeTrackerWriter,
    pub escapes: Vec<core::TextChange>,
}

impl SnippetEmitTextWriter {
    pub fn non_escaping_write(&mut self, s: &str) {
        self.change_tracker_writer.write(s);
    }

    pub fn write(&mut self, s: &str) {
        self.escaping_write(s, |writer| writer.change_tracker_writer.write(s));
    }

    pub fn write_comment(&mut self, text: &str) {
        self.escaping_write(text, |writer| {
            writer.change_tracker_writer.write_comment(text)
        });
    }

    pub fn write_string_literal(&mut self, text: &str) {
        self.escaping_write(text, |writer| {
            writer.change_tracker_writer.write_string_literal(text)
        });
    }

    pub fn write_parameter(&mut self, text: &str) {
        self.escaping_write(text, |writer| {
            writer.change_tracker_writer.write_parameter(text)
        });
    }

    pub fn write_property(&mut self, text: &str) {
        self.escaping_write(text, |writer| {
            writer.change_tracker_writer.write_property(text)
        });
    }

    // The formatter/scanner will have issues with snippet-escaped text,
    // so instead of writing the escaped text directly to the writer,
    // generate a set of changes that can be applied to the unescaped text
    // to escape it post-formatting.
    pub fn escaping_write<F>(&mut self, s: &str, write: F)
    where
        F: FnOnce(&mut Self),
    {
        let escaped = escape_snippet_text(s);
        if escaped != s {
            let start = self.get_text_pos();
            write(self);
            let end = self.get_text_pos();
            self.escapes.push(core::TextChange {
                new_text: escaped,
                text_range: core::new_text_range(start, end),
            });
        } else {
            write(self);
        }
    }

    pub fn clear(&mut self) {
        self.change_tracker_writer.clear();
    }

    pub fn string(&self) -> String {
        self.change_tracker_writer.string()
    }

    pub fn get_text_pos(&self) -> i32 {
        self.change_tracker_writer.get_text_pos()
    }
}
