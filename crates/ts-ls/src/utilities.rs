use std::collections::HashSet;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_debug as debug;
use ts_evaluator as evaluator;
use ts_format as format;
use ts_jsnum as jsnum;
use ts_lsproto as lsproto;
use ts_scanner as scanner;
use ts_stringutil as stringutil;
use ts_tspath as tspath;

use crate::LanguageService;
use crate::lsconv;

use crate::lsutil;

#[inline]
fn node_handle(node: &ast::Node) -> ast::Node {
    *node
}

pub(crate) fn source_node_symbol_handle_from_program(
    program: &dyn checker::Program,
    source_file: &ast::SourceFile,
    node: ast::Node,
) -> Option<ast::SymbolHandle> {
    if node.store_id() != source_file.store().store_id() {
        return None;
    }
    let binding_state = program.binding_state(source_file);
    if node == source_file.as_node() {
        binding_state.source_symbol()
    } else {
        binding_state.symbol(node)
    }
}

pub(crate) fn source_node_symbol_from_program(
    program: &dyn checker::Program,
    source_file: &ast::SourceFile,
    node: ast::Node,
) -> Option<ast::SymbolIdentity> {
    source_node_symbol_handle_from_program(program, source_file, node)
        .map(ast::SymbolIdentity::from_symbol_handle)
}

pub(crate) fn source_node_symbol_declarations_snapshot_from_program(
    program: &dyn checker::Program,
    source_file: &ast::SourceFile,
    node: ast::Node,
) -> Vec<ast::Node> {
    let Some(symbol) = source_node_symbol_handle_from_program(program, source_file, node) else {
        return Vec::new();
    };
    program
        .binding_state(source_file)
        .with_symbol_declarations(symbol, |declarations| declarations.to_vec())
}

pub(crate) fn source_node_symbol_parent_from_program(
    program: &dyn checker::Program,
    source_file: &ast::SourceFile,
    node: ast::Node,
) -> Option<ast::SymbolIdentity> {
    let symbol = source_node_symbol_handle_from_program(program, source_file, node)?;
    program
        .binding_state(source_file)
        .symbol_parent(symbol)
        .map(ast::SymbolIdentity::from_symbol_handle)
}

pub(crate) fn is_in_string(
    source_file: &ast::SourceFile,
    position: i32,
    previous_token: Option<&ast::Node>,
) -> bool {
    if let Some(previous_token) = previous_token {
        let store = source_file.store();
        if ast::is_string_text_containing_node(store, previous_token) {
            let start = astnav::get_start_of_node(*previous_token, source_file);
            let end = store.loc(*previous_token).end();
            if start < position && position < end {
                return true;
            }
            if position == end {
                return ast::is_unterminated_literal(source_file.store(), *previous_token);
            }
        }
    }
    false
}

pub(crate) fn is_module_specifier_like(store: &ast::AstStore, node: ast::Node) -> bool {
    if !ast::is_string_literal_like(store, node) {
        return false;
    }
    let Some(parent) = store.parent(node) else {
        return false;
    };
    if ast::is_require_call(store, parent, false) || ast::is_import_call(store, parent) {
        return store
            .arguments(parent)
            .and_then(|arguments| arguments.first())
            .is_some_and(|argument| argument == node);
    }
    matches!(
        store.kind(parent),
        ast::Kind::ExternalModuleReference
            | ast::Kind::ImportDeclaration
            | ast::Kind::JSImportDeclaration
    )
}

pub(crate) fn get_local_symbol_for_export_specifier(
    store: &ast::AstStore,
    reference_location: ast::Node,
    reference_symbol: ast::SymbolIdentity,
    export_specifier: ast::Node,
    ch: &mut checker::Checker<'_, '_>,
) -> ast::SymbolIdentity {
    if is_export_specifier_alias(store, reference_location, export_specifier) {
        if let Some(symbol) = ch.get_export_specifier_local_target_symbol_public(reference_location)
        {
            return symbol;
        }
    }
    reference_symbol
}

pub(crate) fn is_export_specifier_alias(
    store: &ast::AstStore,
    reference_location: ast::Node,
    export_specifier: ast::Node,
) -> bool {
    let property_name = store.property_name(export_specifier);
    let name = store.name(export_specifier);
    debug::assert(
        property_name.is_some_and(|property_name| property_name == reference_location)
            || name.is_some_and(|name| name == reference_location),
        Some("referenceLocation is not export specifier name or property name".to_string()),
    );
    if let Some(property_name) = property_name {
        property_name == reference_location
    } else {
        store
            .parent(reference_location)
            .and_then(|parent| store.parent(parent))
            .and_then(|export_declaration| store.module_specifier(export_declaration))
            .as_ref()
            .is_none()
    }
}

pub(crate) fn is_in_comment(
    file: &ast::SourceFile,
    position: i32,
    token_at_position: &ast::Node,
) -> Option<ast::CommentRange> {
    crate::format::is_in_comment(file, position, Some(token_at_position))
}

pub(crate) fn position_belongs_to_node(
    candidate: ast::Node,
    position: i32,
    file: &ast::SourceFile,
) -> bool {
    lsutil::position_belongs_to_node(candidate, position, file)
}

#[derive(Clone, Copy)]
pub(crate) struct PossibleTypeArgumentInfo {
    pub called: ast::Node,
    pub n_type_arguments: usize,
}

pub(crate) fn get_possible_type_arguments_info(
    token_in: &ast::Node,
    source_file: &ast::SourceFile,
) -> Option<PossibleTypeArgumentInfo> {
    if !source_file.text().contains('<') {
        return None;
    }
    let store = source_file.store();
    let mut token = Some(*token_in);
    let mut remaining_less_than_tokens = 0;
    let mut n_type_arguments = 0;
    while let Some(current) = token {
        match store.kind(current) {
            ast::Kind::LessThanToken => {
                token = astnav::find_preceding_token(source_file, store.loc(current).pos());
                if token.is_some_and(|t| store.kind(t) == ast::Kind::QuestionDotToken) {
                    token = token.and_then(|t| {
                        astnav::find_preceding_token(source_file, store.loc(t).pos())
                    });
                }
                let token_after = token?;
                if !ast::is_identifier(store, token_after) {
                    return None;
                }
                if remaining_less_than_tokens == 0 {
                    if ast::is_declaration_name(source_file.store(), token_after) {
                        return None;
                    }
                    return Some(PossibleTypeArgumentInfo {
                        called: token_after,
                        n_type_arguments,
                    });
                }
                remaining_less_than_tokens -= 1;
            }
            ast::Kind::GreaterThanGreaterThanGreaterThanToken => remaining_less_than_tokens += 3,
            ast::Kind::GreaterThanGreaterThanToken => remaining_less_than_tokens += 2,
            ast::Kind::GreaterThanToken => remaining_less_than_tokens += 1,
            ast::Kind::CloseBraceToken => {
                token =
                    find_preceding_matching_token(&current, ast::Kind::OpenBraceToken, source_file);
                if token.is_none() {
                    return None;
                }
            }
            ast::Kind::CloseParenToken => {
                token =
                    find_preceding_matching_token(&current, ast::Kind::OpenParenToken, source_file);
                if token.is_none() {
                    return None;
                }
            }
            ast::Kind::CloseBracketToken => {
                token = find_preceding_matching_token(
                    &current,
                    ast::Kind::OpenBracketToken,
                    source_file,
                );
                if token.is_none() {
                    return None;
                }
            }
            ast::Kind::CommaToken => n_type_arguments += 1,
            ast::Kind::EqualsGreaterThanToken
            | ast::Kind::Identifier
            | ast::Kind::StringLiteral
            | ast::Kind::NumericLiteral
            | ast::Kind::BigIntLiteral
            | ast::Kind::TrueKeyword
            | ast::Kind::FalseKeyword
            | ast::Kind::TypeOfKeyword
            | ast::Kind::ExtendsKeyword
            | ast::Kind::KeyOfKeyword
            | ast::Kind::DotToken
            | ast::Kind::BarToken
            | ast::Kind::QuestionToken
            | ast::Kind::ColonToken => {}
            _ => {
                if !ast::is_type_node(store, current) {
                    return None;
                }
            }
        }
        token = astnav::find_preceding_token(source_file, store.loc(current).pos());
    }
    None
}

pub(crate) fn is_name_of_module_declaration(store: &ast::AstStore, node: ast::Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    store.kind(parent) == ast::Kind::ModuleDeclaration
        && store
            .name(parent)
            .as_ref()
            .is_some_and(|name| *name == node)
}

pub(crate) fn is_expression_of_external_module_import_equals_declaration(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    let Some(parent_parent) = store.parent(node).and_then(|parent| store.parent(parent)) else {
        return false;
    };
    ast::is_external_module_import_equals_declaration(store, &parent_parent)
        && ast::get_external_module_import_equals_declaration_expression(store, &parent_parent)
            .is_some_and(|expression| expression == node)
}

pub(crate) fn is_namespace_reference(store: &ast::AstStore, node: ast::Node) -> bool {
    is_qualified_name_namespace_reference(store, node)
        || is_property_access_namespace_reference(store, node)
}

pub(crate) fn is_qualified_name_namespace_reference(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    let mut root = node;
    let mut is_last_clause = true;
    if store
        .parent(root)
        .is_some_and(|parent| store.kind(parent) == ast::Kind::QualifiedName)
    {
        while store
            .parent(root)
            .is_some_and(|parent| store.kind(parent) == ast::Kind::QualifiedName)
        {
            root = store.parent(root).unwrap();
        }
        is_last_clause = store.right(root).is_some_and(|right| right == node);
    }
    store
        .parent(root)
        .is_some_and(|parent| store.kind(parent) == ast::Kind::TypeReference)
        && !is_last_clause
}

pub(crate) fn is_property_access_namespace_reference(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    let mut root = node;
    let mut is_last_clause = true;
    if store
        .parent(root)
        .is_some_and(|parent| store.kind(parent) == ast::Kind::PropertyAccessExpression)
    {
        while store
            .parent(root)
            .is_some_and(|parent| store.kind(parent) == ast::Kind::PropertyAccessExpression)
        {
            root = store.parent(root).unwrap();
        }
        is_last_clause = store.name(root).as_ref().is_some_and(|name| *name == node);
    }
    let Some(root_parent) = store.parent(root) else {
        return false;
    };
    if !is_last_clause
        && store.kind(root_parent) == ast::Kind::ExpressionWithTypeArguments
        && store
            .parent(root_parent)
            .is_some_and(|parent| store.kind(parent) == ast::Kind::HeritageClause)
    {
        let heritage = store.parent(root_parent).unwrap();
        let Some(decl) = store.parent(heritage) else {
            return false;
        };
        return (store.kind(decl) == ast::Kind::ClassDeclaration
            && store.token(heritage) == Some(ast::Kind::ImplementsKeyword))
            || (store.kind(decl) == ast::Kind::InterfaceDeclaration
                && store.token(heritage) == Some(ast::Kind::ExtendsKeyword));
    }
    false
}

pub(crate) fn is_this(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node) {
        ast::Kind::ThisKeyword => true,
        ast::Kind::Identifier => {
            store.text(node) == "this"
                && store
                    .parent(node)
                    .is_some_and(|parent| store.kind(parent) == ast::Kind::Parameter)
        }
        _ => false,
    }
}

pub(crate) fn is_type_reference(store: &ast::AstStore, node: ast::Node) -> bool {
    let node = if ast::is_right_side_of_qualified_name_or_property_access(store, &node) {
        store.parent(node).unwrap_or(node)
    } else {
        node
    };
    match store.kind(node) {
        ast::Kind::ThisKeyword => return !ast::is_expression_node(store, &node),
        ast::Kind::ThisType => return true,
        _ => {}
    }
    let Some(parent) = store.parent(node) else {
        return false;
    };
    match store.kind(parent) {
        ast::Kind::TypeReference => true,
        ast::Kind::ImportType => !store.is_type_of(parent).unwrap_or(false),
        ast::Kind::ExpressionWithTypeArguments => ast::is_part_of_type_node(store, &parent),
        _ => false,
    }
}

pub(crate) fn is_in_right_side_of_internal_import_equals_declaration(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    if store.parent(node).is_none() {
        return false;
    }
    let mut node = node;
    while store
        .parent(node)
        .is_some_and(|parent| store.kind(parent) == ast::Kind::QualifiedName)
    {
        node = store.parent(node).unwrap();
    }
    let Some(parent) = store.parent(node) else {
        return false;
    };
    ast::is_internal_module_import_equals_declaration(store, &parent)
        && store
            .module_reference(parent)
            .is_some_and(|module_reference| module_reference == node)
}

impl LanguageService<'_> {
    pub(crate) fn create_lsp_range_from_node(
        &self,
        node: ast::Node,
        file: &ast::SourceFile,
    ) -> lsproto::Range {
        self.create_lsp_range_from_bounds(
            scanner::get_token_pos_of_node(&node, file, false) as i32,
            file.store().loc(node).end(),
            file,
        )
    }

    pub(crate) fn create_lsp_range_from_bounds(
        &self,
        start: i32,
        end: i32,
        file: &ast::SourceFile,
    ) -> lsproto::Range {
        self.converters
            .to_lsp_range(file, core::new_text_range(start, end))
    }

    pub(crate) fn create_lsp_range_from_range(
        &self,
        text_range: core::TextRange,
        script: &dyn lsconv::Script,
    ) -> lsproto::Range {
        self.converters.to_lsp_range(script, text_range)
    }

    pub(crate) fn create_lsp_position(
        &self,
        position: i32,
        file: &ast::SourceFile,
    ) -> lsproto::Position {
        self.converters
            .position_to_line_and_character(file, position)
    }
}

pub(crate) fn create_range_from_node(node: ast::Node, file: &ast::SourceFile) -> core::TextRange {
    core::new_text_range(
        scanner::get_token_pos_of_node(&node, file, false) as i32,
        file.store().loc(node).end(),
    )
}

pub(crate) fn quote(
    file: &ast::SourceFile,
    preferences: lsutil::UserPreferences,
    text: &str,
) -> String {
    let quote_preference = lsutil::get_quote_preference(file, &preferences);
    let mut quoted =
        core::stringify_json(&text.to_string(), "", "").unwrap_or_else(|_| "\"\"".to_string());
    if quote_preference == lsutil::QuotePreference::Single {
        let stripped = stringutil::strip_quotes(&quoted);
        quoted = stripped.replace('\'', "\\'").replace('"', "\\\"");
    }
    quoted
}

pub const TYPE_KEYWORDS: &[ast::Kind] = &[
    ast::Kind::AnyKeyword,
    ast::Kind::AssertsKeyword,
    ast::Kind::BigIntKeyword,
    ast::Kind::BooleanKeyword,
    ast::Kind::FalseKeyword,
    ast::Kind::InferKeyword,
    ast::Kind::KeyOfKeyword,
    ast::Kind::NeverKeyword,
    ast::Kind::NullKeyword,
    ast::Kind::NumberKeyword,
    ast::Kind::ObjectKeyword,
    ast::Kind::ReadonlyKeyword,
    ast::Kind::StringKeyword,
    ast::Kind::SymbolKeyword,
    ast::Kind::TypeOfKeyword,
    ast::Kind::TrueKeyword,
    ast::Kind::VoidKeyword,
    ast::Kind::UndefinedKeyword,
    ast::Kind::UniqueKeyword,
    ast::Kind::UnknownKeyword,
];

pub(crate) fn is_type_keyword(kind: ast::Kind) -> bool {
    TYPE_KEYWORDS.contains(&kind)
}

pub(crate) fn is_separator(
    store: &ast::AstStore,
    node: ast::Node,
    candidate: Option<ast::Node>,
) -> bool {
    candidate.is_some_and(|candidate| {
        store.parent(node).is_some()
            && (store.kind(candidate) == ast::Kind::CommaToken
                || (store.kind(candidate) == ast::Kind::SemicolonToken
                    && store.parent(node).is_some_and(|parent| {
                        store.kind(parent) == ast::Kind::ObjectLiteralExpression
                    })))
    })
}

pub(crate) fn is_literal_name_of_property_declaration_or_index_access(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    match store.kind(parent) {
        ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature
        | ast::Kind::PropertyAssignment
        | ast::Kind::EnumMember
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::ModuleDeclaration => {
            ast::get_name_of_declaration(store, Some(parent)).is_some_and(|name| name == node)
        }
        ast::Kind::ElementAccessExpression => store
            .argument_expression(parent)
            .is_some_and(|argument_expression| argument_expression == node),
        ast::Kind::ComputedPropertyName => true,
        ast::Kind::LiteralType => store
            .parent(parent)
            .is_some_and(|grandparent| store.kind(grandparent) == ast::Kind::IndexedAccessType),
        _ => false,
    }
}

pub(crate) fn is_object_binding_element_without_property_name(
    store: &ast::AstStore,
    binding_element: &ast::Node,
) -> bool {
    store.kind(*binding_element) == ast::Kind::BindingElement
        && store
            .parent(node_handle(binding_element))
            .is_some_and(|parent| store.kind(parent) == ast::Kind::ObjectBindingPattern)
        && store
            .name(node_handle(binding_element))
            .as_ref()
            .is_some_and(|name| store.kind(*name) == ast::Kind::Identifier)
        && store.property_name(node_handle(binding_element)).is_none()
}

pub(crate) fn is_right_side_of_property_access(store: &ast::AstStore, node: ast::Node) -> bool {
    store.parent(node).is_some_and(|parent| {
        store.kind(parent) == ast::Kind::PropertyAccessExpression
            && store
                .name(parent)
                .as_ref()
                .is_some_and(|name| *name == node)
    })
}

pub(crate) fn is_implementation(store: &ast::AstStore, node: ast::Node) -> bool {
    if store.flags(node).intersects(ast::NodeFlags::Ambient) {
        return !(store.kind(node) == ast::Kind::InterfaceDeclaration
            || store.kind(node) == ast::Kind::TypeAliasDeclaration);
    }
    if ast::is_variable_like(store, node) {
        return ast::has_initializer(store, &node);
    }
    if ast::is_function_like_declaration(store, Some(node)) {
        return store.body(node).is_some();
    }
    ast::is_class_like(store, node) || ast::is_module_or_enum_declaration(store, node)
}

pub(crate) fn is_implementation_expression(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node) {
        ast::Kind::ParenthesizedExpression => store
            .expression(node)
            .as_ref()
            .is_some_and(|expression| is_implementation_expression(store, *expression)),
        ast::Kind::ArrowFunction
        | ast::Kind::FunctionExpression
        | ast::Kind::ObjectLiteralExpression
        | ast::Kind::ClassExpression
        | ast::Kind::ArrayLiteralExpression => true,
        _ => false,
    }
}

pub(crate) fn is_readonly_type_operator(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::ReadonlyKeyword
        && store.parent(node).is_some_and(|parent| {
            store.kind(parent) == ast::Kind::TypeOperator
                && store.operator(parent) == Some(ast::Kind::ReadonlyKeyword)
        })
}

pub(crate) fn is_jump_statement_target(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::Identifier
        && store.parent(node).as_ref().is_some_and(|parent| {
            ast::is_break_or_continue_statement(store, *parent)
                && store
                    .label(*parent)
                    .as_ref()
                    .is_some_and(|label| *label == node)
        })
}

pub(crate) fn is_label_of_labeled_statement(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::Identifier
        && store.parent(node).is_some_and(|parent| {
            store.kind(parent) == ast::Kind::LabeledStatement
                && store
                    .label(parent)
                    .as_ref()
                    .is_some_and(|label| *label == node)
        })
}

pub(crate) fn find_reference_in_position<'a>(
    refs: &'a [&'a ast::FileReference],
    pos: i32,
) -> Option<&'a ast::FileReference> {
    refs.iter()
        .copied()
        .find(|r| r.text_range.contains_inclusive(pos))
}

pub(crate) fn get_containing_node_if_in_heritage_clause(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    if matches!(
        store.kind(node),
        ast::Kind::Identifier | ast::Kind::PropertyAccessExpression
    ) {
        return store
            .parent(node)
            .and_then(|parent| get_containing_node_if_in_heritage_clause(store, parent));
    }
    let Some(parent_parent) = store.parent(node).and_then(|parent| store.parent(parent)) else {
        return None;
    };
    if store.kind(node) == ast::Kind::ExpressionWithTypeArguments
        && (ast::is_class_like(store, parent_parent)
            || store.kind(parent_parent) == ast::Kind::InterfaceDeclaration)
    {
        return Some(parent_parent);
    }
    None
}

pub(crate) fn get_container_node(store: &ast::AstStore, node: ast::Node) -> Option<ast::Node> {
    let mut parent = store.parent(node);
    while let Some(p) = parent {
        match store.kind(p) {
            ast::Kind::SourceFile
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::ClassDeclaration
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::EnumDeclaration
            | ast::Kind::ModuleDeclaration => return Some(p),
            _ => parent = store.parent(p),
        }
    }
    None
}

pub(crate) fn get_adjusted_location(
    store: &ast::AstStore,
    node: ast::Node,
    for_rename: bool,
    source_file: Option<&ast::SourceFile>,
) -> ast::Node {
    let parent = store.parent(node).unwrap();
    let is_modifier = |node: ast::Node| -> bool {
        if ast::is_modifier(store, node)
            && (for_rename || store.kind(node) != ast::Kind::DefaultKeyword)
        {
            return ast::can_have_modifiers(store, parent)
                && store
                    .modifier_nodes(parent)
                    .iter()
                    .any(|modifier| *modifier == node);
        }
        match store.kind(node) {
            ast::Kind::ClassKeyword => {
                ast::is_class_declaration(store, parent) || ast::is_class_expression(store, parent)
            }
            ast::Kind::FunctionKeyword => {
                ast::is_function_declaration(store, parent)
                    || ast::is_function_expression(store, parent)
            }
            ast::Kind::InterfaceKeyword => ast::is_interface_declaration(store, parent),
            ast::Kind::EnumKeyword => ast::is_enum_declaration(store, parent),
            ast::Kind::TypeKeyword => ast::is_type_alias_declaration(store, parent),
            ast::Kind::NamespaceKeyword | ast::Kind::ModuleKeyword => {
                ast::is_module_declaration(store, parent)
            }
            ast::Kind::ImportKeyword => ast::is_import_equals_declaration(store, parent),
            ast::Kind::GetKeyword => ast::is_get_accessor_declaration(store, parent),
            ast::Kind::SetKeyword => ast::is_set_accessor_declaration(store, parent),
            _ => false,
        }
    };
    if is_modifier(node) {
        if let Some(file) = source_file
            && let Some(location) =
                get_adjusted_location_for_declaration(store, parent, for_rename, file)
        {
            return location;
        }
    }

    if matches!(
        store.kind(node),
        ast::Kind::VarKeyword | ast::Kind::ConstKeyword | ast::Kind::LetKeyword
    ) && store.kind(parent) == ast::Kind::VariableDeclarationList
        && store
            .declarations(parent)
            .is_some_and(|declarations| declarations.len() == 1)
    {
        let declaration = store.declarations(parent).unwrap().first().unwrap();
        if store
            .name(declaration)
            .is_some_and(|name| ast::is_identifier(store, name))
        {
            return store.name(declaration).unwrap();
        }
    }

    if store.kind(node) == ast::Kind::TypeKeyword {
        if store.kind(parent) == ast::Kind::ImportClause
            && store.is_type_only(parent).unwrap_or(false)
        {
            if let Some(location) = get_adjusted_location_for_import_declaration(
                store,
                store.parent(parent).unwrap(),
                for_rename,
            ) {
                return location;
            }
        }
        if store.kind(parent) == ast::Kind::ExportDeclaration
            && store.is_type_only(parent).unwrap_or(false)
        {
            if let Some(location) =
                get_adjusted_location_for_export_declaration(store, parent, for_rename)
            {
                return location;
            }
        }
    }

    if store.kind(node) == ast::Kind::AsKeyword {
        if (store.kind(parent) == ast::Kind::ImportSpecifier
            && store.property_name(parent).is_some())
            || (store.kind(parent) == ast::Kind::ExportSpecifier
                && store.property_name(parent).is_some())
            || store.kind(parent) == ast::Kind::NamespaceImport
            || store.kind(parent) == ast::Kind::NamespaceExport
        {
            return store.name(parent).unwrap();
        }
        if store.kind(parent) == ast::Kind::ExportDeclaration {
            if let Some(export_clause) = store.export_clause(parent) {
                if store.kind(export_clause) == ast::Kind::NamespaceExport {
                    return store.name(export_clause).unwrap();
                }
            }
        }
    }

    if store.kind(node) == ast::Kind::ImportKeyword
        && store.kind(parent) == ast::Kind::ImportDeclaration
    {
        if let Some(location) =
            get_adjusted_location_for_import_declaration(store, parent, for_rename)
        {
            return location;
        }
    }

    if store.kind(node) == ast::Kind::ExportKeyword {
        if store.kind(parent) == ast::Kind::ExportDeclaration {
            if let Some(location) =
                get_adjusted_location_for_export_declaration(store, parent, for_rename)
            {
                return location;
            }
        }
        if store.kind(parent) == ast::Kind::ExportAssignment {
            return ast::skip_outer_expressions(
                store,
                store.expression(parent).unwrap(),
                ast::OuterExpressionKinds::ALL,
            );
        }
    }
    if store.kind(node) == ast::Kind::RequireKeyword
        && store.kind(parent) == ast::Kind::ExternalModuleReference
    {
        return store.expression(parent).unwrap();
    }
    if store.kind(node) == ast::Kind::FromKeyword
        && (store.kind(parent) == ast::Kind::ImportDeclaration
            || store.kind(parent) == ast::Kind::ExportDeclaration)
        && store.module_specifier(parent).is_some()
    {
        return store.module_specifier(parent).unwrap();
    }
    if (store.kind(node) == ast::Kind::ExtendsKeyword
        || store.kind(node) == ast::Kind::ImplementsKeyword)
        && store.kind(parent) == ast::Kind::HeritageClause
        && store.token(parent) == Some(store.kind(node))
        && store.types(parent).is_some_and(|types| types.len() == 1)
    {
        let type_node = store.types(parent).unwrap().first().unwrap();
        return store.expression(type_node).unwrap();
    }
    if store.kind(node) == ast::Kind::ExtendsKeyword {
        if store.kind(parent) == ast::Kind::TypeParameter {
            if let Some(constraint) = store.constraint(parent) {
                if store.kind(constraint) == ast::Kind::TypeReference {
                    return store.type_name(constraint).unwrap();
                }
            }
        }
        if store.kind(parent) == ast::Kind::ConditionalType {
            if let Some(extends_type) = store.extends_type(parent) {
                if store.kind(extends_type) == ast::Kind::TypeReference {
                    return store.type_name(extends_type).unwrap();
                }
            }
        }
    }
    if store.kind(node) == ast::Kind::InferKeyword && store.kind(parent) == ast::Kind::InferType {
        let type_parameter = store.type_parameter(parent).unwrap();
        return store.name(type_parameter).unwrap();
    }
    if store.kind(node) == ast::Kind::InKeyword
        && store.kind(parent) == ast::Kind::TypeParameter
        && store
            .parent(parent)
            .is_some_and(|parent| store.kind(parent) == ast::Kind::MappedType)
    {
        return store.name(parent).unwrap();
    }
    if store.kind(node) == ast::Kind::KeyOfKeyword
        && store.kind(parent) == ast::Kind::TypeOperator
        && store.operator(parent) == Some(ast::Kind::KeyOfKeyword)
    {
        if let Some(parent_type) = store.r#type(parent) {
            if store.kind(parent_type) == ast::Kind::TypeReference {
                return store.type_name(parent_type).unwrap();
            }
        }
    }
    if store.kind(node) == ast::Kind::ReadonlyKeyword
        && store.kind(parent) == ast::Kind::TypeOperator
        && store.operator(parent) == Some(ast::Kind::ReadonlyKeyword)
    {
        if let Some(parent_type) = store.r#type(parent) {
            if store.kind(parent_type) == ast::Kind::ArrayType
                && store.element_type(parent_type).is_some_and(|element_type| {
                    store.kind(element_type) == ast::Kind::TypeReference
                })
            {
                let element_type = store.element_type(parent_type).unwrap();
                return store.type_name(element_type).unwrap();
            }
        }
    }

    if !for_rename {
        if ((store.kind(node) == ast::Kind::NewKeyword
            && store.kind(parent) == ast::Kind::NewExpression)
            || (store.kind(node) == ast::Kind::VoidKeyword
                && store.kind(parent) == ast::Kind::VoidExpression)
            || (store.kind(node) == ast::Kind::TypeOfKeyword
                && store.kind(parent) == ast::Kind::TypeOfExpression)
            || (store.kind(node) == ast::Kind::AwaitKeyword
                && store.kind(parent) == ast::Kind::AwaitExpression)
            || (store.kind(node) == ast::Kind::YieldKeyword
                && store.kind(parent) == ast::Kind::YieldExpression)
            || (store.kind(node) == ast::Kind::DeleteKeyword
                && store.kind(parent) == ast::Kind::DeleteExpression))
            && store.expression(parent).is_some()
        {
            return ast::skip_outer_expressions(
                store,
                store.expression(parent).unwrap(),
                ast::OuterExpressionKinds::ALL,
            );
        }
        if (store.kind(node) == ast::Kind::InKeyword
            || store.kind(node) == ast::Kind::InstanceOfKeyword)
            && store.kind(parent) == ast::Kind::BinaryExpression
            && store
                .operator_token(parent)
                .is_some_and(|operator_token| operator_token == node)
        {
            return ast::skip_outer_expressions(
                store,
                store.right(parent).unwrap(),
                ast::OuterExpressionKinds::ALL,
            );
        }
        if store.kind(node) == ast::Kind::AsKeyword && store.kind(parent) == ast::Kind::AsExpression
        {
            if let Some(as_expr_type) = store.r#type(parent) {
                if store.kind(as_expr_type) == ast::Kind::TypeReference {
                    return store.type_name(as_expr_type).unwrap();
                }
            }
        }
        if ((store.kind(node) == ast::Kind::InKeyword
            && store.kind(parent) == ast::Kind::ForInStatement)
            || (store.kind(node) == ast::Kind::OfKeyword
                && store.kind(parent) == ast::Kind::ForOfStatement))
            && store.expression(parent).is_some()
        {
            return ast::skip_outer_expressions(
                store,
                store.expression(parent).unwrap(),
                ast::OuterExpressionKinds::ALL,
            );
        }
    }
    node
}

pub(crate) fn get_adjusted_location_for_declaration(
    store: &ast::AstStore,
    node: ast::Node,
    for_rename: bool,
    source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    if let Some(name) = store.name(node) {
        return Some(name);
    }
    if for_rename {
        return None;
    }
    match store.kind(node) {
        ast::Kind::ClassDeclaration | ast::Kind::FunctionDeclaration => store
            .modifier_nodes(node)
            .into_iter()
            .find(|modifier| store.kind(*modifier) == ast::Kind::DefaultKeyword),
        ast::Kind::ClassExpression => {
            astnav::find_child_of_kind(node, ast::Kind::ClassKeyword, source_file)
        }
        ast::Kind::FunctionExpression => {
            astnav::find_child_of_kind(node, ast::Kind::FunctionKeyword, source_file)
        }
        ast::Kind::Constructor => Some(node),
        _ => None,
    }
}

pub(crate) fn get_adjusted_location_for_import_declaration(
    store: &ast::AstStore,
    node: ast::Node,
    for_rename: bool,
) -> Option<ast::Node> {
    if let Some(import_clause) = store.import_clause(node) {
        if let Some(name) = store.name(import_clause) {
            if store.named_bindings(import_clause).is_some() {
                return None;
            }
            return Some(name);
        }
        if let Some(named_bindings) = store.named_bindings(import_clause) {
            match store.kind(named_bindings) {
                ast::Kind::NamedImports => {
                    let elements = store.elements(named_bindings)?;
                    if elements.len() != 1 {
                        return None;
                    }
                    return store.name(elements.first().unwrap());
                }
                ast::Kind::NamespaceImport => {
                    return store.name(named_bindings);
                }
                _ => {}
            }
        }
    }
    if !for_rename {
        return store.module_specifier(node);
    }
    None
}

pub(crate) fn get_adjusted_location_for_export_declaration(
    store: &ast::AstStore,
    node: ast::Node,
    for_rename: bool,
) -> Option<ast::Node> {
    if let Some(export_clause) = store.export_clause(node) {
        match store.kind(export_clause) {
            ast::Kind::NamedExports => {
                let elements = store.elements(export_clause)?;
                if elements.len() == 1 {
                    return store.name(elements.first().unwrap());
                }
            }
            ast::Kind::NamespaceExport => return store.name(export_clause),
            _ => {}
        }
    }
    if !for_rename {
        return store.module_specifier(node);
    }
    None
}

pub(crate) fn symbol_flags_have_meaning(
    flags: ast::SymbolFlags,
    meaning: ast::SemanticMeaning,
) -> bool {
    if meaning == ast::SEMANTIC_MEANING_ALL {
        return true;
    }
    if meaning.0 & ast::SemanticMeaning::VALUE.0 != 0 {
        return flags & ast::SYMBOL_FLAGS_VALUE != 0;
    }
    if meaning.0 & ast::SemanticMeaning::TYPE.0 != 0 {
        return flags & ast::SYMBOL_FLAGS_TYPE != 0;
    }
    if meaning.0 & ast::SemanticMeaning::NAMESPACE.0 != 0 {
        return flags & ast::SYMBOL_FLAGS_NAMESPACE != 0;
    }
    false
}

pub(crate) fn get_meaning_from_location(
    store: &ast::AstStore,
    node: ast::Node,
) -> ast::SemanticMeaning {
    let reparsed = ast::get_reparsed_node_for_node(store, &node);
    let node = get_adjusted_location(store, reparsed, false, None);
    let parent = store.parent(node);
    if ast::is_source_file(store, node) {
        return ast::SemanticMeaning::VALUE;
    }
    if parent.as_ref().is_some_and(|parent| {
        ast::node_kind_is(
            store,
            parent,
            &[
                ast::Kind::ExportAssignment,
                ast::Kind::ExportSpecifier,
                ast::Kind::ExternalModuleReference,
                ast::Kind::ImportSpecifier,
                ast::Kind::ImportClause,
            ],
        )
    }) || (parent.is_some_and(|parent| {
        store.kind(parent) == ast::Kind::ImportEqualsDeclaration
            && store
                .name(parent)
                .as_ref()
                .is_some_and(|name| *name == node)
    })) {
        return ast::SEMANTIC_MEANING_ALL;
    }
    if is_in_right_side_of_internal_import_equals_declaration(store, node) {
        let name = if store.kind(node) == ast::Kind::QualifiedName {
            Some(node)
        } else if store
            .parent(node)
            .is_some_and(|parent| store.kind(parent) == ast::Kind::QualifiedName)
            && store
                .parent(node)
                .and_then(|parent| store.right(parent))
                .is_some_and(|right| right == node)
        {
            store.parent(node)
        } else {
            None
        };
        if name.is_some_and(|name| {
            store
                .parent(name)
                .is_some_and(|parent| store.kind(parent) == ast::Kind::ImportEqualsDeclaration)
        }) {
            return ast::SEMANTIC_MEANING_ALL;
        }
        return ast::SemanticMeaning::NAMESPACE;
    }
    if ast::is_declaration_name(store, &node) {
        return get_meaning_from_declaration(store, parent.unwrap());
    }
    if is_type_reference(store, node) {
        return ast::SemanticMeaning::TYPE;
    }
    if is_namespace_reference(store, node) {
        return ast::SemanticMeaning::NAMESPACE;
    }
    if parent.is_some_and(|parent| ast::is_type_parameter_declaration(store, parent)) {
        return ast::SemanticMeaning::TYPE;
    }
    if parent.is_some_and(|parent| ast::is_literal_type_node(store, parent)) {
        return ast::SemanticMeaning(ast::SemanticMeaning::TYPE.0 | ast::SemanticMeaning::VALUE.0);
    }
    ast::SemanticMeaning::VALUE
}

pub(crate) fn get_meaning_from_declaration(
    store: &ast::AstStore,
    node: ast::Node,
) -> ast::SemanticMeaning {
    match store.kind(node) {
        ast::Kind::VariableDeclaration
        | ast::Kind::Parameter
        | ast::Kind::BindingElement
        | ast::Kind::PropertyDeclaration
        | ast::Kind::PropertySignature
        | ast::Kind::PropertyAssignment
        | ast::Kind::ShorthandPropertyAssignment
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::ArrowFunction
        | ast::Kind::CatchClause
        | ast::Kind::JsxAttribute => ast::SemanticMeaning::VALUE,
        ast::Kind::TypeParameter
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::JSTypeAliasDeclaration
        | ast::Kind::TypeLiteral => ast::SemanticMeaning::TYPE,
        ast::Kind::EnumMember | ast::Kind::ClassDeclaration => {
            ast::SemanticMeaning(ast::SemanticMeaning::VALUE.0 | ast::SemanticMeaning::TYPE.0)
        }
        ast::Kind::ModuleDeclaration => {
            if ast::is_ambient_module(store, node)
                || ast::get_module_instance_state(store, node)
                    == ast::ModuleInstanceState::Instantiated
            {
                ast::SemanticMeaning(
                    ast::SemanticMeaning::NAMESPACE.0 | ast::SemanticMeaning::VALUE.0,
                )
            } else {
                ast::SemanticMeaning::NAMESPACE
            }
        }
        ast::Kind::EnumDeclaration
        | ast::Kind::NamedImports
        | ast::Kind::ImportSpecifier
        | ast::Kind::ImportEqualsDeclaration
        | ast::Kind::ImportDeclaration
        | ast::Kind::JSImportDeclaration
        | ast::Kind::ExportAssignment
        | ast::Kind::ExportDeclaration => ast::SEMANTIC_MEANING_ALL,
        ast::Kind::SourceFile => {
            ast::SemanticMeaning(ast::SemanticMeaning::NAMESPACE.0 | ast::SemanticMeaning::VALUE.0)
        }
        _ => ast::SEMANTIC_MEANING_ALL,
    }
}

pub(crate) fn get_all_super_type_nodes(store: &ast::AstStore, node: ast::Node) -> Vec<ast::Node> {
    if ast::is_interface_declaration(store, node) {
        return ast::get_heritage_elements(store, node, ast::Kind::ExtendsKeyword);
    }
    if ast::is_class_like(store, node) {
        let mut res = Vec::new();
        if let Some(ext) = ast::get_class_extends_heritage_element(store, &node) {
            res.push(ext);
        }
        res.extend(ast::get_implements_type_nodes(store, &node));
        return res;
    }
    Vec::new()
}

pub(crate) fn get_target_label(
    store: &ast::AstStore,
    mut reference_node: ast::Node,
    label_name: &str,
) -> Option<ast::Node> {
    loop {
        if store.kind(reference_node) == ast::Kind::LabeledStatement
            && store
                .label(reference_node)
                .is_some_and(|label| store.text(label) == label_name)
        {
            return store.label(reference_node);
        }
        reference_node = store.parent(reference_node)?;
    }
}

pub(crate) fn skip_constraint<'a>(
    ty: checker::TypeHandle,
    type_checker: &mut checker::Checker<'a, '_>,
) -> checker::TypeHandle {
    if type_checker.is_type_parameter_public(ty) {
        if let Some(c) = type_checker.get_base_constraint_of_type_public(ty) {
            return c;
        }
    }
    ty
}

#[derive(Default)]
pub(crate) struct CaseClauseTrackerState {
    pub existing_strings: HashSet<String>,
    pub existing_numbers: HashSet<jsnum::Number>,
    pub existing_bigints: HashSet<jsnum::PseudoBigInt>,
}

pub(crate) enum TrackerAddValue {
    String(String),
    Number(jsnum::Number),
}

pub(crate) enum TrackerHasValue {
    String(String),
    Number(jsnum::Number),
    BigInt(jsnum::PseudoBigInt),
}

pub trait CaseClauseTracker {
    fn add_value(&mut self, value: TrackerAddValue);
    fn has_value(&self, value: &TrackerHasValue) -> bool;
}

impl CaseClauseTracker for CaseClauseTrackerState {
    fn add_value(&mut self, value: TrackerAddValue) {
        match value {
            TrackerAddValue::String(v) => {
                self.existing_strings.insert(v);
            }
            TrackerAddValue::Number(v) => {
                self.existing_numbers.insert(v);
            }
        }
    }

    fn has_value(&self, value: &TrackerHasValue) -> bool {
        match value {
            TrackerHasValue::String(v) => self.existing_strings.contains(v),
            TrackerHasValue::Number(v) => self.existing_numbers.contains(v),
            TrackerHasValue::BigInt(v) => self.existing_bigints.contains(v),
        }
    }
}

pub(crate) fn new_case_clause_tracker<'a>(
    store: &ast::AstStore,
    type_checker: &mut checker::Checker<'a, '_>,
    clauses: &[ast::Node],
) -> CaseClauseTrackerState {
    let mut tracker = CaseClauseTrackerState::default();
    for clause in clauses {
        if !ast::is_default_clause(store, *clause) {
            let clause_expression = store.expression(*clause).unwrap();
            let expression = ast::skip_parentheses(store, clause_expression);
            if ast::is_literal_expression(store, expression) {
                match store.kind(expression) {
                    ast::Kind::NoSubstitutionTemplateLiteral | ast::Kind::StringLiteral => {
                        tracker.existing_strings.insert(store.text(expression));
                    }
                    ast::Kind::NumericLiteral => {
                        tracker
                            .existing_numbers
                            .insert(jsnum::from_string(&store.text(expression)));
                    }
                    ast::Kind::BigIntLiteral => {
                        tracker
                            .existing_bigints
                            .insert(jsnum::parse_valid_big_int(&store.text(expression)));
                    }
                    _ => {}
                }
            } else if let Some(symbol) =
                type_checker.get_symbol_at_location_public(clause_expression)
            {
                let value_declaration = type_checker.symbol_value_declaration_public(symbol);
                if let Some(value_declaration) = value_declaration {
                    if ast::is_enum_member(store, value_declaration) {
                        match type_checker.get_enum_member_value_public(value_declaration) {
                            evaluator::Value::String(v) => {
                                tracker.add_value(TrackerAddValue::String(v));
                            }
                            evaluator::Value::Number(v) => {
                                tracker.add_value(TrackerAddValue::Number(v));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    tracker
}

pub(crate) fn range_contains_range(r1: core::TextRange, r2: core::TextRange) -> bool {
    start_end_contains_range(r1.pos(), r1.end(), r2)
}

pub(crate) fn start_end_contains_range(start: i32, end: i32, text_range: core::TextRange) -> bool {
    start <= text_range.pos() && end >= text_range.end()
}

pub(crate) fn get_possible_generic_signatures<'a>(
    store: &ast::AstStore,
    called: &ast::Node,
    type_argument_count: usize,
    c: &mut checker::Checker<'a, '_>,
) -> Vec<checker::SignatureHandle> {
    let mut type_at_location = c.get_type_at_location(*called);
    if store
        .parent(node_handle(called))
        .as_ref()
        .is_some_and(|parent| ast::is_optional_chain(store, *parent))
    {
        type_at_location = remove_optionality(
            type_at_location,
            store
                .parent(node_handle(called))
                .as_ref()
                .is_some_and(|parent| ast::is_optional_chain_root(store, *parent)),
            true,
            c,
        );
    }
    let signatures = if store
        .parent(node_handle(called))
        .as_ref()
        .is_some_and(|parent| ast::is_new_expression(store, *parent))
    {
        c.get_signatures_of_type_public(type_at_location, checker::SIGNATURE_KIND_CONSTRUCT)
    } else {
        c.get_signatures_of_type_public(type_at_location, checker::SIGNATURE_KIND_CALL)
    };
    signatures
        .into_iter()
        .filter(|s| c.signature_type_parameters_public(*s).len() >= type_argument_count)
        .collect()
}

pub(crate) fn remove_optionality<'a>(
    ty: checker::TypeHandle,
    is_optional_expression: bool,
    is_optional_chain: bool,
    c: &mut checker::Checker<'a, '_>,
) -> checker::TypeHandle {
    if is_optional_expression {
        c.get_non_nullable_type_public(ty)
    } else if is_optional_chain {
        c.get_non_optional_type_public(ty)
    } else {
        ty
    }
}

pub(crate) fn is_no_substitution_template_literal(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::NoSubstitutionTemplateLiteral
}

pub(crate) fn is_tagged_template_expression(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::TaggedTemplateExpression
}

pub(crate) fn is_inside_template_literal(
    node: &ast::Node,
    position: i32,
    source_file: &ast::SourceFile,
) -> bool {
    let store = source_file.store();
    ast::is_template_literal_kind(store.kind(*node))
        && (((scanner::get_token_pos_of_node(node, source_file, false) as i32) < position
            && position < store.loc(*node).end())
            || (ast::is_unterminated_literal(store, *node) && position == store.loc(*node).end()))
}

pub(crate) fn is_template_head(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::TemplateHead
}

pub(crate) fn is_template_tail(store: &ast::AstStore, node: ast::Node) -> bool {
    store.kind(node) == ast::Kind::TemplateTail
}

pub(crate) fn find_preceding_matching_token(
    token: &ast::Node,
    matching_token_kind: ast::Kind,
    source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    let store = source_file.store();
    let mut token = *token;
    let close_token_text = scanner::token_to_string(store.kind(token));
    let matching_token_text = scanner::token_to_string(matching_token_kind);
    let best_guess_index = source_file.text().rfind(&matching_token_text)?;
    if source_file
        .text()
        .rfind(&close_token_text)
        .map(|index| index as i32)
        .unwrap_or(-1)
        < best_guess_index as i32
    {
        let node_at_guess = astnav::find_preceding_token(source_file, best_guess_index as i32 + 1);
        if node_at_guess.is_some_and(|n| store.kind(n) == matching_token_kind) {
            return node_at_guess;
        }
    }
    let token_kind = store.kind(token);
    let mut remaining_matching_tokens = 0;
    loop {
        let preceding = astnav::find_preceding_token(source_file, store.loc(token).pos())?;
        token = preceding;
        if store.kind(token) == matching_token_kind {
            if remaining_matching_tokens == 0 {
                return Some(token);
            }
            remaining_matching_tokens -= 1;
        } else if store.kind(token) == token_kind {
            remaining_matching_tokens += 1;
        }
    }
}

pub(crate) fn find_containing_list<'a>(
    node: &ast::Node,
    file: &'a ast::SourceFile,
) -> Option<ast::SourceNodeList<'a>> {
    format::get_containing_list(node, file)
}

pub(crate) fn get_leading_comment_ranges_of_node(
    node: &ast::Node,
    file: &ast::SourceFile,
) -> Vec<ast::CommentRange> {
    let store = file.store();
    if store.kind(*node) == ast::Kind::JsxText {
        return Vec::new();
    }
    scanner::get_leading_comment_ranges(file.text(), store.loc(*node).pos())
}

#[derive(Clone, Copy)]
pub(crate) struct SyntaxChild {
    pub node: Option<ast::Node>,
    pub kind: ast::Kind,
    pub loc: core::TextRange,
}

impl SyntaxChild {
    pub(crate) fn from_node(store: &ast::AstStore, node: ast::Node) -> Self {
        Self {
            node: Some(node),
            kind: store.kind(node),
            loc: store.loc(node),
        }
    }

    pub(crate) fn is_same_node_or_token(&self, store: &ast::AstStore, node: ast::Node) -> bool {
        self.node.is_some_and(|child| child == node)
            || (self.kind == store.kind(node) && self.loc == store.loc(node))
    }
}

pub(crate) fn get_children_with_tokens(
    node: &ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<SyntaxChild> {
    let store = source_file.store();
    let mut child_nodes = Vec::new();
    let _ = store.for_each_present_child(node_handle(node), |child| {
        child_nodes.push(child);
        std::ops::ControlFlow::Continue(())
    });
    if child_nodes.is_empty() {
        return Vec::new();
    }
    let mut children = Vec::new();
    let mut pos = store.loc(*node).pos();
    for child in child_nodes {
        let mut scanner = scanner::get_scanner_for_source_file(source_file, pos.max(0) as usize);
        let mut pos_mut = pos;
        let child_pos = store.loc(child).pos();
        while pos_mut < child_pos {
            let token = scanner.token();
            let token_full_start = scanner.token_full_start();
            let token_end = scanner.token_end();
            if token_end <= pos_mut {
                break;
            }
            children.push(SyntaxChild {
                node: None,
                kind: token,
                loc: core::new_text_range(token_full_start, token_end),
            });
            pos_mut = token_end;
            scanner.scan();
        }
        children.push(SyntaxChild::from_node(store, child));
        pos = store.loc(child).end();
    }
    let mut scanner = scanner::get_scanner_for_source_file(source_file, pos.max(0) as usize);
    let mut pos_mut = pos;
    let node_end = store.loc(*node).end();
    while pos_mut < node_end {
        let token = scanner.token();
        let token_full_start = scanner.token_full_start();
        let token_end = scanner.token_end();
        if token_end <= pos_mut {
            break;
        }
        children.push(SyntaxChild {
            node: None,
            kind: token,
            loc: core::new_text_range(token_full_start, token_end),
        });
        pos_mut = token_end;
        scanner.scan();
    }
    children
}

pub(crate) fn get_containing_object_literal_element(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    let element = get_containing_object_literal_element_worker(store, node)?;
    if store
        .parent(element)
        .as_ref()
        .is_some_and(|parent| ast::is_object_literal_expression(store, *parent))
        || store
            .parent(element)
            .as_ref()
            .is_some_and(|parent| ast::is_jsx_attributes(store, *parent))
    {
        Some(element)
    } else {
        None
    }
}

pub(crate) fn get_containing_object_literal_element_worker(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    match store.kind(node) {
        ast::Kind::StringLiteral
        | ast::Kind::NoSubstitutionTemplateLiteral
        | ast::Kind::NumericLiteral => {
            if store
                .parent(node)
                .is_some_and(|parent| store.kind(parent) == ast::Kind::ComputedPropertyName)
            {
                if store
                    .parent(node)
                    .and_then(|parent| store.parent(parent))
                    .as_ref()
                    .is_some_and(|parent| is_object_literal_or_jsx_element(store, *parent))
                {
                    return store.parent(node).and_then(|parent| store.parent(parent));
                }
                return None;
            }
            let parent = store.parent(node).unwrap();
            let grandparent = store.parent(parent).unwrap();
            if is_object_literal_or_jsx_element(store, parent)
                && (store.kind(grandparent) == ast::Kind::ObjectLiteralExpression
                    || store.kind(grandparent) == ast::Kind::JsxAttributes)
                && store
                    .name(parent)
                    .as_ref()
                    .is_some_and(|name| *name == node)
            {
                return Some(parent);
            }
        }
        ast::Kind::Identifier => {
            let parent = store.parent(node).unwrap();
            let grandparent = store.parent(parent).unwrap();
            if is_object_literal_or_jsx_element(store, parent)
                && (store.kind(grandparent) == ast::Kind::ObjectLiteralExpression
                    || store.kind(grandparent) == ast::Kind::JsxAttributes)
                && store
                    .name(parent)
                    .as_ref()
                    .is_some_and(|name| *name == node)
            {
                return Some(parent);
            }
        }
        _ => {}
    }
    None
}

pub(crate) fn is_object_literal_or_jsx_element(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_object_literal_element(store, &node)
        || ast::is_jsx_attribute(store, node)
        || ast::is_jsx_spread_attribute(store, node)
}

pub(crate) fn node_seen_tracker() -> impl FnMut(ast::Node) -> bool {
    let mut seen: HashSet<ast::Node> = HashSet::new();
    move |node: ast::Node| seen.insert(node)
}

pub(crate) fn to_context_range(
    text_range: Option<core::TextRange>,
    context_file: &ast::SourceFile,
    context: Option<ast::Node>,
) -> Option<core::TextRange> {
    let text_range = text_range?;
    let Some(context) = context else {
        return Some(text_range);
    };
    let context_range = crate::findallreferences::get_range_of_node(context, context_file, None);
    if context_range.pos() != text_range.pos() || context_range.end() != text_range.end() {
        Some(context_range)
    } else {
        None
    }
}

pub(crate) fn get_reference_at_position<'a>(
    source_file: &'a ast::SourceFile,
    position: i32,
    program: &'a compiler::Program,
) -> Option<crate::findallreferences::RefInfo<'a>> {
    if let Some(reference_path) = source_file
        .referenced_files()
        .iter()
        .find(|r| r.text_range.contains_inclusive(position))
    {
        if let Some(file) = program.get_source_file_from_reference_ref(source_file, reference_path)
        {
            return Some(crate::findallreferences::RefInfo {
                reference: Some(reference_path),
                file_name: file.file_name(),
                file: Some(file),
                unverified: false,
            });
        }
        return None;
    }

    if let Some(type_reference_directive) = source_file
        .type_reference_directives()
        .iter()
        .find(|r| r.text_range.contains_inclusive(position))
    {
        if let Some(reference) = program
            .get_resolved_type_reference_directive_from_type_reference_directive(
                type_reference_directive,
                source_file,
            )
        {
            if let Some(file) = program.get_source_file_ref(&reference.resolved_file_name) {
                return Some(crate::findallreferences::RefInfo {
                    reference: Some(type_reference_directive),
                    file_name: file.file_name(),
                    file: Some(file),
                    unverified: false,
                });
            }
        }
        return None;
    }

    if let Some(lib_reference_directive) = source_file
        .lib_reference_directives()
        .iter()
        .find(|r| r.text_range.contains_inclusive(position))
    {
        if let Some(file) = program.get_lib_file_from_reference_ref(lib_reference_directive) {
            return Some(crate::findallreferences::RefInfo {
                reference: Some(lib_reference_directive),
                file_name: file.file_name(),
                file: Some(file),
                unverified: false,
            });
        }
        return None;
    }

    if source_file.imports().is_empty() && source_file.module_augmentations().is_empty() {
        return None;
    }

    let node = astnav::get_touching_token(source_file, position)?;
    let node_text = source_file.store().text(node);
    if !is_module_specifier_like(source_file.store(), node)
        || !tspath::is_external_module_name_relative(&node_text)
    {
        return None;
    }

    if let Some(resolution) = program.get_resolved_module_from_module_specifier(source_file, &node)
    {
        let verified_file_name = resolution.resolved_file_name.clone();
        let mut file_name = resolution.resolved_file_name;
        if file_name.is_empty() {
            file_name = tspath::resolve_path(
                &tspath::get_directory_path(&source_file.file_name()),
                &[&node_text],
            );
        }
        let file = program.get_source_file_ref(&file_name);
        return Some(crate::findallreferences::RefInfo {
            file,
            file_name,
            reference: None,
            unverified: !verified_file_name.is_empty(),
        });
    }
    None
}

pub(crate) fn get_contextual_type_from_parent<'a>(
    store: &ast::AstStore,
    node: ast::Node,
    type_checker: &mut checker::Checker<'a, '_>,
    context_flags: checker::ContextFlags,
) -> Option<checker::TypeHandle> {
    let node_parent = store.parent(node);
    let parent = ast::walk_up_parenthesized_expressions(store, node_parent)?;
    match store.kind(parent) {
        ast::Kind::NewExpression => type_checker.get_contextual_type_public(parent, context_flags),
        ast::Kind::BinaryExpression => {
            if store.operator_token(parent).is_some_and(|operator_token| {
                crate::completions::is_equality_operator_kind(store.kind(operator_token))
            }) {
                let left = store.left(parent).unwrap();
                let right = store.right(parent).unwrap();
                let contextual_node = if right == node { left } else { right };
                Some(type_checker.get_type_at_location(contextual_node))
            } else {
                type_checker.get_contextual_type_public(node, context_flags)
            }
        }
        ast::Kind::CaseClause => {
            crate::completions::get_switched_type(store, &parent, type_checker)
        }
        _ => type_checker.get_contextual_type_public(node, context_flags),
    }
}

pub(crate) fn has_contextual_or_ancestor_string_literal_type(
    store: &ast::AstStore,
    node: ast::Node,
    type_checker: &mut checker::Checker<'_, '_>,
) -> bool {
    if let Some(contextual_type) =
        get_contextual_type_from_parent(store, node, type_checker, checker::CONTEXT_FLAGS_NONE)
    {
        return is_string_literal_or_union_of_string_literals(type_checker, contextual_type);
    }
    let Some(ancestor_type_node) = get_ancestor_type_node(store, node) else {
        return false;
    };
    let ancestor_type = type_checker.get_type_at_location(ancestor_type_node);
    is_string_literal_or_union_of_string_literals(type_checker, ancestor_type)
}

fn is_string_literal_or_union_of_string_literals(
    checker: &checker::Checker<'_, '_>,
    t: checker::TypeHandle,
) -> bool {
    checker.is_string_literal_type_public(t)
        || (checker.is_union_type_public(t)
            && checker
                .type_types_public(t)
                .iter()
                .all(|t| checker.is_string_literal_type_public(*t)))
}

pub(crate) fn get_ancestor_type_node(store: &ast::AstStore, node: ast::Node) -> Option<ast::Node> {
    let mut last_type_node: Option<ast::Node> = None;
    let mut current = Some(node);
    while let Some(n) = current {
        if ast::is_type_node(store, n) {
            last_type_node = Some(n);
        }
        let should_stop = !(store
            .parent(n)
            .as_ref()
            .is_some_and(|parent| ast::is_qualified_name(store, *parent))
            || store
                .parent(n)
                .as_ref()
                .is_some_and(|parent| ast::is_type_node(store, *parent))
            || store
                .parent(n)
                .as_ref()
                .is_some_and(|parent| ast::is_type_element(store, parent)));
        if should_stop {
            break;
        }
        current = store.parent(n);
    }
    last_type_node
}

pub(crate) fn is_source_file_with_global_exports(
    program: &dyn checker::Program,
    source_file: &ast::SourceFile,
    node: Option<&ast::Node>,
) -> bool {
    node.is_some_and(|node| {
        node.store_id() == source_file.store().store_id()
            && ast::is_source_file(source_file.store(), *node)
            && !program.binding_state(source_file).global_exports_is_empty()
    })
}
