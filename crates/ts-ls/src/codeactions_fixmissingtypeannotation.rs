use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_nodebuilder as nodebuilder;
use ts_printer as printer;
use ts_scanner as scanner;

use crate::autoimport;
use crate::change;
use crate::codeactions::{
    CodeAction, CodeFixContext, CodeFixProvider, CombinedCodeActions, contains_error_code,
};
use crate::diagnostics::get_all_diagnostics_with_checker;

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

fn synthetic_modifier_list(
    factory: &mut ast::NodeFactory,
    modifiers: impl IntoIterator<Item = ast::Node>,
    flags: ast::ModifierFlags,
) -> ast::ModifierList {
    factory.new_modifier_list(
        core::new_text_range(-1, -1),
        core::new_text_range(-1, -1),
        modifiers,
        flags,
    )
}

pub fn isolated_declarations_fix_error_codes() -> Vec<i32> {
    vec![
        diagnostics::FUNCTION_MUST_HAVE_AN_EXPLICIT_RETURN_TYPE_ANNOTATION_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::METHOD_MUST_HAVE_AN_EXPLICIT_RETURN_TYPE_ANNOTATION_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::AT_LEAST_ONE_ACCESSOR_MUST_HAVE_AN_EXPLICIT_TYPE_ANNOTATION_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::VARIABLE_MUST_HAVE_AN_EXPLICIT_TYPE_ANNOTATION_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::PARAMETER_MUST_HAVE_AN_EXPLICIT_TYPE_ANNOTATION_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::PROPERTY_MUST_HAVE_AN_EXPLICIT_TYPE_ANNOTATION_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::EXPRESSION_TYPE_CAN_T_BE_INFERRED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::BINDING_ELEMENTS_CAN_T_BE_EXPORTED_DIRECTLY_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::COMPUTED_PROPERTY_NAMES_ON_CLASS_OR_OBJECT_LITERALS_CANNOT_BE_INFERRED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::COMPUTED_PROPERTIES_MUST_BE_NUMBER_OR_STRING_LITERALS_VARIABLES_OR_DOTTED_EXPRESSIONS_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::ENUM_MEMBER_INITIALIZERS_MUST_BE_COMPUTABLE_WITHOUT_REFERENCES_TO_EXTERNAL_SYMBOLS_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::EXTENDS_CLAUSE_CAN_T_CONTAIN_AN_EXPRESSION_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::OBJECTS_THAT_CONTAIN_SHORTHAND_PROPERTIES_CAN_T_BE_INFERRED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::OBJECTS_THAT_CONTAIN_SPREAD_ASSIGNMENTS_CAN_T_BE_INFERRED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::ARRAYS_WITH_SPREAD_ELEMENTS_CAN_T_INFERRED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::DEFAULT_EXPORTS_CAN_T_BE_INFERRED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::ONLY_CONST_ARRAYS_CAN_BE_INFERRED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::ASSIGNING_PROPERTIES_TO_FUNCTIONS_WITHOUT_DECLARING_THEM_IS_NOT_SUPPORTED_WITH_ISOLATED_DECLARATIONS_ADD_AN_EXPLICIT_DECLARATION_FOR_THE_PROPERTIES_ASSIGNED_TO_THIS_FUNCTION.code(),
        diagnostics::DECLARATION_EMIT_FOR_THIS_PARAMETER_REQUIRES_IMPLICITLY_ADDING_UNDEFINED_TO_ITS_TYPE_THIS_IS_NOT_SUPPORTED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::TYPE_CONTAINING_PRIVATE_NAME_0_CAN_T_BE_USED_WITH_ISOLATED_DECLARATIONS.code(),
        diagnostics::ADD_SATISFIES_AND_A_TYPE_ASSERTION_TO_THIS_EXPRESSION_SATISFIES_T_AS_T_TO_MAKE_THE_TYPE_EXPLICIT.code(),
    ]
}

pub const FIX_MISSING_TYPE_ANNOTATION_ON_EXPORTS_FIX_ID: &str = "fixMissingTypeAnnotationOnExports";

pub static ISOLATED_DECLARATIONS_FIX_PROVIDER: CodeFixProvider = CodeFixProvider {
    error_codes: isolated_declarations_fix_error_codes,
    get_code_actions: get_isolated_declarations_code_actions,
    fix_ids: &[FIX_MISSING_TYPE_ANNOTATION_ON_EXPORTS_FIX_ID],
    get_all_code_actions: Some(get_all_isolated_declarations_code_actions),
};

pub fn can_have_type_annotation_kinds() -> Vec<ast::Kind> {
    vec![
        ast::Kind::GetAccessor,
        ast::Kind::MethodDeclaration,
        ast::Kind::PropertyDeclaration,
        ast::Kind::FunctionDeclaration,
        ast::Kind::FunctionExpression,
        ast::Kind::ArrowFunction,
        ast::Kind::VariableDeclaration,
        ast::Kind::Parameter,
        ast::Kind::ExportAssignment,
        ast::Kind::ClassDeclaration,
        ast::Kind::ObjectBindingPattern,
        ast::Kind::ArrayBindingPattern,
    ]
}

pub const DECLARATION_EMIT_NODE_BUILDER_FLAGS: nodebuilder::Flags =
    nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS
        | nodebuilder::FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL
        | nodebuilder::FLAGS_USE_TYPE_OF_FUNCTION
        | nodebuilder::FLAGS_USE_STRUCTURAL_FALLBACK
        | nodebuilder::FLAGS_ALLOW_EMPTY_TUPLE
        | nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS
        | nodebuilder::FLAGS_NO_TRUNCATION;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TypePrintMode {
    #[default]
    Full = 0,
    Relative = 1,
    Widened = 2,
}

pub const TYPE_PRINT_MODE_FULL: TypePrintMode = TypePrintMode::Full;
pub const TYPE_PRINT_MODE_RELATIVE: TypePrintMode = TypePrintMode::Relative;
pub const TYPE_PRINT_MODE_WIDENED: TypePrintMode = TypePrintMode::Widened;

pub fn get_isolated_declarations_code_actions(
    context: &core::Context,
    fix_context: &CodeFixContext,
) -> Result<Vec<CodeAction>, core::Error> {
    fix_context.program.with_type_checker_for_file_using(
        compiler::CheckerAccess::context(context),
        fix_context.source_file,
        |checker| {
            let mut fixes = Vec::new();
            let mut seen: Vec<CodeAction> = Vec::new();

            let mut add_fix = |action: Option<CodeAction>| {
                let Some(action) = action else {
                    return;
                };
                let i = seen
                    .binary_search_by(|existing| existing.compare(&action).cmp(&0))
                    .unwrap_or_else(|i| i);
                if seen
                    .get(i)
                    .is_some_and(|existing| existing.compare(&action) == 0)
                {
                    return;
                }
                seen.insert(
                    i,
                    CodeAction {
                        description: action.description.clone(),
                        changes: action.changes.clone(),
                        fix_id: action.fix_id.clone(),
                        fix_all_description: action.fix_all_description.clone(),
                    },
                );
                fixes.push(action);
            };

            let modes = [
                TypePrintMode::Full,
                TypePrintMode::Relative,
                TypePrintMode::Widened,
            ];

            for mode in modes {
                add_fix(try_code_action(context, fix_context, checker, |f| {
                    f.type_print_mode = mode;
                    f.add_type_annotation(fix_context.span)
                }));
            }

            for mode in modes {
                add_fix(try_code_action(context, fix_context, checker, |f| {
                    f.type_print_mode = mode;
                    f.add_inline_assertion(fix_context.span)
                }));
            }

            add_fix(try_code_action(context, fix_context, checker, |f| {
                f.type_print_mode = TypePrintMode::Full;
                f.extract_as_variable(fix_context.span)
            }));
            Ok(fixes)
        },
    )
}

pub fn get_all_isolated_declarations_code_actions(
    context: &core::Context,
    fix_context: &CodeFixContext,
) -> Result<Option<CombinedCodeActions>, core::Error> {
    fix_context.program.with_type_checker_for_file_using(
        compiler::CheckerAccess::context(context),
        fix_context.source_file,
        |checker| {
            let all_diags = get_all_diagnostics_with_checker(
                context,
                fix_context.program,
                fix_context.source_file,
                checker,
            );
            let change_tracker = change::new_tracker(
                context.clone(),
                fix_context.program.options(),
                fix_context.ls.format_options(),
                &fix_context.ls.converters,
            );

            let mut fixer = IsolatedDeclarationsFixer {
                source_file: fix_context.source_file,
                program: fix_context.program,
                checker,
                change_tracker: Some(change_tracker),
                import_adder: None,
                locale: locale::und(),
                fixed_nodes: std::collections::HashSet::new(),
                type_print_mode: TypePrintMode::Full,
                symbols_to_import: Vec::new(),
                mutated_target: false,
            };

            for diag in all_diags {
                if contains_error_code(&isolated_declarations_fix_error_codes(), diag.code()) {
                    let span = core::new_text_range(diag.pos(), diag.end());
                    fixer.add_type_annotation(span);
                }
            }

            let symbols_to_import = fixer.symbols_to_import.clone();
            for sym in symbols_to_import {
                fixer.add_symbol_to_existing_import(sym);
            }

            let Some(change_tracker) = fixer.change_tracker.as_mut() else {
                return Ok(None);
            };
            let changes = change_tracker.get_changes();
            let file_changes = changes
                .get(&fix_context.source_file.file_name())
                .cloned()
                .unwrap_or_default();
            if file_changes.is_empty() {
                return Ok(None);
            }

            Ok(Some(CombinedCodeActions {
                description: diagnostics::ADD_ALL_MISSING_TYPE_ANNOTATIONS
                    .localize(locale::und(), vec![]),
                changes: file_changes,
            }))
        },
    )
}

pub fn try_code_action<'a>(
    context: &core::Context,
    fix_context: &'a CodeFixContext<'a>,
    checker: &mut checker::Checker<'a, '_>,
    f: impl for<'checker, 'state> Fn(&mut IsolatedDeclarationsFixer<'a, 'checker, 'state>) -> String,
) -> Option<CodeAction> {
    let change_tracker = change::new_tracker(
        context.clone(),
        fix_context.program.options(),
        fix_context.ls.format_options(),
        &fix_context.ls.converters,
    );

    let import_adder = None;

    let mut fixer = IsolatedDeclarationsFixer {
        source_file: fix_context.source_file,
        program: fix_context.program,
        checker,
        change_tracker: Some(change_tracker),
        import_adder,
        locale: locale::und(),
        fixed_nodes: std::collections::HashSet::new(),
        type_print_mode: TypePrintMode::Full,
        symbols_to_import: Vec::new(),
        mutated_target: false,
    };

    let description = f(&mut fixer);
    if description.is_empty() {
        return None;
    }

    let symbols_to_import = fixer.symbols_to_import.clone();
    for sym in symbols_to_import {
        fixer.add_symbol_to_existing_import(sym);
    }

    let change_tracker = fixer.change_tracker.as_mut()?;
    let mut file_changes = change_tracker
        .get_changes()
        .remove(&fix_context.source_file.file_name())
        .unwrap_or_default();

    if fixer
        .import_adder
        .as_ref()
        .is_some_and(|import_adder| import_adder.has_fixes())
    {
        file_changes.extend(fixer.import_adder.as_ref().unwrap().edits());
    }

    if file_changes.is_empty() {
        return None;
    }

    Some(CodeAction {
        description,
        changes: file_changes,
        fix_id: FIX_MISSING_TYPE_ANNOTATION_ON_EXPORTS_FIX_ID.to_string(),
        fix_all_description: diagnostics::ADD_ALL_MISSING_TYPE_ANNOTATIONS
            .localize(locale::und(), vec![]),
    })
}

pub struct IsolatedDeclarationsFixer<'a, 'checker, 'state> {
    pub source_file: &'a ast::SourceFile,
    pub program: &'a compiler::Program,
    pub checker: &'checker mut checker::Checker<'a, 'state>,
    pub change_tracker: Option<change::Tracker<'a>>,
    pub import_adder: Option<autoimport::ImportAdder<'a>>,
    pub locale: locale::Locale,
    pub fixed_nodes: std::collections::HashSet<u64>,
    pub type_print_mode: TypePrintMode,
    pub(crate) symbols_to_import: Vec<ast::SymbolIdentity>,
    pub mutated_target: bool,
}

impl<'a, 'checker, 'state> IsolatedDeclarationsFixer<'a, 'checker, 'state> {
    fn checker_mut(&mut self) -> &mut checker::Checker<'a, 'state> {
        self.checker
    }

    fn source_store(&self) -> &'a ast::AstStore {
        self.source_file.store()
    }

    pub fn add_type_annotation(&mut self, span: core::TextRange) -> String {
        let node_with_diag = astnav::get_token_at_position(self.source_file, span.pos());

        let source_file = self.source_file;
        let expando_function =
            find_expando_function(self.checker_mut(), source_file.store(), node_with_diag);
        if let Some(expando_function) = expando_function {
            if ast::is_function_declaration(self.source_file.store(), expando_function) {
                return self.create_namespace_for_expando_properties(expando_function);
            }
            return self.fix_isolated_declaration_error(expando_function);
        }

        if let Some(node_missing_type) =
            find_ancestor_with_missing_type(self.source_file.store(), node_with_diag)
        {
            return self.fix_isolated_declaration_error(node_missing_type);
        }
        String::new()
    }

    pub(crate) fn create_namespace_for_expando_properties(
        &mut self,
        expando_func: ast::Node,
    ) -> String {
        let Some(func_name) = self.source_store().name(expando_func) else {
            return String::new();
        };

        let t = self.checker_mut().get_type_at_location(expando_func);
        let elements = self.checker_mut().get_properties_of_type_public(t);
        if elements.is_empty() {
            return String::new();
        }

        let mut new_properties = Vec::new();
        for symbol in elements {
            let Some(symbol_name) = self.checker_mut().symbol_name_public(symbol) else {
                continue;
            };
            if !scanner::is_identifier_text(&symbol_name, core::LanguageVariant::Standard) {
                continue;
            }
            if self
                .checker_mut()
                .symbol_value_declaration_public(symbol)
                .is_some_and(|decl| ast::is_variable_declaration(self.source_file.store(), decl))
            {
                continue;
            }

            let Some(sym_type) = self
                .checker_mut()
                .get_type_of_symbol_identity_public(symbol)
            else {
                continue;
            };
            let Some(type_node) = self.type_to_minimized_reference_type(
                sym_type,
                expando_func,
                DECLARATION_EMIT_NODE_BUILDER_FLAGS,
            ) else {
                continue;
            };

            let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
            let var_name = factory.new_identifier(symbol_name);
            let var_decl = factory.new_variable_declaration(var_name, None, Some(type_node), None);
            let export_token = factory.new_token(ast::Kind::ExportKeyword);
            let declarations = synthetic_node_list(factory, vec![var_decl]);
            let var_decl_list =
                factory.new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
            let modifiers =
                synthetic_modifier_list(factory, vec![export_token], ast::MODIFIER_FLAGS_EXPORT);
            let var_stmt = factory.new_variable_statement(Some(modifiers), var_decl_list);
            new_properties.push(var_stmt);
        }

        if new_properties.is_empty() {
            return String::new();
        }

        let mut modifiers = Vec::new();
        if ast::has_syntactic_modifier(
            self.source_file.store(),
            expando_func,
            ast::ModifierFlags::Export,
        ) {
            modifiers.push(
                self.change_tracker
                    .as_mut()
                    .unwrap()
                    .node_factory
                    .new_token(ast::Kind::ExportKeyword),
            );
        }
        modifiers.push(
            self.change_tracker
                .as_mut()
                .unwrap()
                .node_factory
                .new_token(ast::Kind::DeclareKeyword),
        );

        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        let namespace_modifiers = synthetic_modifier_list(
            factory,
            modifiers,
            ast::MODIFIER_FLAGS_EXPORT | ast::MODIFIER_FLAGS_AMBIENT,
        );
        let namespace_name = factory.new_identifier(self.source_file.store().text(func_name));
        let namespace_body_nodes = synthetic_node_list(factory, new_properties);
        let namespace_body = factory.new_module_block(namespace_body_nodes);
        let namespace = factory.new_module_declaration(
            Some(namespace_modifiers),
            ast::Kind::NamespaceKeyword,
            namespace_name,
            Some(namespace_body),
        );
        factory.mark_change_tracker_ambient_export_context(namespace);

        self.change_tracker.as_mut().unwrap().insert_node_after(
            self.source_file,
            expando_func,
            namespace,
        );
        diagnostics::ANNOTATE_TYPES_OF_PROPERTIES_EXPANDO_FUNCTION_IN_A_NAMESPACE
            .localize(self.locale.clone(), vec![])
    }
}

pub fn needs_parenthesized_expression_for_assertion(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    !ast::is_entity_name_expression(store, node)
        && !ast::is_call_expression(store, node)
        && !ast::is_object_literal_expression(store, node)
        && !ast::is_array_literal_expression(store, node)
}

pub fn create_as_expression(
    factory: &mut ast::NodeFactory,
    node: ast::Node,
    type_node: ast::Node,
) -> ast::Node {
    let node = if needs_parenthesized_expression_for_assertion(factory.store(), node) {
        factory.new_parenthesized_expression(node)
    } else {
        node
    };
    factory.new_as_expression(node, type_node)
}

pub fn deep_clone_for_change_factory(
    factory: &mut ast::NodeFactory,
    source_file: &ast::SourceFile,
    node: ast::Node,
) -> ast::Node {
    if node.store_id() == factory.store().store_id() {
        node
    } else {
        factory.deep_clone_node_from_store(source_file.store(), node)
    }
}

impl<'a, 'checker, 'state> IsolatedDeclarationsFixer<'a, 'checker, 'state> {
    pub fn add_inline_assertion(&mut self, span: core::TextRange) -> String {
        let node_with_diag = astnav::get_token_at_position(self.source_file, span.pos());

        let source_store = self.source_file.store();
        if find_expando_function(self.checker_mut(), source_store, node_with_diag).is_some() {
            return String::new();
        }

        let Some(target_node) =
            find_best_fitting_node(self.source_file.store(), node_with_diag, span)
        else {
            return String::new();
        };
        if is_value_signature_declaration(self.source_file.store(), Some(target_node))
            || self
                .source_file
                .store()
                .parent(target_node)
                .is_some_and(|parent| {
                    is_value_signature_declaration(self.source_file.store(), Some(parent))
                })
        {
            return String::new();
        }

        let is_expression_target = ast::is_expression(self.source_file.store(), target_node);
        let is_shorthand_property_assignment_target =
            ast::is_shorthand_property_assignment(self.source_file.store(), target_node);
        if !is_shorthand_property_assignment_target
            && is_named_declaration_kind(self.source_file.store(), target_node)
        {
            return String::new();
        }
        if ast::find_ancestor(
            self.source_file.store(),
            Some(target_node),
            |store, node| ast::is_binding_pattern(store, node),
        )
        .is_some()
        {
            return String::new();
        }
        if ast::find_ancestor(
            self.source_file.store(),
            Some(target_node),
            |store, node| ast::is_enum_member(store, node),
        )
        .is_some()
        {
            return String::new();
        }
        if is_expression_target
            && (ast::find_ancestor_kind(
                self.source_file.store(),
                Some(target_node),
                ast::Kind::HeritageClause,
            )
            .is_some()
                || ast::find_ancestor(
                    self.source_file.store(),
                    Some(target_node),
                    |store, node| ast::is_type_node(store, node),
                )
                .is_some())
        {
            return String::new();
        }
        if ast::is_spread_element(self.source_file.store(), target_node) {
            return String::new();
        }

        let variable_declaration = ast::find_ancestor_kind(
            self.source_file.store(),
            Some(target_node),
            ast::Kind::VariableDeclaration,
        );
        let variable_type = variable_declaration.as_ref().map(|variable_declaration| {
            self.checker_mut()
                .get_type_at_location(*variable_declaration)
        });
        if let Some(variable_type) = variable_type {
            if self.checker_mut().type_flags_public(variable_type)
                & checker::TYPE_FLAGS_UNIQUE_ES_SYMBOL
                != 0
            {
                return String::new();
            }
        }

        if !is_expression_target && !is_shorthand_property_assignment_target {
            return String::new();
        }

        let Some(type_node) = self.infer_type(target_node, variable_type) else {
            return String::new();
        };
        if self.mutated_target {
            return String::new();
        }

        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        if is_shorthand_property_assignment_target {
            let Some(shorthand_name) = self.source_file.store().name(target_node) else {
                return String::new();
            };
            let cloned_name =
                deep_clone_for_change_factory(factory, self.source_file, shorthand_name);
            let as_expr = create_as_expression(factory, cloned_name, type_node);
            self.change_tracker.as_mut().unwrap().insert_node_at(
                self.source_file,
                core::TextPos(self.source_file.store().loc(target_node).end()),
                as_expr,
                change::NodeOptions {
                    prefix: ": ".to_string(),
                    ..Default::default()
                },
            );
        } else if is_expression_target {
            let mut cloned_target =
                deep_clone_for_change_factory(factory, self.source_file, target_node);
            if needs_parenthesized_expression_for_assertion(self.source_file.store(), target_node) {
                cloned_target = factory.new_parenthesized_expression(cloned_target);
            }
            let cloned_type = deep_clone_for_change_factory(factory, self.source_file, type_node);
            let satisfies_expr = factory.new_satisfies_expression(cloned_target, cloned_type);
            let satisfies_as_expr = factory.new_as_expression(satisfies_expr, type_node.clone());
            self.change_tracker.as_mut().unwrap().replace_node(
                self.source_file,
                target_node,
                satisfies_as_expr,
                None,
            );
        } else {
            return String::new();
        }

        diagnostics::ADD_SATISFIES_AND_AN_INLINE_TYPE_ASSERTION_WITH_0.localize(
            self.locale.clone(),
            vec![Box::new(type_to_string_for_diag(
                type_node,
                self.source_file,
                self.change_tracker.as_mut().unwrap(),
            ))],
        )
    }

    pub fn extract_as_variable(&mut self, span: core::TextRange) -> String {
        let node_with_diag = astnav::get_token_at_position(self.source_file, span.pos());
        let Some(target_node) =
            find_best_fitting_node(self.source_file.store(), node_with_diag, span)
        else {
            return String::new();
        };
        if is_value_signature_declaration(self.source_file.store(), Some(target_node))
            || self
                .source_file
                .store()
                .parent(target_node)
                .is_some_and(|parent| {
                    is_value_signature_declaration(self.source_file.store(), Some(parent))
                })
        {
            return String::new();
        }
        if !ast::is_expression(self.source_file.store(), target_node) {
            return String::new();
        }

        if ast::is_array_literal_expression(self.source_file.store(), target_node) {
            let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
            let const_id = factory.new_identifier("const".to_string());
            let const_ref = factory.new_type_reference_node(const_id, None);
            let cloned = deep_clone_for_change_factory(factory, self.source_file, target_node);
            let as_expr = create_as_expression(factory, cloned, const_ref);
            self.change_tracker.as_mut().unwrap().replace_node(
                self.source_file,
                target_node,
                as_expr,
                None,
            );
            return diagnostics::MARK_ARRAY_LITERAL_AS_CONST.localize(self.locale.clone(), vec![]);
        }

        let Some(parent_property_assignment) = ast::find_ancestor_kind(
            self.source_file.store(),
            Some(target_node),
            ast::Kind::PropertyAssignment,
        ) else {
            return String::new();
        };
        if self
            .source_file
            .store()
            .parent(target_node)
            .is_some_and(|parent| parent == parent_property_assignment)
            && ast::is_entity_name_expression(self.source_file.store(), target_node)
        {
            return String::new();
        }

        let temp_name = self
            .change_tracker
            .as_mut()
            .unwrap()
            .emit_context
            .factory
            .new_unique_name_ex(
                &get_identifier_name_for_node(self.source_file.store(), target_node),
                printer::AutoGenerateOptions {
                    flags: printer::GENERATED_IDENTIFIER_FLAGS_OPTIMISTIC,
                    ..Default::default()
                },
            );

        let mut replacement_target = target_node;
        let mut initialization_node = target_node;
        if ast::is_spread_element(self.source_file.store(), replacement_target) {
            let parent = self.source_file.store().parent(replacement_target);
            let Some(walked) =
                ast::walk_up_parenthesized_expressions(self.source_file.store(), parent)
            else {
                return String::new();
            };
            replacement_target = walked;
            if self
                .source_file
                .store()
                .parent(replacement_target)
                .is_some_and(|parent| is_const_assertion(self.source_file.store(), parent))
            {
                replacement_target = self.source_file.store().parent(replacement_target).unwrap();
                initialization_node = replacement_target;
            } else {
                let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
                let const_id = factory.new_identifier("const".to_string());
                let const_ref = factory.new_type_reference_node(const_id, None);
                let cloned_replacement_target =
                    deep_clone_for_change_factory(factory, self.source_file, replacement_target);
                let init = create_as_expression(factory, cloned_replacement_target, const_ref);
                initialization_node = init;
            }
        }

        if ast::is_entity_name_expression(self.source_file.store(), replacement_target) {
            return String::new();
        }

        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        let cloned_init =
            deep_clone_for_change_factory(factory, self.source_file, initialization_node);
        let var_decl = factory.new_variable_declaration(temp_name, None, None, Some(cloned_init));
        let declarations = synthetic_node_list(factory, vec![var_decl]);
        let var_decl_list =
            factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        let var_stmt = factory.new_variable_statement(None, var_decl_list);

        let Some(statement) = ast::find_ancestor(
            self.source_file.store(),
            Some(target_node),
            |store, node| ast::is_statement(store, node),
        ) else {
            return String::new();
        };
        self.change_tracker.as_mut().unwrap().insert_node_before(
            self.source_file,
            statement,
            var_stmt,
            false,
            change::LEADING_TRIVIA_OPTION_NONE,
        );

        let type_query = self
            .change_tracker
            .as_mut()
            .unwrap()
            .node_factory
            .new_type_query_node(temp_name, None);
        let as_expr = self
            .change_tracker
            .as_mut()
            .unwrap()
            .node_factory
            .new_as_expression(temp_name, type_query);
        self.change_tracker.as_mut().unwrap().replace_node(
            self.source_file,
            replacement_target,
            as_expr,
            None,
        );

        let id_text = type_to_string_for_diag(
            temp_name,
            self.source_file,
            self.change_tracker.as_mut().unwrap(),
        );
        diagnostics::EXTRACT_TO_VARIABLE_AND_REPLACE_WITH_0_AS_TYPEOF_0
            .localize(self.locale.clone(), vec![Box::new(id_text)])
    }
}

pub fn is_expando_property_declaration_for_fix(
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> bool {
    node.is_some_and(|node| {
        ast::is_property_access_expression(store, node)
            || ast::is_element_access_expression(store, node)
            || ast::is_binary_expression(store, node)
    })
}

pub fn find_expando_function<'a>(
    checker: &mut checker::Checker<'a, '_>,
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> Option<ast::Node> {
    let node = node?;
    let expando_declaration = ast::find_ancestor_or_quit(store, Some(node), |store, n| {
        if ast::is_statement(store, n) {
            return ast::FindAncestorResult::Quit;
        }
        if is_expando_property_declaration_for_fix(store, Some(n)) {
            return ast::FindAncestorResult::True;
        }
        ast::FindAncestorResult::False
    })?;

    if !is_expando_property_declaration_for_fix(store, Some(expando_declaration)) {
        return None;
    }

    let mut assignment_target = expando_declaration;
    if ast::is_binary_expression(store, assignment_target) {
        assignment_target = store.left(assignment_target)?;
        if !is_expando_property_declaration_for_fix(store, Some(assignment_target)) {
            return None;
        }
    }

    let expression = if ast::is_property_access_expression(store, assignment_target) {
        store.expression(assignment_target)?
    } else if ast::is_element_access_expression(store, assignment_target) {
        store.expression(assignment_target)?
    } else {
        return None;
    };

    let target_type = checker.get_type_at_location(expression);
    let properties = checker.get_properties_of_type_public(target_type);
    let found = properties.into_iter().any(|p| {
        checker
            .symbol_value_declaration_public(p)
            .as_ref()
            .is_some_and(|value_declaration| {
                *value_declaration == expando_declaration
                    || store
                        .parent(expando_declaration)
                        .is_some_and(|parent| *value_declaration == parent)
            })
    });
    if !found {
        return None;
    }

    let symbol = checker.type_symbol_public(target_type)?;
    let fn_decl = checker.symbol_value_declaration_public(symbol)?;
    if (ast::is_function_expression(store, fn_decl) || ast::is_arrow_function(store, fn_decl))
        && store
            .parent(fn_decl)
            .is_some_and(|parent| ast::is_variable_declaration(store, parent))
    {
        return store.parent(fn_decl);
    }
    if ast::is_function_declaration(store, fn_decl) {
        return Some(fn_decl);
    }
    None
}

impl<'a, 'checker, 'state> IsolatedDeclarationsFixer<'a, 'checker, 'state> {
    pub(crate) fn fix_isolated_declaration_error(&mut self, node: ast::Node) -> String {
        let node_id = ast::get_node_id(self.source_file.store(), node);
        if self.fixed_nodes.contains(&node_id) {
            return String::new();
        }
        self.fixed_nodes.insert(node_id);

        match self.source_file.store().kind(node) {
            ast::Kind::Parameter
            | ast::Kind::PropertyDeclaration
            | ast::Kind::VariableDeclaration => self.add_type_to_variable_like(node),
            ast::Kind::ArrowFunction
            | ast::Kind::FunctionExpression
            | ast::Kind::FunctionDeclaration
            | ast::Kind::MethodDeclaration
            | ast::Kind::GetAccessor => self.add_type_to_signature_declaration(node),
            ast::Kind::ExportAssignment => self.transform_export_assignment(node),
            ast::Kind::ClassDeclaration => self.transform_extends_clause_with_expression(node),
            ast::Kind::ObjectBindingPattern | ast::Kind::ArrayBindingPattern => {
                self.transform_destructuring_patterns(node)
            }
            _ => String::new(),
        }
    }

    pub(crate) fn add_type_to_signature_declaration(&mut self, func_node: ast::Node) -> String {
        if self.source_file.store().type_node(func_node).is_some() {
            return String::new();
        }
        let Some(type_node) = self.infer_type(func_node, None) else {
            return String::new();
        };
        self.change_tracker
            .as_mut()
            .unwrap()
            .try_insert_type_annotation(self.source_file, func_node, type_node);
        diagnostics::ADD_RETURN_TYPE_0.localize(
            self.locale.clone(),
            vec![Box::new(type_to_string_for_diag(
                type_node,
                self.source_file,
                self.change_tracker.as_mut().unwrap(),
            ))],
        )
    }

    pub(crate) fn transform_export_assignment(&mut self, default_export: ast::Node) -> String {
        if self
            .source_file
            .store()
            .is_export_equals(default_export)
            .unwrap_or(false)
        {
            return String::new();
        }

        let Some(expression) = self.source_file.store().expression(default_export) else {
            return String::new();
        };
        let Some(type_node) = self.infer_type(expression, None) else {
            return String::new();
        };

        let default_identifier = self
            .change_tracker
            .as_mut()
            .unwrap()
            .emit_context
            .factory
            .new_unique_name("_default");
        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        let cloned_expression =
            deep_clone_for_change_factory(factory, self.source_file, expression);
        let var_decl = factory.new_variable_declaration(
            default_identifier,
            None,
            Some(type_node),
            Some(cloned_expression),
        );
        let declarations = synthetic_node_list(factory, vec![var_decl]);
        let var_decl_list =
            factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        let var_stmt = factory.new_variable_statement(None, var_decl_list);
        let modifiers = self
            .source_file
            .store()
            .modifiers(default_export)
            .map(|modifiers| {
                let cloned_modifiers: Vec<_> = modifiers
                    .nodes()
                    .into_iter()
                    .map(|modifier| {
                        deep_clone_for_change_factory(factory, self.source_file, modifier)
                    })
                    .collect();
                synthetic_modifier_list(factory, cloned_modifiers, modifiers.modifier_flags())
            });
        let new_export = factory.new_export_assignment(modifiers, false, None, default_identifier);

        self.change_tracker
            .as_mut()
            .unwrap()
            .replace_node_with_nodes(
                self.source_file,
                default_export,
                vec![var_stmt, new_export],
                None,
            );
        diagnostics::EXTRACT_DEFAULT_EXPORT_TO_VARIABLE.localize(self.locale.clone(), vec![])
    }

    pub(crate) fn transform_extends_clause_with_expression(
        &mut self,
        class_decl: ast::Node,
    ) -> String {
        let class_decl = &class_decl;
        let store = self.source_file.store();
        let extends_clause = store.heritage_clauses(*class_decl).and_then(|clauses| {
            clauses
                .iter()
                .find(|clause| store.token(*clause) == Some(ast::Kind::ExtendsKeyword))
        });
        let Some(extends_clause) = extends_clause else {
            return String::new();
        };
        let Some(heritage_types) = store.types(extends_clause) else {
            return String::new();
        };
        if heritage_types.is_empty() {
            return String::new();
        }
        let heritage_expression = heritage_types.first().unwrap();
        let Some(expression) = store.expression(heritage_expression) else {
            return String::new();
        };
        let Some(heritage_type_node) = self.infer_type(expression, None) else {
            return String::new();
        };

        let base_name = store
            .name(*class_decl)
            .map(|name| format!("{}Base", self.source_file.store().text(name)))
            .unwrap_or_else(|| "Anonymous".to_string());
        let base_class_name = self
            .change_tracker
            .as_mut()
            .unwrap()
            .emit_context
            .factory
            .new_unique_name_ex(
                &base_name,
                printer::AutoGenerateOptions {
                    flags: printer::GENERATED_IDENTIFIER_FLAGS_OPTIMISTIC,
                    ..Default::default()
                },
            );

        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        let cloned_expression =
            deep_clone_for_change_factory(factory, self.source_file, expression);
        let var_decl = factory.new_variable_declaration(
            base_class_name,
            None,
            Some(heritage_type_node),
            Some(cloned_expression),
        );
        let declarations = synthetic_node_list(factory, vec![var_decl]);
        let var_decl_list =
            factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        let var_stmt = factory.new_variable_statement(None, var_decl_list);
        let replacement = factory.new_expression_with_type_arguments(base_class_name, None);

        self.change_tracker.as_mut().unwrap().insert_node_before(
            self.source_file,
            *class_decl,
            var_stmt,
            false,
            change::LEADING_TRIVIA_OPTION_NONE,
        );

        self.change_tracker.as_mut().unwrap().replace_node(
            self.source_file,
            heritage_expression,
            replacement,
            None,
        );

        diagnostics::EXTRACT_BASE_CLASS_TO_VARIABLE.localize(self.locale.clone(), vec![])
    }

    pub(crate) fn transform_destructuring_patterns(
        &mut self,
        binding_pattern: ast::Node,
    ) -> String {
        let Some(enclosing_variable_declaration) = self.source_file.store().parent(binding_pattern)
        else {
            return String::new();
        };
        if !ast::is_variable_declaration(self.source_file.store(), enclosing_variable_declaration) {
            return String::new();
        }
        let Some(enclosing_var_stmt) = self
            .source_file
            .store()
            .parent(enclosing_variable_declaration)
            .and_then(|parent| self.source_file.store().parent(parent))
        else {
            return String::new();
        };
        if !ast::is_variable_statement(self.source_file.store(), enclosing_var_stmt) {
            return String::new();
        }

        let Some(initializer) = self
            .source_file
            .store()
            .initializer(enclosing_variable_declaration)
        else {
            return String::new();
        };

        let mut new_nodes = Vec::new();
        let base_expr_node;
        if !ast::is_identifier(self.source_file.store(), initializer) {
            let temp_name = self
                .change_tracker
                .as_mut()
                .unwrap()
                .emit_context
                .factory
                .new_unique_name_ex(
                    "dest",
                    printer::AutoGenerateOptions {
                        flags: printer::GENERATED_IDENTIFIER_FLAGS_OPTIMISTIC,
                        ..Default::default()
                    },
                );
            let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
            let cloned_initializer =
                deep_clone_for_change_factory(factory, self.source_file, initializer);
            let var_decl =
                factory.new_variable_declaration(temp_name, None, None, Some(cloned_initializer));
            let declarations = synthetic_node_list(factory, vec![var_decl]);
            let var_decl_list =
                factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
            let var_stmt = factory.new_variable_statement(None, var_decl_list);
            new_nodes.push(var_stmt);
            base_expr_node = temp_name;
        } else {
            base_expr_node = self
                .change_tracker
                .as_mut()
                .unwrap()
                .node_factory
                .new_identifier(self.source_file.store().text(initializer));
        }

        self.extract_binding_elements(
            binding_pattern,
            base_expr_node,
            &mut new_nodes,
            enclosing_var_stmt,
        );
        if new_nodes.is_empty() {
            return String::new();
        }

        let Some(decl_list_node) = self
            .source_file
            .store()
            .declaration_list(enclosing_var_stmt)
        else {
            return String::new();
        };
        let Some(declarations) = self.source_file.store().declarations(decl_list_node) else {
            return String::new();
        };
        if declarations.len() > 1 {
            let remaining_decls: Vec<_> = declarations
                .iter()
                .filter(|d| *d != enclosing_variable_declaration)
                .collect();
            if !remaining_decls.is_empty() {
                let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
                let declarations = synthetic_node_list(factory, remaining_decls);
                let var_decl_list = factory.new_variable_declaration_list(
                    declarations,
                    self.source_file.store().flags(decl_list_node),
                );
                let modifiers =
                    self.source_file
                        .store()
                        .modifiers(enclosing_var_stmt)
                        .map(|modifiers| {
                            let cloned_modifiers: Vec<_> = modifiers
                                .nodes()
                                .into_iter()
                                .map(|modifier| {
                                    deep_clone_for_change_factory(
                                        factory,
                                        self.source_file,
                                        modifier,
                                    )
                                })
                                .collect();
                            synthetic_modifier_list(
                                factory,
                                cloned_modifiers,
                                modifiers.modifier_flags(),
                            )
                        });
                new_nodes.push(factory.new_variable_statement(modifiers, var_decl_list));
            }
        }

        self.change_tracker
            .as_mut()
            .unwrap()
            .replace_node_with_nodes(self.source_file, enclosing_var_stmt, new_nodes, None);
        diagnostics::EXTRACT_BINDING_EXPRESSIONS_TO_VARIABLE.localize(self.locale.clone(), vec![])
    }

    pub fn extract_binding_elements(
        &mut self,
        binding_pattern: ast::Node,
        base_expr: ast::Node,
        new_nodes: &mut Vec<ast::Node>,
        enclosing_var_stmt: ast::Node,
    ) {
        let store = self.source_file.store();
        if ast::is_object_binding_pattern(store, binding_pattern) {
            let Some(elements) = store.elements(binding_pattern) else {
                return;
            };
            for element in elements {
                if ast::is_omitted_expression(store, element) {
                    continue;
                }
                let Some(name) = store.name(element) else {
                    continue;
                };
                let property_name = store.property_name(element);
                let access_expr = if property_name.is_some_and(|property_name| {
                    ast::is_computed_property_name(store, property_name)
                }) {
                    let Some(computed_expression) =
                        property_name.and_then(|name| store.expression(name))
                    else {
                        continue;
                    };
                    let identifier_for_computed_property = self
                        .change_tracker
                        .as_mut()
                        .unwrap()
                        .emit_context
                        .factory
                        .new_generated_name_for_node(
                            self.source_file.store(),
                            &computed_expression,
                        );
                    let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
                    let cloned_computed_expression = deep_clone_for_change_factory(
                        factory,
                        self.source_file,
                        computed_expression,
                    );
                    let comp_var_decl = factory.new_variable_declaration(
                        identifier_for_computed_property,
                        None,
                        None,
                        Some(cloned_computed_expression),
                    );
                    let comp_var_decls = synthetic_node_list(factory, vec![comp_var_decl]);
                    let comp_var_decl_list = factory
                        .new_variable_declaration_list(comp_var_decls, ast::NodeFlags::CONST);
                    new_nodes.push(factory.new_variable_statement(None, comp_var_decl_list));
                    factory.new_element_access_expression(
                        base_expr,
                        None,
                        identifier_for_computed_property,
                        ast::NodeFlags::NONE,
                    )
                } else if let Some(property_name) = property_name {
                    let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
                    let property_name =
                        factory.new_identifier(self.source_file.store().text(property_name));
                    factory.new_property_access_expression(
                        base_expr,
                        None,
                        property_name,
                        ast::NodeFlags::NONE,
                    )
                } else if ast::is_identifier(store, name) {
                    let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
                    let property_name = factory.new_identifier(self.source_file.store().text(name));
                    factory.new_property_access_expression(
                        base_expr,
                        None,
                        property_name,
                        ast::NodeFlags::NONE,
                    )
                } else {
                    continue;
                };

                if ast::is_binding_pattern(store, name) {
                    self.extract_binding_elements(name, access_expr, new_nodes, enclosing_var_stmt);
                } else {
                    self.emit_binding_element_variable(
                        name,
                        element,
                        access_expr,
                        new_nodes,
                        enclosing_var_stmt,
                    );
                }
            }
        } else if ast::is_array_binding_pattern(store, binding_pattern) {
            let Some(elements) = store.elements(binding_pattern) else {
                return;
            };
            for (i, element) in elements.into_iter().enumerate() {
                if ast::is_omitted_expression(store, element) {
                    continue;
                }
                let Some(name) = store.name(element) else {
                    continue;
                };
                let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
                let index = factory.new_numeric_literal(i.to_string(), ast::TokenFlags::NONE);
                let access_expr = factory.new_element_access_expression(
                    base_expr,
                    None,
                    index,
                    ast::NodeFlags::NONE,
                );
                if ast::is_binding_pattern(store, name) {
                    self.extract_binding_elements(name, access_expr, new_nodes, enclosing_var_stmt);
                } else {
                    self.emit_binding_element_variable(
                        name,
                        element,
                        access_expr,
                        new_nodes,
                        enclosing_var_stmt,
                    );
                }
            }
        }
    }

    pub fn emit_binding_element_variable(
        &mut self,
        name: ast::Node,
        binding_element: ast::Node,
        access_expr: ast::Node,
        new_nodes: &mut Vec<ast::Node>,
        enclosing_var_stmt: ast::Node,
    ) {
        let type_node = self.infer_type(name, None);
        let mut variable_initializer = access_expr;
        let export_modifier = self.get_export_modifier(enclosing_var_stmt);

        if let Some(initializer) = self.source_file.store().initializer(binding_element) {
            let temp_base_name = self
                .source_file
                .store()
                .property_name(binding_element)
                .filter(|property_name| {
                    ast::is_identifier(self.source_file.store(), *property_name)
                })
                .map(|property_name| self.source_file.store().text(property_name))
                .unwrap_or_else(|| "temp".to_string());
            let temp_name = self
                .change_tracker
                .as_mut()
                .unwrap()
                .emit_context
                .factory
                .new_unique_name_ex(
                    &temp_base_name,
                    printer::AutoGenerateOptions {
                        flags: printer::GENERATED_IDENTIFIER_FLAGS_OPTIMISTIC,
                        ..Default::default()
                    },
                );
            let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
            let temp_var_decl =
                factory.new_variable_declaration(temp_name, None, None, Some(variable_initializer));
            let declarations = synthetic_node_list(factory, vec![temp_var_decl]);
            let temp_var_decl_list =
                factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
            new_nodes.push(factory.new_variable_statement(None, temp_var_decl_list));

            let equals_token = factory.new_token(ast::Kind::EqualsEqualsEqualsToken);
            let undefined = factory.new_identifier("undefined".to_string());
            let condition =
                factory.new_binary_expression(None, temp_name, None, equals_token, undefined);
            let question_token = factory.new_token(ast::Kind::QuestionToken);
            let colon_token = factory.new_token(ast::Kind::ColonToken);
            let cloned_initializer =
                deep_clone_for_change_factory(factory, self.source_file, initializer);
            variable_initializer = factory.new_conditional_expression(
                condition,
                question_token,
                cloned_initializer,
                colon_token,
                variable_initializer,
            );
        }

        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        let var_name = factory.new_identifier(self.source_file.store().text(name));
        let var_decl =
            factory.new_variable_declaration(var_name, None, type_node, Some(variable_initializer));
        let var_decls = synthetic_node_list(factory, vec![var_decl]);
        let var_decl_list = factory.new_variable_declaration_list(var_decls, ast::NodeFlags::CONST);
        new_nodes.push(factory.new_variable_statement(export_modifier, var_decl_list));
    }

    pub fn get_export_modifier(
        &mut self,
        enclosing_var_stmt: ast::Node,
    ) -> Option<ast::ModifierList> {
        if ast::has_syntactic_modifier(
            self.source_file.store(),
            enclosing_var_stmt,
            ast::ModifierFlags::Export,
        ) {
            let export_token = self
                .change_tracker
                .as_mut()
                .unwrap()
                .node_factory
                .new_token(ast::Kind::ExportKeyword);
            return Some(synthetic_modifier_list(
                &mut self.change_tracker.as_mut().unwrap().node_factory,
                vec![export_token],
                ast::MODIFIER_FLAGS_EXPORT,
            ));
        }
        None
    }

    pub(crate) fn infer_type(
        &mut self,
        node: ast::Node,
        variable_type: Option<checker::TypeHandle>,
    ) -> Option<ast::Node> {
        self.mutated_target = false;
        let type_print_mode = self.type_print_mode;
        if type_print_mode == TypePrintMode::Relative {
            return self.relative_type(node);
        }

        let mut t = if is_value_signature_declaration(self.source_file.store(), Some(node)) {
            let signature = self
                .checker_mut()
                .get_signature_from_declaration_public(node);
            if let Some(type_predicate) = self
                .checker_mut()
                .get_type_predicate_of_signature_public(signature)
            {
                let predicate_type = self
                    .checker_mut()
                    .type_predicate_type_public(type_predicate)?;
                let enclosing_decl =
                    ast::find_ancestor(self.source_file.store(), Some(node), |store, node| {
                        ast::is_declaration(store, node)
                    })
                    .unwrap_or_else(|| self.source_file.as_node());
                let mut flags = DECLARATION_EMIT_NODE_BUILDER_FLAGS;
                if self.checker_mut().type_flags_public(predicate_type)
                    & checker::TYPE_FLAGS_UNIQUE_ES_SYMBOL
                    != 0
                {
                    flags |= nodebuilder::FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE;
                }
                return self
                    .checker_mut()
                    .type_predicate_to_type_predicate_node_for_ls_public(
                        type_predicate,
                        Some(enclosing_decl),
                        flags,
                    );
            }
            self.checker_mut()
                .get_return_type_of_signature_public(signature)
        } else {
            self.checker_mut().get_type_at_location(node)
        };

        if type_print_mode == TypePrintMode::Widened {
            if let Some(variable_type) = variable_type {
                t = variable_type;
            }
            let widened_type = self.checker_mut().get_widened_literal_type_public(t);
            if self
                .checker_mut()
                .is_type_assignable_to_public(widened_type, t)
            {
                return None;
            }
            t = widened_type;
        }

        let enclosing_decl =
            ast::find_ancestor(self.source_file.store(), Some(node), |store, node| {
                ast::is_declaration(store, node)
            })
            .unwrap_or_else(|| self.source_file.as_node());
        let flags = DECLARATION_EMIT_NODE_BUILDER_FLAGS | self.get_extra_flags(&node, t);

        if ast::is_parameter_declaration(self.source_file.store(), node)
            && self
                .checker_mut()
                .requires_adding_implicit_undefined_public(node)
        {
            let undefined_type = self.checker_mut().get_undefined_type();
            t = self
                .checker_mut()
                .get_union_type_ex_public(vec![undefined_type, t], checker::UNION_REDUCTION_NONE);
        }

        self.type_to_minimized_reference_type(t, enclosing_decl, flags)
    }

    fn get_extra_flags(&mut self, node: &ast::Node, t: checker::TypeHandle) -> nodebuilder::Flags {
        if (ast::is_variable_declaration(self.source_file.store(), *node)
            || (ast::is_property_declaration(self.source_file.store(), *node)
                && ast::has_syntactic_modifier(
                    self.source_file.store(),
                    *node,
                    ast::ModifierFlags::Static | ast::ModifierFlags::Readonly,
                )))
            && self.checker.type_flags_public(t) & checker::TYPE_FLAGS_UNIQUE_ES_SYMBOL != 0
        {
            return nodebuilder::FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE;
        }
        nodebuilder::FLAGS_NONE
    }

    pub(crate) fn create_type_of_from_entity_name_expression(
        &mut self,
        node: ast::Node,
    ) -> ast::Node {
        self.change_tracker
            .as_mut()
            .unwrap()
            .node_factory
            .new_type_query_node(node, None)
    }

    pub fn type_from_array_spread_elements(
        &mut self,
        node: ast::Node,
        name: &str,
    ) -> Option<ast::Node> {
        let is_in_const_context =
            ast::find_ancestor(self.source_file.store(), Some(node), |store, node| {
                is_const_assertion(store, node)
            })
            .is_some();
        if !is_in_const_context {
            return None;
        }
        let name = if name.is_empty() { "temp" } else { name };
        self.type_from_spreads(
            node,
            name,
            is_in_const_context,
            |store, n| {
                store
                    .elements(n)
                    .map(|elements| elements.iter().collect())
                    .unwrap_or_default()
            },
            |store, node| ast::is_spread_element(store, node),
            |factory, expr| factory.new_spread_element(expr),
            |factory, elements| {
                let list = synthetic_node_list(factory, elements);
                factory.new_array_literal_expression(list, true)
            },
            |factory, types| {
                let rest_types: Vec<_> = types
                    .into_iter()
                    .map(|t| factory.new_rest_type_node(t))
                    .collect();
                let list = synthetic_node_list(factory, rest_types);
                factory.new_tuple_type_node(list)
            },
        )
    }

    pub fn type_from_object_spread_assignment(
        &mut self,
        node: ast::Node,
        name: &str,
    ) -> Option<ast::Node> {
        let is_in_const_context =
            ast::find_ancestor(self.source_file.store(), Some(node), |store, node| {
                is_const_assertion(store, node)
            })
            .is_some();
        let name = if name.is_empty() { "temp" } else { name };
        self.type_from_spreads(
            node,
            name,
            is_in_const_context,
            |store, n| {
                store
                    .properties(n)
                    .map(|properties| properties.iter().collect())
                    .unwrap_or_default()
            },
            |store, node| ast::is_spread_assignment(store, node),
            |factory, expr| factory.new_spread_assignment(expr),
            |factory, elements| {
                let list = synthetic_node_list(factory, elements);
                factory.new_object_literal_expression(list, true)
            },
            |factory, types| {
                let list = synthetic_node_list(factory, types);
                factory.new_intersection_type_node(list)
            },
        )
    }

    pub fn type_from_spreads(
        &mut self,
        node: ast::Node,
        name: &str,
        is_in_const_context: bool,
        get_children: impl Fn(&ast::AstStore, ast::Node) -> Vec<ast::Node>,
        is_spread: impl Fn(&ast::AstStore, ast::Node) -> bool,
        create_spread: impl Fn(&mut ast::NodeFactory, ast::Node) -> ast::Node,
        make_node_of_kind: impl Fn(&mut ast::NodeFactory, Vec<ast::Node>) -> ast::Node,
        final_type: impl Fn(&mut ast::NodeFactory, Vec<ast::Node>) -> ast::Node,
    ) -> Option<ast::Node> {
        let mut intersection_types = Vec::new();
        let mut new_spreads = Vec::new();
        let mut current_variable_properties = Vec::new();

        let statement = ast::find_ancestor(self.source_file.store(), Some(node), |store, node| {
            ast::is_statement(store, node)
        });
        let children = get_children(self.source_file.store(), node);
        for prop in children {
            if is_spread(self.source_file.store(), prop) {
                self.finalizes_variable_part(
                    name,
                    is_in_const_context,
                    statement,
                    &make_node_of_kind,
                    &create_spread,
                    &mut current_variable_properties,
                    &mut intersection_types,
                    &mut new_spreads,
                );
                let Some(prop_expression) = self.source_file.store().expression(prop) else {
                    continue;
                };
                if ast::is_entity_name_expression(self.source_file.store(), prop_expression) {
                    intersection_types
                        .push(self.create_type_of_from_entity_name_expression(prop_expression));
                    new_spreads.push(prop);
                } else {
                    self.make_spread_variable(
                        name,
                        is_in_const_context,
                        statement,
                        &create_spread,
                        prop_expression,
                        &mut intersection_types,
                        &mut new_spreads,
                    );
                }
            } else {
                current_variable_properties.push(prop);
            }
        }

        if new_spreads.is_empty() {
            return None;
        }

        self.finalizes_variable_part(
            name,
            is_in_const_context,
            statement,
            &make_node_of_kind,
            &create_spread,
            &mut current_variable_properties,
            &mut intersection_types,
            &mut new_spreads,
        );

        let replacement = {
            let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
            make_node_of_kind(factory, new_spreads)
        };
        self.change_tracker.as_mut().unwrap().replace_node(
            self.source_file,
            node,
            replacement,
            None,
        );
        self.mutated_target = true;

        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        Some(final_type(factory, intersection_types))
    }

    pub fn make_spread_variable(
        &mut self,
        name: &str,
        is_in_const_context: bool,
        statement: Option<ast::Node>,
        create_spread: impl Fn(&mut ast::NodeFactory, ast::Node) -> ast::Node,
        expression: ast::Node,
        intersection_types: &mut Vec<ast::Node>,
        new_spreads: &mut Vec<ast::Node>,
    ) {
        let temp_base_name = format!("{}_Part{}", name, new_spreads.len() + 1);
        let temp_name = self
            .change_tracker
            .as_mut()
            .unwrap()
            .emit_context
            .factory
            .new_unique_name_ex(
                &temp_base_name,
                printer::AutoGenerateOptions {
                    flags: printer::GENERATED_IDENTIFIER_FLAGS_OPTIMISTIC,
                    ..Default::default()
                },
            );

        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        let initializer = if !is_in_const_context {
            deep_clone_for_change_factory(factory, self.source_file, expression)
        } else {
            let const_id = factory.new_identifier("const".to_string());
            let const_ref = factory.new_type_reference_node(const_id, None);
            let cloned = deep_clone_for_change_factory(factory, self.source_file, expression);
            factory.new_as_expression(cloned, const_ref)
        };

        let var_decl = factory.new_variable_declaration(temp_name, None, None, Some(initializer));
        let declarations = synthetic_node_list(factory, vec![var_decl]);
        let var_decl_list =
            factory.new_variable_declaration_list(declarations, ast::NodeFlags::CONST);
        let var_stmt = factory.new_variable_statement(None, var_decl_list);

        if let Some(statement) = statement {
            self.change_tracker.as_mut().unwrap().insert_node_before(
                self.source_file,
                statement,
                var_stmt,
                false,
                change::LEADING_TRIVIA_OPTION_NONE,
            );
        }

        intersection_types.push(self.create_type_of_from_entity_name_expression(temp_name));
        let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
        new_spreads.push(create_spread(factory, temp_name));
    }

    pub fn finalizes_variable_part(
        &mut self,
        name: &str,
        is_in_const_context: bool,
        statement: Option<ast::Node>,
        make_node_of_kind: impl Fn(&mut ast::NodeFactory, Vec<ast::Node>) -> ast::Node,
        create_spread: impl Fn(&mut ast::NodeFactory, ast::Node) -> ast::Node,
        current_variable_properties: &mut Vec<ast::Node>,
        intersection_types: &mut Vec<ast::Node>,
        new_spreads: &mut Vec<ast::Node>,
    ) {
        if !current_variable_properties.is_empty() {
            let expression = {
                let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
                make_node_of_kind(factory, std::mem::take(current_variable_properties))
            };
            self.make_spread_variable(
                name,
                is_in_const_context,
                statement,
                create_spread,
                expression,
                intersection_types,
                new_spreads,
            );
        }
    }
}

pub(crate) fn is_const_assertion(store: &ast::AstStore, node: ast::Node) -> bool {
    if ast::is_assertion_expression(store, &node) {
        return store
            .type_node(node)
            .as_ref()
            .is_some_and(|node| ast::is_const_type_reference(store, node));
    }
    false
}

impl<'a, 'checker, 'state> IsolatedDeclarationsFixer<'a, 'checker, 'state> {
    pub(crate) fn relative_type(&mut self, node: ast::Node) -> Option<ast::Node> {
        if ast::is_parameter_declaration(self.source_file.store(), node) {
            return None;
        }
        if ast::is_shorthand_property_assignment(self.source_file.store(), node) {
            let name_node = self.source_file.store().name(node)?;
            return Some(self.create_type_of_from_entity_name_expression(name_node));
        }
        if ast::is_entity_name_expression(self.source_file.store(), node) {
            return Some(self.create_type_of_from_entity_name_expression(node));
        }
        if is_const_assertion(self.source_file.store(), node) {
            let expression = self.source_file.store().expression(node)?;
            return self.relative_type(expression);
        }
        if ast::is_array_literal_expression(self.source_file.store(), node) {
            let var_decl = ast::find_ancestor_kind(
                self.source_file.store(),
                Some(node),
                ast::Kind::VariableDeclaration,
            );
            let part_name = var_decl
                .and_then(|var_decl| self.source_file.store().name(var_decl))
                .filter(|name| ast::is_identifier(self.source_file.store(), *name))
                .map(|name| self.source_file.store().text(name))
                .unwrap_or_default();
            return self.type_from_array_spread_elements(node, &part_name);
        }
        if ast::is_object_literal_expression(self.source_file.store(), node) {
            let var_decl = ast::find_ancestor_kind(
                self.source_file.store(),
                Some(node),
                ast::Kind::VariableDeclaration,
            );
            let part_name = var_decl
                .and_then(|var_decl| self.source_file.store().name(var_decl))
                .filter(|name| ast::is_identifier(self.source_file.store(), *name))
                .map(|name| self.source_file.store().text(name))
                .unwrap_or_default();
            return self.type_from_object_spread_assignment(node, &part_name);
        }
        if ast::is_variable_declaration(self.source_file.store(), node)
            && self.source_file.store().initializer(node).is_some()
        {
            let initializer = self.source_file.store().initializer(node).unwrap();
            return self.relative_type(initializer);
        }
        if ast::is_conditional_expression(self.source_file.store(), node) {
            let true_expr = self.source_file.store().when_true(node)?;
            let false_expr = self.source_file.store().when_false(node)?;
            let true_type = self.relative_type(true_expr)?;
            let true_mutated = self.mutated_target;
            let false_type = self.relative_type(false_expr)?;
            self.mutated_target = true_mutated || self.mutated_target;
            let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
            let node_list = synthetic_node_list(factory, vec![true_type, false_type]);
            return Some(factory.new_union_type_node(node_list));
        }
        None
    }

    pub fn type_to_minimized_reference_type(
        &mut self,
        ty: checker::TypeHandle,
        enclosing_decl: ast::Node,
        flags: nodebuilder::Flags,
    ) -> Option<ast::Node> {
        let (mut type_node, id_to_symbol) = {
            let change_tracker = self.change_tracker.as_mut()?;
            let (type_node, id_to_symbol) = self.checker.type_to_type_node_for_ls_public(
                &mut change_tracker.emit_context,
                ty,
                Some(enclosing_decl),
                flags,
                nodebuilder::INTERNAL_FLAGS_WRITE_COMPUTED_PROPS,
            );
            (type_node?, id_to_symbol)
        };

        if ast::is_type_reference_node(
            self.change_tracker
                .as_ref()
                .unwrap()
                .emit_context
                .factory
                .node_factory
                .store(),
            type_node,
        ) && self.checker_mut().object_flags_public(ty) & checker::OBJECT_FLAGS_REFERENCE != 0
        {
            let type_args = self.checker_mut().get_type_arguments_public(ty);
            let node_type_args = self
                .change_tracker
                .as_ref()
                .unwrap()
                .emit_context
                .factory
                .node_factory
                .store()
                .type_arguments(type_node);
            if !type_args.is_empty() && node_type_args.is_some_and(|args| !args.is_empty()) {
                let source_file = self.source_file;
                let cutoff =
                    end_of_required_type_parameters(self.checker_mut(), source_file.store(), ty);
                let node_type_args = self
                    .change_tracker
                    .as_ref()
                    .unwrap()
                    .emit_context
                    .factory
                    .node_factory
                    .store()
                    .type_arguments(type_node)
                    .unwrap();
                if cutoff < node_type_args.len() {
                    let args_to_clone: Vec<_> = node_type_args.iter().take(cutoff).collect();
                    let change_tracker = self.change_tracker.as_mut().unwrap();
                    let source = change_tracker.emit_context.factory.node_factory.store();
                    let factory = &mut change_tracker.node_factory;
                    let cloned_args: Vec<_> = args_to_clone
                        .into_iter()
                        .map(|arg| factory.deep_clone_node_from_store(source, arg))
                        .collect();
                    let trimmed_args = synthetic_node_list(factory, cloned_args);
                    let type_name = source.type_name(type_node)?;
                    let type_name = factory.deep_clone_node_from_store(source, type_name);
                    type_node = factory.update_type_reference_node(
                        type_node,
                        type_name,
                        Some(trimmed_args),
                    );
                }
            }
        }

        let result = {
            let change_tracker = self.change_tracker.as_mut().unwrap();
            if type_node.store_id() == change_tracker.node_factory.store().store_id() {
                let mut conversion_factory =
                    ast::NodeFactory::new(ast::NodeFactoryHooks::default());
                let conversion_result =
                    autoimport::try_get_auto_importable_reference_from_type_node_from_identifiers(
                        change_tracker.node_factory.store(),
                        &mut conversion_factory,
                        &type_node,
                        id_to_symbol,
                    );
                let type_node = change_tracker.node_factory.deep_clone_node_from_store(
                    conversion_factory.store(),
                    conversion_result.type_node,
                );
                autoimport::import_adder::AutoImportableReferenceTypeNode {
                    type_node,
                    symbols: conversion_result.symbols,
                    converted: conversion_result.converted,
                }
            } else {
                let source = change_tracker.emit_context.factory.node_factory.store();
                autoimport::try_get_auto_importable_reference_from_type_node_from_identifiers(
                    source,
                    &mut change_tracker.node_factory,
                    &type_node,
                    id_to_symbol,
                )
            }
        };
        if result.converted {
            self.symbols_to_import.extend(result.symbols);
        }
        Some(result.type_node)
    }
}

pub fn end_of_required_type_parameters<'a>(
    checker: &mut checker::Checker<'a, '_>,
    store: &ast::AstStore,
    ty: checker::TypeHandle,
) -> usize {
    let type_args = checker.get_type_arguments_public(ty);
    if type_args.is_empty() {
        return 0;
    }
    let target = checker.type_target_public(ty);
    if !checker.is_interface_type_public(target) {
        return type_args.len();
    }
    let type_params = checker.interface_type_parameters_public(target);
    let local_type_params = checker.interface_local_type_parameters_public(target);
    let outer_count = type_params.len() - local_type_params.len();
    for cutoff in 0..type_args.len() {
        let local_idx = cutoff as isize - outer_count as isize;
        if local_idx < 0
            || local_idx as usize >= local_type_params.len()
            || !type_param_has_default(checker, store, local_type_params[local_idx as usize])
        {
            continue;
        }
        let filled_in = checker.fill_missing_type_arguments_public(
            type_args[..cutoff].to_vec(),
            type_params.clone(),
            cutoff,
            false,
        );
        let mut all_match = true;
        for (i, fill) in filled_in.iter().enumerate() {
            if *fill != type_args[i] {
                all_match = false;
                break;
            }
        }
        if all_match {
            return cutoff;
        }
    }
    type_args.len()
}

pub fn type_param_has_default(
    checker: &mut checker::Checker<'_, '_>,
    store: &ast::AstStore,
    tp: checker::TypeHandle,
) -> bool {
    let Some(sym) = checker.type_symbol_public(tp) else {
        return false;
    };
    checker
        .collect_symbol_declarations_public(sym)
        .iter()
        .any(|decl| {
            ast::is_type_parameter_declaration(store, *decl) && store.default_type(*decl).is_some()
        })
}

impl<'a, 'checker, 'state> IsolatedDeclarationsFixer<'a, 'checker, 'state> {
    pub(crate) fn add_type_to_variable_like(&mut self, decl: ast::Node) -> String {
        let Some(type_node) = self.infer_type(decl, None) else {
            return String::new();
        };
        if let Some(existing_type) = self.source_file.store().type_node(decl) {
            self.change_tracker.as_mut().unwrap().replace_node(
                self.source_file,
                existing_type,
                type_node,
                None,
            );
        } else {
            self.change_tracker
                .as_mut()
                .unwrap()
                .try_insert_type_annotation(self.source_file, decl, type_node);
            if ast::is_parameter_declaration(self.source_file.store(), decl)
                && self
                    .source_file
                    .store()
                    .parent(decl)
                    .is_some_and(|parent| ast::is_arrow_function(self.source_file.store(), parent))
            {
                let parent = self.source_file.store().parent(decl).unwrap();
                self.change_tracker
                    .as_mut()
                    .unwrap()
                    .parenthesize_arrow_parameters(self.source_file, parent);
            }
        }
        diagnostics::ADD_ANNOTATION_OF_TYPE_0.localize(
            self.locale.clone(),
            vec![Box::new(type_to_string_for_diag(
                type_node,
                self.source_file,
                self.change_tracker.as_mut().unwrap(),
            ))],
        )
    }
}

pub fn type_to_string_for_diag(
    type_node: ast::Node,
    source_file: &ast::SourceFile,
    change_tracker: &mut change::Tracker,
) -> String {
    let saved_flags = change_tracker.emit_context.emit_flags(&type_node);
    change_tracker
        .emit_context
        .set_emit_flags(&type_node, saved_flags | printer::EF_SINGLE_LINE);
    let mut p = printer::new_printer(
        printer::PrinterOptions {
            new_line: core::NewLineKind::LF,
            ..Default::default()
        },
        printer::PrintHandlers::default(),
        Some(change_tracker.emit_context.fork()),
    );
    let result = p.emit(&type_node, Some(source_file));
    change_tracker
        .emit_context
        .set_emit_flags(&type_node, saved_flags);
    if result.len() > 160 {
        format!("{}...", &result[..157])
    } else {
        result
    }
}

pub fn find_ancestor_with_missing_type(
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> Option<ast::Node> {
    let node = node?;
    ast::find_ancestor(store, Some(node), |store, n| {
        if !can_have_type_annotation_kinds().contains(&store.kind(n)) {
            return false;
        }
        if ast::is_object_binding_pattern(store, n) || ast::is_array_binding_pattern(store, n) {
            return store
                .parent(n)
                .is_some_and(|parent| ast::is_variable_declaration(store, parent));
        }
        true
    })
}

pub fn find_best_fitting_node(
    store: &ast::AstStore,
    node: Option<ast::Node>,
    span: core::TextRange,
) -> Option<ast::Node> {
    let mut node = node?;
    while store.loc(node).end() < span.pos() + span.len() {
        let parent = store.parent(node)?;
        node = parent;
    }
    while store.parent(node).is_some_and(|parent| {
        store.loc(parent).pos() == store.loc(node).pos()
            && store.loc(parent).end() == store.loc(node).end()
    }) {
        let parent = store.parent(node)?;
        node = parent;
    }
    if ast::is_identifier(store, node)
        && store
            .parent(node)
            .as_ref()
            .is_some_and(|parent| ast::has_initializer(store, parent))
        && store
            .parent(node)
            .and_then(|parent| store.initializer(parent))
            .is_some()
    {
        return store
            .parent(node)
            .and_then(|parent| store.initializer(parent));
    }
    if ast::is_identifier(store, node)
        && store
            .parent(node)
            .as_ref()
            .is_some_and(|parent| ast::is_shorthand_property_assignment(store, *parent))
    {
        return store.parent(node);
    }
    Some(node)
}

pub(crate) fn is_named_declaration_kind(store: &ast::AstStore, node: ast::Node) -> bool {
    matches!(
        store.kind(node),
        ast::Kind::ArrowFunction
            | ast::Kind::BindingElement
            | ast::Kind::ClassDeclaration
            | ast::Kind::ClassExpression
            | ast::Kind::ClassStaticBlockDeclaration
            | ast::Kind::Constructor
            | ast::Kind::EnumDeclaration
            | ast::Kind::EnumMember
            | ast::Kind::ExportSpecifier
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::GetAccessor
            | ast::Kind::ImportClause
            | ast::Kind::ImportEqualsDeclaration
            | ast::Kind::ImportSpecifier
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::JsxAttribute
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::ModuleDeclaration
            | ast::Kind::NamespaceExportDeclaration
            | ast::Kind::NamespaceImport
            | ast::Kind::NamespaceExport
            | ast::Kind::Parameter
            | ast::Kind::PropertyAssignment
            | ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
            | ast::Kind::SetAccessor
            | ast::Kind::ShorthandPropertyAssignment
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::TypeParameter
            | ast::Kind::VariableDeclaration
            | ast::Kind::NamedTupleMember
    )
}

pub(crate) fn is_value_signature_declaration(
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> bool {
    node.is_some_and(|node| {
        ast::is_function_expression(store, node)
            || ast::is_arrow_function(store, node)
            || ast::is_method_declaration(store, node)
            || ast::is_accessor(store, &node)
            || ast::is_function_declaration(store, node)
            || ast::is_constructor_declaration(store, node)
    })
}

pub(crate) fn get_identifier_name_for_node(store: &ast::AstStore, node: ast::Node) -> String {
    if ast::is_property_access_expression(store, node) {
        if let Some(name) = store.name(node) {
            if ast::is_identifier(store, name)
                && !ast::is_private_identifier(store, name)
                && scanner::identifier_to_keyword_kind(store, name) == ast::Kind::Unknown
            {
                return store.text(name);
            }
        }
    }
    "newLocal".to_string()
}

impl<'a, 'checker, 'state> IsolatedDeclarationsFixer<'a, 'checker, 'state> {
    pub(crate) fn add_symbol_to_existing_import(&mut self, sym: ast::SymbolIdentity) {
        let (module_symbol, symbol_name) = {
            let Some(symbol_name) = self.checker_mut().symbol_name_public(sym) else {
                return;
            };
            let Some(module_symbol) = self.checker_mut().symbol_parent_public(sym) else {
                return;
            };
            (module_symbol, symbol_name)
        };

        let Some(statements) = self
            .source_file
            .store()
            .statements(self.source_file.as_node())
        else {
            return;
        };
        for stmt in statements {
            if !ast::is_import_declaration(self.source_file.store(), stmt) {
                continue;
            }
            let Some(import_clause_node) = self.source_file.store().import_clause(stmt) else {
                continue;
            };
            let Some(module_specifier) = self.source_file.store().module_specifier(stmt) else {
                continue;
            };
            let Some(import_module_symbol) = self
                .checker_mut()
                .get_symbol_at_location_public(module_specifier)
            else {
                continue;
            };
            let Some(import_module_target) = self
                .checker_mut()
                .get_merged_symbol_public(import_module_symbol)
            else {
                continue;
            };
            let Some(module_target) = self.checker_mut().get_merged_symbol_public(module_symbol)
            else {
                continue;
            };
            if import_module_target != module_target {
                continue;
            }

            if self
                .source_file
                .store()
                .named_bindings(import_clause_node)
                .is_some_and(|named_bindings| {
                    ast::is_named_imports(self.source_file.store(), named_bindings)
                })
            {
                let named_bindings = self
                    .source_file
                    .store()
                    .named_bindings(import_clause_node)
                    .unwrap();
                let existing_elements = self.source_file.store().elements(named_bindings);
                let factory = &mut self.change_tracker.as_mut().unwrap().node_factory;
                let specifier_name = factory.new_identifier(symbol_name.clone());
                let new_specifier = factory.new_import_specifier(false, None, specifier_name);
                let mut new_elements: Vec<_> = existing_elements
                    .map(|elements| {
                        elements
                            .iter()
                            .map(|element| {
                                deep_clone_for_change_factory(factory, self.source_file, element)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                new_elements.push(new_specifier);
                let import_elements = synthetic_node_list(factory, new_elements);
                let new_named_imports = factory.new_named_imports(import_elements);
                let import_name = self
                    .source_file
                    .store()
                    .name(import_clause_node)
                    .map(|name| deep_clone_for_change_factory(factory, self.source_file, name));
                let new_import_clause = factory.new_import_clause(
                    self.source_file.store().phase_modifier(import_clause_node),
                    import_name,
                    Some(new_named_imports),
                );
                let module_specifier =
                    deep_clone_for_change_factory(factory, self.source_file, module_specifier);
                let attributes = self.source_file.store().attributes(stmt).map(|attributes| {
                    deep_clone_for_change_factory(factory, self.source_file, attributes)
                });
                let modifiers = self.source_file.store().modifiers(stmt).map(|modifiers| {
                    let cloned_modifiers: Vec<_> = modifiers
                        .nodes()
                        .into_iter()
                        .map(|modifier| {
                            deep_clone_for_change_factory(factory, self.source_file, modifier)
                        })
                        .collect();
                    synthetic_modifier_list(factory, cloned_modifiers, modifiers.modifier_flags())
                });
                let new_import_decl = factory.new_import_declaration(
                    modifiers,
                    Some(new_import_clause),
                    module_specifier,
                    attributes,
                );
                self.change_tracker.as_mut().unwrap().replace_node(
                    self.source_file,
                    stmt,
                    new_import_decl,
                    None,
                );
            }
            return;
        }
    }
}
