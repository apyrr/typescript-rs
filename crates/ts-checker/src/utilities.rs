use std::cmp;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, OnceLock};

use ts_ast as ast;
use ts_binder as binder;
use ts_core as core;
use ts_debug as debug;
use ts_diagnostics as diagnostics;
use ts_jsnum as jsnum;
use ts_module as module;
use ts_printer as printer;
use ts_scanner as scanner;
use ts_tspath as tspath;

use crate::checker::*;

#[inline]
fn node_handle(node: ast::Node) -> ast::Node {
    node
}

fn node_matches_name(store: &ast::AstStore, left: ast::Node, right: ast::Node) -> bool {
    left == right || store.kind(left) == store.kind(right) && store.text(left) == store.text(right)
}

pub(crate) fn any_string(value: &LiteralValue) -> String {
    value.as_string().to_string()
}

pub(crate) fn any_number(value: &LiteralValue) -> jsnum::Number {
    value.as_number()
}

fn any_bool(value: &LiteralValue) -> bool {
    value.as_bool()
}

pub fn new_diagnostic_for_node(
    store: &ast::AstStore,
    node: Option<ast::Node>,
    message: &'static diagnostics::Message,
    args: impl IntoDiagnosticArgs,
) -> ast::Diagnostic {
    let mut file = None;
    let mut loc = core::TextRange::default();
    if let Some(node) = node {
        if let Some(source_file_node) = ast::get_source_file_of_node(store, Some(node)) {
            let source_file = store.source_file_view(source_file_node);
            loc = scanner::get_error_range_for_node(&source_file, &node);
            file = Some(source_file.diagnostic_file());
        }
    }
    let args = args
        .into_diagnostic_args()
        .into_iter()
        .map(|arg| Box::new(arg) as diagnostics::Argument)
        .collect::<Vec<_>>();
    ast::new_diagnostic_with_file(file, loc, message, &args)
}

pub fn new_diagnostic_chain_for_node(
    store: &ast::AstStore,
    chain: Option<ast::Diagnostic>,
    node: Option<ast::Node>,
    message: &'static diagnostics::Message,
    args: impl IntoDiagnosticArgs,
) -> ast::Diagnostic {
    if let Some(chain) = chain {
        let args = args
            .into_diagnostic_args()
            .into_iter()
            .map(|arg| Box::new(arg) as diagnostics::Argument)
            .collect::<Vec<_>>();
        return ast::new_diagnostic_chain(Some(chain), message, &args);
    }
    new_diagnostic_for_node(store, node, message, args)
}

pub(crate) fn find_in_map<K, V>(
    m: &HashMap<K, V>,
    mut predicate: impl FnMut(&V) -> bool,
) -> Option<&V>
where
    K: Eq + std::hash::Hash,
{
    for value in m.values() {
        if predicate(value) {
            return Some(value);
        }
    }
    None
}

pub(crate) fn token_is_identifier_or_keyword(token: ast::Kind) -> bool {
    token >= ast::Kind::Identifier
}

pub(crate) fn token_is_identifier_or_keyword_or_greater_than(token: ast::Kind) -> bool {
    token == ast::Kind::GreaterThanToken || token_is_identifier_or_keyword(token)
}

pub fn has_override_modifier(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::has_syntactic_modifier(store, node, ast::ModifierFlags::Override)
}

pub(crate) fn has_async_modifier(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::has_syntactic_modifier(store, node, ast::ModifierFlags::Async)
}

pub fn get_selected_modifier_flags(
    store: &ast::AstStore,
    node: ast::Node,
    flags: ast::ModifierFlags,
) -> ast::ModifierFlags {
    store
        .modifiers(node_handle(node))
        .map_or(ast::ModifierFlags::None, |modifiers| {
            modifiers.modifier_flags()
        })
        & flags
}

pub(crate) fn has_readonly_modifier(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::has_modifier(store, node, ast::ModifierFlags::Readonly)
}

pub(crate) fn is_empty_object_literal(store: &ast::AstStore, expression: ast::Node) -> bool {
    ast::is_object_literal_expression(store, node_handle(expression))
        && store
            .properties(node_handle(expression))
            .map_or(true, |properties| properties.is_empty())
}

pub type AssignmentKind = i32;

pub const ASSIGNMENT_KIND_NONE: AssignmentKind = 0;
pub const ASSIGNMENT_KIND_DEFINITE: AssignmentKind = 1;
pub const ASSIGNMENT_KIND_COMPOUND: AssignmentKind = 2;

pub type AssignmentTarget = ast::Node; // BinaryExpression | PrefixUnaryExpression | PostfixUnaryExpression | ForInOrOfStatement

pub(crate) fn get_assignment_target_kind(store: &ast::AstStore, node: ast::Node) -> AssignmentKind {
    let target = ast::get_assignment_target(store, node);
    let Some(target) = target else {
        return ASSIGNMENT_KIND_NONE;
    };
    match store.kind(target) {
        ast::Kind::BinaryExpression => {
            let binary_operator = store.kind(store.operator_token(target).unwrap());
            if binary_operator == ast::Kind::EqualsToken
                || ast::is_logical_or_coalescing_assignment_operator(binary_operator)
            {
                return ASSIGNMENT_KIND_DEFINITE;
            }
            ASSIGNMENT_KIND_COMPOUND
        }
        ast::Kind::PrefixUnaryExpression | ast::Kind::PostfixUnaryExpression => {
            ASSIGNMENT_KIND_COMPOUND
        }
        ast::Kind::ForInStatement | ast::Kind::ForOfStatement => ASSIGNMENT_KIND_DEFINITE,
        _ => panic!("Unhandled case in getAssignmentTargetKind"),
    }
}

pub(crate) fn is_delete_target(store: &ast::AstStore, node: ast::Node) -> bool {
    if !ast::is_access_expression(store, node_handle(node)) {
        return false;
    }
    let parent = store.parent(node_handle(node));
    let walked = ast::walk_up_parenthesized_expressions(store, parent);
    walked.is_some_and(|walked| store.kind(walked) == ast::Kind::DeleteExpression)
}

pub(crate) fn is_in_compound_like_assignment(store: &ast::AstStore, node: ast::Node) -> bool {
    let target = ast::get_assignment_target(store, node);
    target.is_some_and(|target| {
        ast::is_assignment_expression(store, target, true /*excludeCompoundAssignment*/)
            && is_compound_like_assignment(store, target)
    })
}

pub(crate) fn is_compound_like_assignment(store: &ast::AstStore, assignment: ast::Node) -> bool {
    let right_node = store.right(node_handle(assignment)).unwrap();
    let right = ast::skip_parentheses(store, right_node);
    store.kind(right) == ast::Kind::BinaryExpression
        && is_shift_operator_or_higher(store.kind(store.operator_token(right).unwrap()))
}

pub(crate) fn is_const_type_reference(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_const_type_reference(store, node)
}

pub fn get_single_variable_of_variable_statement<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    if !ast::is_variable_statement(store, node_handle(node)) {
        return None;
    }
    store
        .declaration_list(node_handle(node))
        .and_then(|declaration_list| store.declarations(declaration_list))
        .and_then(|declarations| declarations.first())
}

pub fn is_type_reference_identifier(store: &ast::AstStore, mut node: ast::Node) -> bool {
    while store
        .parent(node_handle(node))
        .is_some_and(|parent| store.kind(parent) == ast::Kind::QualifiedName)
    {
        node = store.parent(node_handle(node)).unwrap();
    }
    store
        .parent(node_handle(node))
        .is_some_and(|parent| ast::is_type_reference_node(store, parent))
}

pub fn is_in_type_query(store: &ast::AstStore, node: ast::Node) -> bool {
    // TypeScript 1.0 spec (April 2014): 3.6.3
    // A type query consists of the keyword typeof followed by an expression.
    // The expression is restricted to a single identifier or a sequence of identifiers separated by periods
    ast::find_ancestor_or_quit(store, Some(node), |store, n| match store.kind(n) {
        ast::Kind::TypeQuery => ast::FindAncestorResult::True,
        ast::Kind::Identifier | ast::Kind::QualifiedName => ast::FindAncestorResult::False,
        _ => ast::FindAncestorResult::Quit,
    })
    .is_some()
}

pub(crate) fn can_have_locals(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node_handle(node)) {
        ast::Kind::ArrowFunction
        | ast::Kind::Block
        | ast::Kind::CallSignature
        | ast::Kind::CaseBlock
        | ast::Kind::CatchClause
        | ast::Kind::ClassStaticBlockDeclaration
        | ast::Kind::ConditionalType
        | ast::Kind::Constructor
        | ast::Kind::ConstructorType
        | ast::Kind::ConstructSignature
        | ast::Kind::ForStatement
        | ast::Kind::ForInStatement
        | ast::Kind::ForOfStatement
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::FunctionType
        | ast::Kind::GetAccessor
        | ast::Kind::IndexSignature
        | ast::Kind::MappedType
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::ModuleDeclaration
        | ast::Kind::SetAccessor
        | ast::Kind::SourceFile
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::JSTypeAliasDeclaration => true,
        _ => false,
    }
}

pub(crate) fn is_shorthand_ambient_module(store: &ast::AstStore, node: Option<ast::Node>) -> bool {
    // The only kind of module that can be missing a body is a shorthand ambient module.
    node.is_some()
        && store.kind(node_handle(node.unwrap())) == ast::Kind::ModuleDeclaration
        && store.body(node_handle(node.unwrap())).is_none()
}

pub(crate) fn get_alias_declaration_from_name<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    let parent = store.parent(node_handle(node))?;
    match store.kind(parent) {
        ast::Kind::ImportClause
        | ast::Kind::ImportSpecifier
        | ast::Kind::NamespaceImport
        | ast::Kind::ExportSpecifier
        | ast::Kind::ExportAssignment
        | ast::Kind::ImportEqualsDeclaration
        | ast::Kind::NamespaceExport => Some(parent),
        ast::Kind::QualifiedName => get_alias_declaration_from_name(store, parent),
        _ => None,
    }
}

pub(crate) fn entity_name_to_string(store: &ast::AstStore, name: ast::Node) -> String {
    ast::entity_name_to_string(store, name, None)
}

pub(crate) fn get_containing_qualified_name_node<'a>(
    store: &'a ast::AstStore,
    mut node: ast::Node,
) -> ast::Node {
    while store
        .parent(node_handle(node))
        .is_some_and(|parent| ast::is_qualified_name(store, parent))
    {
        node = store.parent(node_handle(node)).unwrap();
    }
    node
}

pub(crate) fn is_side_effect_import(store: &ast::AstStore, node: ast::Node) -> bool {
    let ancestor = ast::find_ancestor(store, Some(node), ast::is_import_declaration);
    ancestor.is_some() && store.import_clause(ancestor.unwrap()).is_none()
}

pub(crate) fn get_external_module_require_argument<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    if ast::is_variable_declaration_initialized_to_require(store, node) {
        return store
            .initializer(node_handle(node))
            .and_then(|initializer| store.arguments(initializer).and_then(|args| args.first()));
    }
    None
}

pub(crate) fn is_right_side_of_access_expression(store: &ast::AstStore, node: ast::Node) -> bool {
    let Some(parent) = store.parent(node_handle(node)) else {
        return false;
    };
    (ast::is_property_access_expression(store, parent)
        && store
            .name(parent)
            .is_some_and(|name| node_matches_name(store, name, node)))
        || ast::is_element_access_expression(store, parent)
            && store
                .argument_expression(parent)
                .is_some_and(|argument| node_matches_name(store, argument, node))
}

pub(crate) fn is_top_level_in_external_module_augmentation(
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    let Some(parent) = store.parent(node_handle(node)) else {
        return false;
    };
    ast::is_module_block(store, parent)
        && store
            .parent(parent)
            .is_some_and(|parent| ast::is_external_module_augmentation(store, &parent))
}

pub(crate) fn is_syntactic_default(store: &ast::AstStore, node: ast::Node) -> bool {
    (ast::is_export_assignment(store, node_handle(node))
        && !store.is_export_equals(node_handle(node)).unwrap_or(false))
        || ast::has_syntactic_modifier(store, node, ast::ModifierFlags::Default)
        || ast::is_export_specifier(store, node_handle(node))
        || ast::is_namespace_export(store, node_handle(node))
}

pub(crate) fn is_type_alias(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_type_or_js_type_alias_declaration(store, node)
}

pub(crate) fn has_only_expression_initializer(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node_handle(node)) {
        ast::Kind::VariableDeclaration
        | ast::Kind::Parameter
        | ast::Kind::BindingElement
        | ast::Kind::PropertyDeclaration
        | ast::Kind::PropertyAssignment
        | ast::Kind::EnumMember => true,
        _ => false,
    }
}

pub fn has_dot_dot_dot_token(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node_handle(node)) {
        ast::Kind::Parameter
        | ast::Kind::BindingElement
        | ast::Kind::NamedTupleMember
        | ast::Kind::JsxExpression => store.dot_dot_dot_token(node_handle(node)).is_some(),
        _ => false,
    }
}

pub fn is_type_any<'a>(checker: &Checker<'a, '_>, t: Option<TypeHandle>) -> bool {
    t.is_some_and(|t| checker.type_flags(t) & TYPE_FLAGS_ANY != 0)
}

pub(crate) fn is_exclamation_token(store: &ast::AstStore, node: Option<ast::Node>) -> bool {
    node.is_some_and(|node| store.kind(node_handle(node)) == ast::Kind::ExclamationToken)
}

pub fn is_optional_declaration(store: &ast::AstStore, declaration: ast::Node) -> bool {
    ast::has_question_token(store, declaration)
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn is_optional_parameter(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        if ast::is_parameter_declaration(store, node_handle(node))
            && store.question_token(node_handle(node)).is_some()
        {
            return true;
        }
        if !ast::is_parameter_declaration(store, node_handle(node)) {
            return false;
        }
        if store.initializer(node_handle(node)).is_some() {
            let parent = store.parent(node_handle(node)).unwrap();
            let signature = self.get_signature_from_declaration(parent);
            let parameters = store.parameters(parent);
            let parameter_index = parameters
                .expect("function-like declaration should have parameters")
                .iter()
                .position(|p| p == node)
                .unwrap_or(usize::MAX);
            debug::assert(parameter_index != usize::MAX, None);
            // Only consider syntactic or instantiated parameters as optional, not `void` parameters as this function is used
            // in grammar checks and checking for `void` too early results in parameter types widening too early
            // and causes some noImplicitAny errors to be lost.
            return parameter_index
                >= self.get_min_argument_count_ex(
                    signature,
                    MIN_ARGUMENT_COUNT_FLAGS_STRONG_ARITY_FOR_UNTYPED_JS
                        | MIN_ARGUMENT_COUNT_FLAGS_VOID_IS_NON_OPTIONAL,
                );
        }
        let iife = store
            .parent(node_handle(node))
            .and_then(|parent| ast::get_immediately_invoked_function_expression(store, parent));
        if let Some(iife) = iife {
            let parent = store.parent(node_handle(node)).unwrap();
            let parameters = store.parameters(parent);
            let parameter_index = parameters
                .expect("function-like declaration should have parameters")
                .iter()
                .position(|p| p == node)
                .unwrap_or(usize::MAX);
            return store.type_node(node_handle(node)).is_none()
                && store.dot_dot_dot_token(node_handle(node)).is_none()
                && parameter_index >= self.get_effective_call_arguments(iife).len();
        }
        false
    }
}

pub(crate) fn is_empty_array_literal(store: &ast::AstStore, expression: ast::Node) -> bool {
    ast::is_array_literal_expression(store, node_handle(expression))
        && store
            .elements(node_handle(expression))
            .map_or(true, |elements| elements.is_empty())
}

pub(crate) fn declaration_belongs_to_private_ambient_member(
    store: &ast::AstStore,
    declaration: ast::Node,
) -> bool {
    let root = ast::get_root_declaration(store, declaration);
    let mut member_declaration = Some(root);
    if store.kind(root) == ast::Kind::Parameter {
        member_declaration = store.parent(root);
    }
    member_declaration.is_some_and(|node| is_private_within_ambient(store, node))
}

pub(crate) fn is_private_within_ambient(store: &ast::AstStore, node: ast::Node) -> bool {
    (ast::has_modifier(store, node, ast::ModifierFlags::Private)
        || ast::is_private_identifier_class_element_declaration(store, node))
        && store.flags(node_handle(node)) & ast::NodeFlags::Ambient != 0
}

pub(crate) fn is_type_assertion(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_assertion_expression(store, &ast::skip_parentheses(store, node))
}

pub(crate) fn create_symbol_table(
    symbols: &[(ast::SymbolName, SymbolIdentity)],
) -> SymbolIdentityTable {
    if symbols.is_empty() {
        return SymbolIdentityTable::default();
    }
    let mut result = SymbolIdentityTable::default();
    for (name, symbol) in symbols.iter() {
        result.insert(name.clone(), *symbol);
    }
    result
}

fn compare_symbol_identity_ids(
    checker: &Checker<'_, '_>,
    s1: SymbolIdentity,
    s2: SymbolIdentity,
) -> isize {
    match checker.compare_symbol_identity_tiebreaker(s1, s2) {
        cmp::Ordering::Less => -1,
        cmp::Ordering::Equal => 0,
        cmp::Ordering::Greater => 1,
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct SourceOrderKey {
    file_order: usize,
    pos: i32,
}

impl SourceOrderKey {
    fn for_node(checker: &Checker<'_, '_>, node: ast::Node) -> Self {
        let source_file = checker.try_source_file_for_node_with_order(node);
        let file_order = source_file.map(|(_, order)| order).unwrap_or(usize::MAX);
        Self {
            file_order,
            pos: source_order_node_pos(checker, node, source_file.map(|(file, _)| file)),
        }
    }

    fn pos(self) -> i32 {
        self.pos
    }

    fn cmp(self, other: Self) -> cmp::Ordering {
        self.file_order
            .cmp(&other.file_order)
            .then_with(|| self.pos().cmp(&other.pos()))
    }
}

struct SymbolIdentitySortKey<'a> {
    symbol: SymbolIdentity,
    declaration: Option<SourceOrderKey>,
    name: &'a ast::SymbolName,
    symbol_id: ast::SymbolId,
}

impl<'a> SymbolIdentitySortKey<'a> {
    fn new(checker: &'a Checker<'_, '_>, symbol: SymbolIdentity) -> Self {
        Self {
            symbol,
            declaration: checker
                .first_symbol_identity_declaration(symbol)
                .map(|declaration| SourceOrderKey::for_node(checker, declaration)),
            name: checker.missing_name_symbol_identity_name_ref(symbol),
            symbol_id: checker.symbol_handle_id(symbol.symbol_handle()),
        }
    }

    fn cmp(&self, other: &Self) -> cmp::Ordering {
        compare_optional_source_order_keys(self.declaration, other.declaration)
            .then_with(|| self.name.cmp(&other.name))
            // Fall back to symbol IDs. This is a last resort that should happen only when symbols have
            // no declaration and duplicate names.
            .then_with(|| self.symbol_id.cmp_for_ordering(other.symbol_id))
    }
}

fn source_order_node_pos(
    checker: &Checker<'_, '_>,
    node: ast::Node,
    source_file: Option<&ast::SourceFile>,
) -> i32 {
    let factory_store = checker.factory().store();
    if node.store_id() == factory_store.store_id() {
        return factory_store.loc(node_handle(node)).pos();
    }
    source_file
        .map(ast::SourceFile::store)
        .unwrap_or_else(|| checker.store_for_node(node))
        .loc(node_handle(node))
        .pos()
}

fn compare_optional_source_order_keys(
    left: Option<SourceOrderKey>,
    right: Option<SourceOrderKey>,
) -> cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => cmp::Ordering::Less,
        (None, Some(_)) => cmp::Ordering::Greater,
        (None, None) => cmp::Ordering::Equal,
    }
}

fn compare_nodes_by_source_order<'a>(
    checker: &Checker<'a, '_>,
    n1: Option<ast::Node>,
    n2: Option<ast::Node>,
) -> isize {
    if n1 == n2 {
        return 0;
    }
    if n1.is_none() {
        return 1;
    }
    if n2.is_none() {
        return -1;
    }
    let n1 = n1.unwrap();
    let n2 = n2.unwrap();
    if let Some(store) = checker.try_same_non_shared_source_store_for_nodes(n1, n2) {
        return store.loc(node_handle(n1)).pos() as isize
            - store.loc(node_handle(n2)).pos() as isize;
    }
    let s1 = checker.try_source_file_for_node_with_order(n1);
    let s2 = checker.try_source_file_for_node_with_order(n2);
    let same_file = match (s1, s2) {
        (Some((_, f1)), Some((_, f2))) => f1 == f2,
        (None, None) => true,
        _ => false,
    };
    if !same_file {
        let f1 = s1.map(|(_, index)| index).unwrap_or(usize::MAX);
        let f2 = s2.map(|(_, index)| index).unwrap_or(usize::MAX);
        return f1 as isize - f2 as isize;
    }
    source_order_node_pos(checker, n1, s1.map(|(file, _)| file)) as isize
        - source_order_node_pos(checker, n2, s2.map(|(file, _)| file)) as isize
}

fn compare_symbol_identities_by_source_order<'a>(
    checker: &Checker<'a, '_>,
    s1: Option<SymbolIdentity>,
    s2: Option<SymbolIdentity>,
) -> isize {
    let (s1, s2) = match (s1, s2) {
        (Some(s1), Some(s2)) => {
            if s1 == s2 {
                return 0;
            }
            (s1, s2)
        }
        (None, None) => return 0,
        (None, Some(_)) => return 1,
        (Some(_), None) => return -1,
    };
    let declaration1 = checker.first_symbol_identity_declaration(s1);
    let declaration2 = checker.first_symbol_identity_declaration(s2);
    if declaration1.is_some() && declaration2.is_some() {
        let r = compare_nodes_by_source_order(checker, declaration1, declaration2);
        if r != 0 {
            return r;
        }
    } else if declaration1.is_some() {
        return -1;
    } else if declaration2.is_some() {
        return 1;
    }
    match checker
        .missing_name_symbol_identity_name_ref(s1)
        .cmp(checker.missing_name_symbol_identity_name_ref(s2))
    {
        cmp::Ordering::Less => return -1,
        cmp::Ordering::Greater => return 1,
        cmp::Ordering::Equal => {}
    }
    compare_symbol_identity_ids(checker, s1, s2)
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn sort_symbol_identities(&mut self, symbols: &mut [SymbolIdentity]) {
        if symbols.len() < 2 {
            return;
        }
        let mut keyed = symbols
            .iter()
            .copied()
            .map(|symbol| SymbolIdentitySortKey::new(self, symbol))
            .collect::<Vec<_>>();
        keyed.sort_by(|left, right| left.cmp(right));
        for (symbol, key) in symbols.iter_mut().zip(keyed) {
            *symbol = key.symbol;
        }
    }

    pub(crate) fn compare_symbol_identities_worker(
        &self,
        s1: Option<SymbolIdentity>,
        s2: Option<SymbolIdentity>,
    ) -> isize {
        compare_symbol_identities_by_source_order(self, s1, s2)
    }

    fn compare_nodes(&self, n1: Option<ast::Node>, n2: Option<ast::Node>) -> isize {
        compare_nodes_by_source_order(self, n1, n2)
    }
}

pub fn compare_types<'a>(
    checker: &Checker<'a, '_>,
    t1: Option<TypeHandle>,
    t2: Option<TypeHandle>,
) -> isize {
    if t1 == t2 {
        return 0;
    }
    if t1.is_none() {
        return -1;
    }
    if t2.is_none() {
        return 1;
    }
    let t1 = t1.unwrap();
    let t2 = t2.unwrap();
    let r1 = checker.type_record(t1);
    let r2 = checker.type_record(t2);
    let flags1 = r1.flags;
    let flags2 = r2.flags;
    let object_flags1 = r1.object_flags;
    let object_flags2 = r2.object_flags;
    // First sort in order of increasing type flags values.
    let c = get_sort_order_flags_from_flags(flags1) - get_sort_order_flags_from_flags(flags2);
    if c != 0 {
        return c;
    }
    // Order named types by name and, in the case of aliased types, by alias type arguments.
    let c = compare_type_names_from_records(
        checker,
        r1,
        r2,
        flags1,
        flags2,
        object_flags1,
        object_flags2,
    );
    if c != 0 {
        return c;
    }
    // We have unnamed types or types with identical names. Now sort by data specific to the type.
    match () {
        _ if flags1
            & (TYPE_FLAGS_ANY
                | TYPE_FLAGS_UNKNOWN
                | TYPE_FLAGS_STRING
                | TYPE_FLAGS_NUMBER
                | TYPE_FLAGS_BOOLEAN
                | TYPE_FLAGS_BIG_INT
                | TYPE_FLAGS_ES_SYMBOL
                | TYPE_FLAGS_VOID
                | TYPE_FLAGS_UNDEFINED
                | TYPE_FLAGS_NULL
                | TYPE_FLAGS_NEVER
                | TYPE_FLAGS_NON_PRIMITIVE)
            != 0 => {}
        _ if flags1 & TYPE_FLAGS_OBJECT != 0 => {
            let c = compare_symbol_identities_by_source_order(checker, r1.symbol, r2.symbol);
            if c != 0 {
                return c;
            }
            if object_flags1 & OBJECT_FLAGS_REFERENCE != 0
                && object_flags2 & OBJECT_FLAGS_REFERENCE != 0
            {
                let reference1 = r1.as_type_reference().unwrap();
                let reference2 = r2.as_type_reference().unwrap();
                let target1 = reference1.object.target.unwrap();
                let target2 = reference2.object.target.unwrap();
                let target_record1 = checker.type_record(target1);
                let target_record2 = checker.type_record(target2);
                if target_record1.object_flags & OBJECT_FLAGS_TUPLE != 0
                    && target_record2.object_flags & OBJECT_FLAGS_TUPLE != 0
                {
                    // Tuple types have no associated symbol, instead we order by tuple element information.
                    let c = compare_tuple_types(
                        checker,
                        target_record1.as_tuple_type(),
                        target_record2.as_tuple_type(),
                    );
                    if c != 0 {
                        return c;
                    }
                }
                if reference1.node.is_none() && reference2.node.is_none() {
                    // Non-deferred type references with the same target are sorted by their type argument lists.
                    let c = compare_type_lists(
                        checker,
                        reference1.resolved_type_arguments.as_ref().unwrap(),
                        reference2.resolved_type_arguments.as_ref().unwrap(),
                    );
                    if c != 0 {
                        return c;
                    }
                } else {
                    // Deferred type references with the same target are ordered by the source location of the reference.
                    let c =
                        compare_nodes_by_source_order(checker, reference1.node, reference2.node);
                    if c != 0 {
                        return c;
                    }
                    // Instantiations of the same deferred type reference are ordered by their associated type mappers
                    // (which reflect the mapping of in-scope type parameters to type arguments).
                    let m1 = r1.as_object_type().unwrap().mapper;
                    let m2 = r2.as_object_type().unwrap().mapper;
                    let c = compare_type_mappers(checker, m1, m2);
                    if c != 0 {
                        return c;
                    }
                }
            } else if object_flags1 & OBJECT_FLAGS_REFERENCE != 0 {
                return -1;
            } else if object_flags2 & OBJECT_FLAGS_REFERENCE != 0 {
                return 1;
            } else {
                // Order unnamed non-reference object types by kind associated type mappers. Reverse mapped types have
                // neither symbols nor mappers so they're ultimately ordered by unstable type IDs, but given their rarity
                // this should be fine.
                let c = (object_flags1 & OBJECT_FLAGS_OBJECT_TYPE_KIND_MASK) as isize
                    - (object_flags2 & OBJECT_FLAGS_OBJECT_TYPE_KIND_MASK) as isize;
                if c != 0 {
                    return c;
                }
                let m1 = r1.as_object_type().unwrap().mapper;
                let m2 = r2.as_object_type().unwrap().mapper;
                let c = compare_type_mappers(checker, m1, m2);
                if c != 0 {
                    return c;
                }
            }
        }
        _ if flags1 & TYPE_FLAGS_UNION != 0 => {
            // Unions are ordered by origin and then constituent type lists.
            let u1 = r1.as_union_type();
            let u2 = r2.as_union_type();
            let o1 = u1.origin;
            let o2 = u2.origin;
            if o1.is_none() && o2.is_none() {
                let c = compare_type_lists(
                    checker,
                    &u1.union_or_intersection.types,
                    &u2.union_or_intersection.types,
                );
                if c != 0 {
                    return c;
                }
            } else if o1.is_none() {
                return 1;
            } else if o2.is_none() {
                return -1;
            } else {
                let c = compare_types(checker, o1, o2);
                if c != 0 {
                    return c;
                }
            }
        }
        _ if flags1 & TYPE_FLAGS_INTERSECTION != 0 => {
            // Intersections are ordered by their constituent type lists.
            let c = compare_type_lists(
                checker,
                &r1.as_intersection_type().union_or_intersection.types,
                &r2.as_intersection_type().union_or_intersection.types,
            );
            if c != 0 {
                return c;
            }
        }
        _ if flags1 & (TYPE_FLAGS_ENUM | TYPE_FLAGS_ENUM_LITERAL | TYPE_FLAGS_UNIQUE_ES_SYMBOL)
            != 0 =>
        {
            // Enum members are ordered by their symbol (and thus their declaration order).
            let c = compare_symbol_identities_by_source_order(checker, r1.symbol, r2.symbol);
            if c != 0 {
                return c;
            }
        }
        _ if flags1 & TYPE_FLAGS_STRING_LITERAL != 0 => {
            // String literal types are ordered by their values.
            let c = any_string(&r1.as_literal_type().value)
                .cmp(&any_string(&r2.as_literal_type().value));
            if c != cmp::Ordering::Equal {
                return if c == cmp::Ordering::Less { -1 } else { 1 };
            }
        }
        _ if flags1 & TYPE_FLAGS_NUMBER_LITERAL != 0 => {
            // Numeric literal types are ordered by their values.
            let c = jsnum::compare(
                any_number(&r1.as_literal_type().value),
                any_number(&r2.as_literal_type().value),
            );
            if c != 0 {
                return c as isize;
            }
        }
        _ if flags1 & TYPE_FLAGS_BOOLEAN_LITERAL != 0 => {
            let b1 = any_bool(&r1.as_literal_type().value);
            let b2 = any_bool(&r2.as_literal_type().value);
            if b1 != b2 {
                return if b1 { 1 } else { -1 };
            }
        }
        _ if flags1 & TYPE_FLAGS_TYPE_PARAMETER != 0 => {
            let c = compare_symbol_identities_by_source_order(checker, r1.symbol, r2.symbol);
            if c != 0 {
                return c;
            }
        }
        _ if flags1 & TYPE_FLAGS_INDEX != 0 => {
            let i1 = r1.as_index_type();
            let i2 = r2.as_index_type();
            let c = compare_types(checker, i1.target, i2.target);
            if c != 0 {
                return c;
            }
            let c = i1.index_flags as isize - i2.index_flags as isize;
            if c != 0 {
                return c;
            }
        }
        _ if flags1 & TYPE_FLAGS_INDEXED_ACCESS != 0 => {
            let i1 = r1.as_indexed_access_type();
            let i2 = r2.as_indexed_access_type();
            let c = compare_types(checker, i1.object_type, i2.object_type);
            if c != 0 {
                return c;
            }
            let c = compare_types(checker, i1.index_type, i2.index_type);
            if c != 0 {
                return c;
            }
        }
        _ if flags1 & TYPE_FLAGS_CONDITIONAL != 0 => {
            let c1 = r1.as_conditional_type();
            let c2 = r2.as_conditional_type();
            let n1 = checker
                .semantic_state
                .conditional_root_record(c1.root.unwrap())
                .node
                .unwrap();
            let n2 = checker
                .semantic_state
                .conditional_root_record(c2.root.unwrap())
                .node
                .unwrap();
            let c = compare_nodes_by_source_order(checker, Some(n1), Some(n2));
            if c != 0 {
                return c;
            }
            let c = compare_type_mappers(checker, c1.mapper, c2.mapper);
            if c != 0 {
                return c;
            }
        }
        _ if flags1 & TYPE_FLAGS_SUBSTITUTION != 0 => {
            let s1 = r1.as_substitution_type();
            let s2 = r2.as_substitution_type();
            let c = compare_types(checker, s1.base_type, s2.base_type);
            if c != 0 {
                return c;
            }
            let c = compare_types(checker, s1.constraint, s2.constraint);
            if c != 0 {
                return c;
            }
        }
        _ if flags1 & TYPE_FLAGS_TEMPLATE_LITERAL != 0 => {
            let tl1 = r1.as_template_literal_type();
            let tl2 = r2.as_template_literal_type();
            let c = tl1.texts.cmp(&tl2.texts);
            if c != cmp::Ordering::Equal {
                return if c == cmp::Ordering::Less { -1 } else { 1 };
            }
            let c = compare_type_lists(checker, &tl1.types, &tl2.types);
            if c != 0 {
                return c;
            }
        }
        _ if flags1 & TYPE_FLAGS_STRING_MAPPING != 0 => {
            let s1 = r1.as_string_mapping_type();
            let s2 = r2.as_string_mapping_type();
            let c = compare_types(checker, s1.target, s2.target);
            if c != 0 {
                return c;
            }
        }
        _ => {}
    }
    // Fall back to type IDs. This results in type creation order for built-in types.
    r1.ts_id as isize - r2.ts_id as isize
}

#[inline]
fn get_sort_order_flags_from_flags(flags: TypeFlags) -> isize {
    // Return TypeFlagsEnum for all enum-like unit types (they'll be sorted by their symbols)
    if flags & (TYPE_FLAGS_ENUM_LITERAL | TYPE_FLAGS_ENUM) != 0 && flags & TYPE_FLAGS_UNION == 0 {
        return TYPE_FLAGS_ENUM as isize;
    }
    flags as isize
}

fn compare_type_names_from_records<'a>(
    checker: &Checker<'a, '_>,
    r1: &crate::semantic::TypeRecord,
    r2: &crate::semantic::TypeRecord,
    flags1: TypeFlags,
    flags2: TypeFlags,
    object_flags1: ObjectFlags,
    object_flags2: ObjectFlags,
) -> isize {
    let s1 = get_type_name_symbol_identity_from_record(checker, r1, flags1, object_flags1);
    let s2 = get_type_name_symbol_identity_from_record(checker, r2, flags2, object_flags2);
    if s1 == s2 {
        if let Some(alias1) = r1.alias {
            let alias1 = checker.semantic_state.type_alias_record(alias1);
            let alias2 = r2
                .alias
                .map(|alias| checker.semantic_state.type_alias_record(alias))
                .expect("matching aliased type must have alias record");
            return compare_type_lists(checker, &alias1.type_arguments, &alias2.type_arguments);
        }
        return 0;
    }
    if s1.is_none() {
        return 1;
    }
    if s2.is_none() {
        return -1;
    }
    match checker
        .missing_name_symbol_identity_name_ref(s1.unwrap())
        .cmp(checker.missing_name_symbol_identity_name_ref(s2.unwrap()))
    {
        cmp::Ordering::Less => -1,
        cmp::Ordering::Equal => 0,
        cmp::Ordering::Greater => 1,
    }
}

fn get_type_name_symbol_identity_from_record<'a>(
    checker: &Checker<'a, '_>,
    record: &crate::semantic::TypeRecord,
    flags: TypeFlags,
    object_flags: ObjectFlags,
) -> Option<SymbolIdentity> {
    if let Some(alias) = record.alias {
        return checker.semantic_state.type_alias_record(alias).symbol;
    }
    if flags & (TYPE_FLAGS_TYPE_PARAMETER | TYPE_FLAGS_STRING_MAPPING) != 0
        || object_flags & (OBJECT_FLAGS_CLASS_OR_INTERFACE | OBJECT_FLAGS_REFERENCE) != 0
    {
        return record.symbol;
    }
    None
}

pub(crate) fn compare_tuple_types<'a>(
    checker: &Checker<'a, '_>,
    t1: &TupleTypeRecord,
    t2: &TupleTypeRecord,
) -> isize {
    if t1.readonly != t2.readonly {
        return if t1.readonly { 1 } else { -1 };
    }
    if t1.element_infos.len() != t2.element_infos.len() {
        return t1.element_infos.len() as isize - t2.element_infos.len() as isize;
    }
    for i in 0..t1.element_infos.len() {
        let c = t1.element_infos[i].flags as isize - t2.element_infos[i].flags as isize;
        if c != 0 {
            return c;
        }
    }
    for i in 0..t1.element_infos.len() {
        let c = compare_element_labels(
            checker,
            t1.element_infos[i].labeled_declaration,
            t2.element_infos[i].labeled_declaration,
        );
        if c != 0 {
            return c;
        }
    }
    0
}

pub(crate) fn compare_element_labels<'a>(
    checker: &Checker<'a, '_>,
    n1: Option<ast::Node>,
    n2: Option<ast::Node>,
) -> isize {
    if n1 == n2 {
        return 0;
    }
    if n1.is_none() {
        return -1;
    }
    if n2.is_none() {
        return 1;
    }
    let n1 = n1.unwrap();
    let n2 = n2.unwrap();
    let store1 = checker.store_for_node(n1);
    let store2 = checker.store_for_node(n2);
    let name1 = store1.name(node_handle(n1)).unwrap();
    let name2 = store2.name(node_handle(n2)).unwrap();
    match store1.text(name1).cmp(&store2.text(name2)) {
        cmp::Ordering::Less => -1,
        cmp::Ordering::Equal => 0,
        cmp::Ordering::Greater => 1,
    }
}

pub(crate) fn compare_type_lists<'a>(
    checker: &Checker<'a, '_>,
    s1: &[TypeHandle],
    s2: &[TypeHandle],
) -> isize {
    if s1.len() != s2.len() {
        return s1.len() as isize - s2.len() as isize;
    }
    for (i, t1) in s1.iter().enumerate() {
        let c = compare_types(checker, Some(*t1), Some(s2[i]));
        if c != 0 {
            return c;
        }
    }
    0
}

pub(crate) fn compare_type_mappers<'a>(
    checker: &Checker<'a, '_>,
    m1: Option<TypeMapperHandle>,
    m2: Option<TypeMapperHandle>,
) -> isize {
    fn compare_bool(left: bool, right: bool) -> isize {
        match left.cmp(&right) {
            cmp::Ordering::Less => -1,
            cmp::Ordering::Equal => 0,
            cmp::Ordering::Greater => 1,
        }
    }

    fn compare_u64(left: u64, right: u64) -> isize {
        match left.cmp(&right) {
            cmp::Ordering::Less => -1,
            cmp::Ordering::Equal => 0,
            cmp::Ordering::Greater => 1,
        }
    }

    fn compare_usize(left: usize, right: usize) -> isize {
        match left.cmp(&right) {
            cmp::Ordering::Less => -1,
            cmp::Ordering::Equal => 0,
            cmp::Ordering::Greater => 1,
        }
    }

    fn compare_deferred_type_mapper_target_lists<'a>(
        checker: &Checker<'a, '_>,
        left: &[DeferredTypeMapperTarget],
        right: &[DeferredTypeMapperTarget],
    ) -> isize {
        if left.len() != right.len() {
            return left.len() as isize - right.len() as isize;
        }
        for (index, left) in left.iter().enumerate() {
            let c = compare_deferred_type_mapper_targets(checker, left, &right[index]);
            if c != 0 {
                return c;
            }
        }
        0
    }

    fn compare_deferred_type_mapper_targets<'a>(
        checker: &Checker<'a, '_>,
        left: &DeferredTypeMapperTarget,
        right: &DeferredTypeMapperTarget,
    ) -> isize {
        match (left, right) {
            (
                DeferredTypeMapperTarget::EffectiveTypeArgumentAtIndex {
                    parent: left_parent,
                    type_parameters: left_type_parameters,
                    index: left_index,
                },
                DeferredTypeMapperTarget::EffectiveTypeArgumentAtIndex {
                    parent: right_parent,
                    type_parameters: right_type_parameters,
                    index: right_index,
                },
            ) => {
                let c = checker.compare_nodes(Some(*left_parent), Some(*right_parent));
                if c != 0 {
                    return c;
                }
                let c = compare_type_lists(checker, left_type_parameters, right_type_parameters);
                if c != 0 {
                    return c;
                }
                compare_usize(*left_index, *right_index)
            }
        }
    }

    fn mapper_rank(mapper: &TypeMapperRecordData) -> isize {
        match mapper {
            TypeMapperRecordData::Identity => 0,
            TypeMapperRecordData::Simple(_) => 1,
            TypeMapperRecordData::Array(_) => 2,
            TypeMapperRecordData::ArrayToSingle(_) => 3,
            TypeMapperRecordData::Deferred(_) => 4,
            TypeMapperRecordData::Function(_) => 5,
            TypeMapperRecordData::Merged(_) => 6,
            TypeMapperRecordData::Composite(_) => 7,
            TypeMapperRecordData::Inference(_) => 8,
        }
    }

    if m1 == m2 {
        return 0;
    }
    if m1.is_none() {
        return 1;
    }
    if m2.is_none() {
        return -1;
    }
    let record1 = &checker.semantic_state.mapper_record(m1.unwrap()).data;
    let record2 = &checker.semantic_state.mapper_record(m2.unwrap()).data;
    let rank1 = mapper_rank(record1);
    let rank2 = mapper_rank(record2);
    if rank1 != rank2 {
        return rank1 - rank2;
    }
    match (record1, record2) {
        (TypeMapperRecordData::Identity, TypeMapperRecordData::Identity) => 0,
        (TypeMapperRecordData::Simple(m1), TypeMapperRecordData::Simple(m2)) => {
            let c = compare_types(checker, Some(m1.source), Some(m2.source));
            if c != 0 {
                return c;
            }
            compare_types(checker, Some(m1.target), Some(m2.target))
        }
        (TypeMapperRecordData::Array(m1), TypeMapperRecordData::Array(m2)) => {
            let c = compare_type_lists(checker, &m1.sources, &m2.sources);
            if c != 0 {
                return c;
            }
            compare_type_lists(checker, &m1.targets, &m2.targets)
        }
        (TypeMapperRecordData::ArrayToSingle(m1), TypeMapperRecordData::ArrayToSingle(m2)) => {
            let c = compare_type_lists(checker, &m1.sources, &m2.sources);
            if c != 0 {
                return c;
            }
            compare_types(checker, Some(m1.target), Some(m2.target))
        }
        (TypeMapperRecordData::Deferred(m1), TypeMapperRecordData::Deferred(m2)) => {
            let c = compare_type_lists(checker, &m1.sources, &m2.sources);
            if c != 0 {
                return c;
            }
            let c = compare_deferred_type_mapper_target_lists(checker, &m1.targets, &m2.targets);
            if c != 0 {
                return c;
            }
            compare_u64(m1.identity, m2.identity)
        }
        (TypeMapperRecordData::Function(m1), TypeMapperRecordData::Function(m2)) => {
            match m1.kind.cmp(&m2.kind) {
                cmp::Ordering::Less => -1,
                cmp::Ordering::Equal => 0,
                cmp::Ordering::Greater => 1,
            }
        }
        (TypeMapperRecordData::Merged(m1), TypeMapperRecordData::Merged(m2)) => {
            let c = compare_type_mappers(checker, Some(m1.left), Some(m2.left));
            if c != 0 {
                return c;
            }
            compare_type_mappers(checker, Some(m1.right), Some(m2.right))
        }
        (TypeMapperRecordData::Composite(m1), TypeMapperRecordData::Composite(m2)) => {
            let c = compare_type_mappers(checker, Some(m1.left), Some(m2.left));
            if c != 0 {
                return c;
            }
            compare_type_mappers(checker, Some(m1.right), Some(m2.right))
        }
        (TypeMapperRecordData::Inference(m1), TypeMapperRecordData::Inference(m2)) => {
            let c = compare_bool(m1.fixing, m2.fixing);
            if c != 0 {
                return c;
            }
            let c = match m1.context.cmp(&m2.context) {
                cmp::Ordering::Less => -1,
                cmp::Ordering::Equal => 0,
                cmp::Ordering::Greater => 1,
            };
            if c != 0 {
                return c;
            }
            compare_u64(m1.identity, m2.identity)
        }
        _ => unreachable!("mapper ranks matched for different variants"),
    }
}

pub(crate) fn is_exponentiation_operator(kind: ast::Kind) -> bool {
    kind == ast::Kind::AsteriskAsteriskToken
}
pub(crate) fn is_multiplicative_operator(kind: ast::Kind) -> bool {
    kind == ast::Kind::AsteriskToken
        || kind == ast::Kind::SlashToken
        || kind == ast::Kind::PercentToken
}
pub(crate) fn is_multiplicative_operator_or_higher(kind: ast::Kind) -> bool {
    is_exponentiation_operator(kind) || is_multiplicative_operator(kind)
}
pub(crate) fn is_additive_operator(kind: ast::Kind) -> bool {
    kind == ast::Kind::PlusToken || kind == ast::Kind::MinusToken
}
pub(crate) fn is_additive_operator_or_higher(kind: ast::Kind) -> bool {
    is_additive_operator(kind) || is_multiplicative_operator_or_higher(kind)
}
pub(crate) fn is_shift_operator(kind: ast::Kind) -> bool {
    kind == ast::Kind::LessThanLessThanToken
        || kind == ast::Kind::GreaterThanGreaterThanToken
        || kind == ast::Kind::GreaterThanGreaterThanGreaterThanToken
}
pub(crate) fn is_shift_operator_or_higher(kind: ast::Kind) -> bool {
    is_shift_operator(kind) || is_additive_operator_or_higher(kind)
}
pub(crate) fn is_relational_operator(kind: ast::Kind) -> bool {
    kind == ast::Kind::LessThanToken
        || kind == ast::Kind::LessThanEqualsToken
        || kind == ast::Kind::GreaterThanToken
        || kind == ast::Kind::GreaterThanEqualsToken
        || kind == ast::Kind::InstanceOfKeyword
        || kind == ast::Kind::InKeyword
}
pub(crate) fn is_relational_operator_or_higher(kind: ast::Kind) -> bool {
    is_relational_operator(kind) || is_shift_operator_or_higher(kind)
}
pub(crate) fn is_equality_operator(kind: ast::Kind) -> bool {
    kind == ast::Kind::EqualsEqualsToken
        || kind == ast::Kind::EqualsEqualsEqualsToken
        || kind == ast::Kind::ExclamationEqualsToken
        || kind == ast::Kind::ExclamationEqualsEqualsToken
}
pub(crate) fn is_equality_operator_or_higher(kind: ast::Kind) -> bool {
    is_equality_operator(kind) || is_relational_operator_or_higher(kind)
}
pub(crate) fn is_bitwise_operator(kind: ast::Kind) -> bool {
    kind == ast::Kind::AmpersandToken
        || kind == ast::Kind::BarToken
        || kind == ast::Kind::CaretToken
}
pub(crate) fn is_bitwise_operator_or_higher(kind: ast::Kind) -> bool {
    is_bitwise_operator(kind) || is_equality_operator_or_higher(kind)
}
pub(crate) fn is_logical_operator_or_higher(kind: ast::Kind) -> bool {
    ast::is_logical_binary_operator(kind) || is_bitwise_operator_or_higher(kind)
}
pub(crate) fn is_assignment_operator_or_higher(kind: ast::Kind) -> bool {
    kind == ast::Kind::QuestionQuestionToken
        || is_logical_operator_or_higher(kind)
        || ast::is_assignment_operator(kind)
}
pub(crate) fn is_binary_operator(kind: ast::Kind) -> bool {
    is_assignment_operator_or_higher(kind) || kind == ast::Kind::CommaToken
}

pub(crate) fn is_object_literal_type<'a>(checker: &Checker<'a, '_>, t: TypeHandle) -> bool {
    checker.object_flags(t) & OBJECT_FLAGS_OBJECT_LITERAL != 0
}

pub(crate) fn is_declaration_readonly(store: &ast::AstStore, declaration: ast::Node) -> bool {
    ast::get_combined_modifier_flags(store, declaration).intersects(ast::ModifierFlags::Readonly)
        && !store
            .parent(node_handle(declaration))
            .is_some_and(|parent| {
                ast::is_parameter_property_declaration(store, declaration, parent)
            })
}

pub(crate) struct OrderedSet<T: Eq + std::hash::Hash + Clone> {
    values_by_key: HashSet<T>,
    values: Vec<T>,
}

impl<T: Eq + std::hash::Hash + Clone> OrderedSet<T> {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            values_by_key: HashSet::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn contains(&self, value: T) -> bool {
        self.values_by_key.contains(&value)
    }

    pub(crate) fn add(&mut self, value: T) {
        self.values_by_key.insert(value.clone());
        self.values.push(value);
    }

    pub(crate) fn into_values(self) -> Vec<T> {
        self.values
    }
}

pub(crate) fn get_containing_function_or_class_static_block<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    let parent = store.parent(node_handle(node));
    ast::find_ancestor(store, parent, |store, ancestor| {
        ast::is_function_like_or_class_static_block_declaration(store, Some(ancestor))
    })
}

pub(crate) fn is_node_descendant_of(
    store: &ast::AstStore,
    node: Option<ast::Node>,
    ancestor: ast::Node,
) -> bool {
    ast::is_node_descendant_of(store, node, Some(ancestor))
}

pub(crate) fn is_type_usable_as_property_name<'a>(
    checker: &Checker<'a, '_>,
    t: TypeHandle,
) -> bool {
    checker.is_type_usable_as_property_name(t)
}

/**
 * Gets the symbolic name for a member from its type.
 */
pub(crate) fn get_property_name_from_type<'a>(checker: &Checker<'a, '_>, t: TypeHandle) -> String {
    checker.get_property_name_from_type(t)
}

pub(crate) fn is_numeric_literal_name(name: &str) -> bool {
    // The intent of numeric names is that
    //     - they are names with text in a numeric form, and that
    //     - setting properties/indexing with them is always equivalent to doing so with the numeric literal 'numLit',
    //         acquired by applying the abstract 'ToNumber' operation on the name's text.
    //
    // The subtlety is in the latter portion, as we cannot reliably say that anything that looks like a numeric literal is a numeric name.
    // In fact, it is the case that the text of the name must be equal to 'ToString(numLit)' for this to hold.
    //
    // Consider the property name '"0xF00D"'. When one indexes with '0xF00D', they are actually indexing with the value of 'ToString(0xF00D)'
    // according to the ECMAScript specification, so it is actually as if the user indexed with the string '"61453"'.
    // Thus, the text of all numeric literals equivalent to '61543' such as '0xF00D', '0xf00D', '0170015', etc. are not valid numeric names
    // because their 'ToString' representation is not equal to their original text.
    // This is motivated by ECMA-262 sections 9.3.1, 9.8.1, 11.1.5, and 11.2.1.
    //
    // Here, we test whether 'ToString(ToNumber(name))' is exactly equal to 'name'.
    // The '+' prefix operator is equivalent here to applying the abstract ToNumber operation.
    // Applying the 'toString()' method on a number gives us the abstract ToString operation on a number.
    //
    // Note that this accepts the values 'Infinity', '-Infinity', and 'NaN', and that this is intentional.
    // This is desired behavior, because when indexing with them as numeric entities, you are indexing
    // with the strings '"Infinity"', '"-Infinity"', and '"NaN"' respectively.
    jsnum::from_string(name).to_string() == name
}

pub(crate) fn is_this_property(store: &ast::AstStore, node: ast::Node) -> bool {
    (ast::is_property_access_expression(store, node)
        || ast::is_element_access_expression(store, node))
        && store
            .expression(node_handle(node))
            .is_some_and(|expression| store.kind(expression) == ast::Kind::ThisKeyword)
}

pub(crate) fn is_valid_number_string(s: &str, round_trip_only: bool) -> bool {
    if s.is_empty() {
        return false;
    }
    let n = jsnum::from_string(s);
    !n.is_nan() && !n.is_inf() && (!round_trip_only || n.to_string() == s)
}

pub(crate) fn is_valid_big_int_string(s: &str, round_trip_only: bool) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut scanner = scanner::Scanner::new(String::new(), core::ScriptTarget::default());
    scanner.set_skip_trivia(false);
    let success = Arc::new(Mutex::new(true));
    let success_for_error = success.clone();
    scanner.set_on_error(Some(Arc::new(Mutex::new(
        move |_: &diagnostics::Message, _: usize, _: usize, _: &[String]| {
            *success_for_error
                .lock()
                .unwrap_or_else(|err| err.into_inner()) = false;
        },
    ))));
    scanner.set_text(Arc::from(format!("{s}n")));
    let mut result = scanner.scan();
    let negative = result == ast::Kind::MinusToken;
    if negative {
        result = scanner.scan();
    }
    let flags = scanner.token_flags();
    // validate that
    // * scanning proceeded without error
    // * a bigint can be scanned, and that when it is scanned, it is
    // * the full length of the input string (so the scanner is one character beyond the augmented input length)
    // * it does not contain a numeric separator (the `BigInt` constructor does not accept a numeric separator in its input)
    *success.lock().unwrap_or_else(|err| err.into_inner())
        && result == ast::Kind::BigIntLiteral
        && scanner.token_end() == (s.len() + 1) as i32
        && !flags.contains(ast::TokenFlags::CONTAINS_SEPARATOR)
        && (!round_trip_only
            || pseudo_big_int_to_string(jsnum::new_pseudo_big_int(
                &jsnum::parse_pseudo_big_int(scanner.token_value()),
                negative,
            )) == s)
}

pub(crate) fn is_valid_es_symbol_declaration(store: &ast::AstStore, node: ast::Node) -> bool {
    if ast::is_variable_declaration(store, node) {
        return ast::is_var_const(store, node)
            && store
                .name(node_handle(node))
                .is_some_and(|name| ast::is_identifier(store, name))
            && is_variable_declaration_in_variable_statement(store, node);
    }
    if ast::is_property_declaration(store, node) {
        return has_readonly_modifier(store, node) && ast::has_static_modifier(store, node);
    }
    ast::is_property_signature_declaration(store, node) && has_readonly_modifier(store, node)
}

pub(crate) fn is_variable_declaration_in_variable_statement(
    store: &ast::AstStore,
    node: ast::Node,
) -> bool {
    store
        .parent(node_handle(node))
        .as_ref()
        .is_some_and(|parent| {
            ast::is_variable_declaration_list(store, *parent)
                && store
                    .parent(node_handle(*parent))
                    .as_ref()
                    .is_some_and(|parent| ast::is_variable_statement(store, *parent))
        })
}

pub(crate) fn is_late_bound_name(name: &str) -> bool {
    name.strip_prefix(ast::INTERNAL_SYMBOL_NAME_PREFIX)
        .is_some_and(|name| name.starts_with('@'))
}

pub(crate) fn is_object_or_array_literal_type<'a>(
    checker: &Checker<'a, '_>,
    t: TypeHandle,
) -> bool {
    checker.object_flags(t) & (OBJECT_FLAGS_OBJECT_LITERAL | OBJECT_FLAGS_ARRAY_LITERAL) != 0
}

pub(crate) fn get_containing_class_excluding_class_decorators(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::ClassLikeDeclaration> {
    let parent = store.parent(node_handle(node));
    let decorator = ast::find_ancestor_or_quit(store, parent, |store, n| {
        if ast::is_class_like(store, n) {
            return ast::FindAncestorResult::Quit;
        }
        if ast::is_decorator(store, n) {
            return ast::FindAncestorResult::True;
        }
        ast::FindAncestorResult::False
    });
    let decorator = decorator;
    if let Some(decorator) = decorator {
        if let Some(parent) = store.parent(node_handle(decorator)) {
            if ast::is_class_like(store, parent) {
                return ast::get_containing_class(store, parent);
            }
        }
        return ast::get_containing_class(store, decorator);
    }
    ast::get_containing_class(store, node)
}

pub(crate) fn is_this_type_parameter<'a>(checker: &Checker<'a, '_>, t: TypeHandle) -> bool {
    checker.type_flags(t) & TYPE_FLAGS_TYPE_PARAMETER != 0
        && checker.type_record(t).as_type_parameter().is_this_type
}

pub(crate) fn is_class_instance_property(store: &ast::AstStore, node: ast::Node) -> bool {
    if ast::is_in_js_file(store, node) && ast::is_expando_property_declaration(store, Some(node)) {
        let left = store.left(node_handle(node)).unwrap();
        return (!ast::is_bindable_static_access_expression(
            store, left, false, /*excludeThisKeyword*/
        ) || !store
            .expression(node_handle(left))
            .is_some_and(|expression| ast::is_prototype_access(store, &expression)))
            && !ast::is_bindable_static_name_expression(
                store, left, true, /*excludeThisKeyword*/
            );
    }
    store.parent(node_handle(node)).is_some()
        && ast::is_class_like(store, store.parent(node_handle(node)).unwrap())
        && ast::is_property_declaration(store, node)
        && !ast::has_accessor_modifier(store, node)
}

pub(crate) fn is_this_initialized_object_binding_expression(
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    if !(ast::is_shorthand_property_assignment(store, node)
        || ast::is_property_assignment(store, node))
    {
        return false;
    }
    let Some(parent) = store.parent(node_handle(node)) else {
        return false;
    };
    let Some(grandparent) = store.parent(parent) else {
        return false;
    };
    ast::is_binary_expression(store, grandparent)
        && store
            .operator_token(grandparent)
            .is_some_and(|operator| store.kind(operator) == ast::Kind::EqualsToken)
        && store
            .right(grandparent)
            .is_some_and(|right| store.kind(right) == ast::Kind::ThisKeyword)
}

pub(crate) fn is_this_initialized_declaration(
    store: &ast::AstStore,
    node: Option<ast::Node>,
) -> bool {
    node.is_some()
        && ast::is_variable_declaration(store, node.unwrap())
        && store.initializer(node_handle(node.unwrap())).is_some()
        && store.kind(store.initializer(node_handle(node.unwrap())).unwrap())
            == ast::Kind::ThisKeyword
}

pub(crate) fn is_infinity_or_nan_string(name: &str) -> bool {
    name == "Infinity" || name == "-Infinity" || name == "NaN"
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn is_mutable_local_variable_declaration(&mut self, declaration: ast::Node) -> bool {
        let store = self.store_for_node(declaration);
        // Return true if symbol is a non-exported and non-global `let` variable
        store
            .parent(node_handle(declaration))
            .is_some_and(|parent| {
                store.flags(parent) & ast::NodeFlags::LET != 0
                    && !(ast::get_combined_modifier_flags(store, declaration)
                        .intersects(ast::ModifierFlags::Export)
                        || store.parent(parent).is_some_and(|parent_parent| {
                            store.kind(parent_parent) == ast::Kind::VariableStatement
                                && store.parent(parent_parent).is_some_and(|grand_parent| {
                                    store.kind(grand_parent) == ast::Kind::SourceFile
                                        && ast::is_global_source_file(
                                            &store.source_file_view(grand_parent),
                                        )
                                })
                        }))
            })
    }
}

pub(crate) fn is_in_ambient_or_type_node(store: &ast::AstStore, node: ast::Node) -> bool {
    store.flags(node_handle(node)) & ast::NodeFlags::Ambient != 0
        || ast::find_ancestor(store, Some(node), |store, n| {
            ast::is_interface_declaration(store, n)
                || ast::is_type_or_js_type_alias_declaration(store, &n)
                || ast::is_type_literal_node(store, n)
        })
        .is_some()
}

pub(crate) fn is_literal_expression_of_object(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node) {
        ast::Kind::ObjectLiteralExpression
        | ast::Kind::ArrayLiteralExpression
        | ast::Kind::RegularExpressionLiteral
        | ast::Kind::FunctionExpression
        | ast::Kind::ClassExpression => true,
        _ => false,
    }
}

pub(crate) fn can_have_flow_node(store: &ast::AstStore, node: ast::Node) -> bool {
    store.has_flow_node_base(node_handle(node))
}

pub(crate) fn is_non_null_access(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_access_expression(store, node)
        && store
            .expression(node_handle(node))
            .is_some_and(|expression| ast::is_non_null_expression(store, expression))
}

pub(crate) fn get_binding_element_property_name<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    store.property_name_or_name(node_handle(node))
}

pub(crate) fn is_call_chain(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_call_expression(store, node)
        && store.flags(node_handle(node)) & ast::NodeFlags::OptionalChain != 0
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn call_like_expression_may_have_type_arguments(&self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        ast::is_call_or_new_expression(store, node)
            || ast::is_tagged_template_expression(store, node)
            || ast::is_jsx_opening_like_element(store, node)
    }
}

pub(crate) fn is_super_call(store: &ast::AstStore, n: ast::Node) -> bool {
    ast::is_call_expression(store, n)
        && store
            .expression(node_handle(n))
            .is_some_and(|expression| store.kind(expression) == ast::Kind::SuperKeyword)
}

pub(crate) fn get_members_of_declaration<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Vec<ast::Node> {
    match store.kind(node) {
        ast::Kind::InterfaceDeclaration
        | ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::TypeLiteral => store
            .members(node_handle(node))
            .map(|members| members.iter().collect())
            .unwrap_or_default(),
        ast::Kind::ObjectLiteralExpression => store
            .properties(node_handle(node))
            .map(|properties| properties.iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub(crate) fn is_in_right_side_of_import_or_export_assignment(
    store: &ast::AstStore,
    mut node: ast::EntityName,
) -> bool {
    while store
        .parent(node_handle(node))
        .as_ref()
        .is_some_and(|parent| store.kind(*parent) == ast::Kind::QualifiedName)
    {
        node = store.parent(node_handle(node)).unwrap();
    }

    let Some(parent) = store.parent(node_handle(node)) else {
        return false;
    };
    store.kind(parent) == ast::Kind::ImportEqualsDeclaration
        && store
            .module_reference(parent)
            .as_ref()
            .is_some_and(|module_reference| node_matches_name(store, *module_reference, node))
        || store.kind(parent) == ast::Kind::ExportAssignment
            && store
                .expression(parent)
                .as_ref()
                .is_some_and(|expression| node_matches_name(store, *expression, node))
}

pub(crate) fn is_jsx_intrinsic_tag_name(store: &ast::AstStore, tag_name: ast::Node) -> bool {
    ast::is_identifier(store, tag_name)
        && scanner::is_intrinsic_jsx_name(&store.text(node_handle(tag_name)))
        || ast::is_jsx_namespaced_name(store, tag_name)
}

pub(crate) fn get_containing_object_literal<'a>(
    store: &'a ast::AstStore,
    f: &'a ast::SignatureDeclaration,
) -> Option<ast::Node> {
    if (store.kind(*f) == ast::Kind::MethodDeclaration
        || store.kind(*f) == ast::Kind::GetAccessor
        || store.kind(*f) == ast::Kind::SetAccessor)
        && store
            .parent(node_handle(*f))
            .is_some_and(|parent| store.kind(parent) == ast::Kind::ObjectLiteralExpression)
    {
        return store.parent(node_handle(*f));
    } else if store.kind(*f) == ast::Kind::FunctionExpression
        && store
            .parent(node_handle(*f))
            .is_some_and(|parent| store.kind(parent) == ast::Kind::PropertyAssignment)
    {
        return store
            .parent(node_handle(*f))
            .and_then(|parent| store.parent(parent));
    }
    None
}

pub(crate) fn is_import_type_qualifier_part<'a>(
    store: &'a ast::AstStore,
    mut node: ast::Node,
) -> Option<ast::Node> {
    let mut parent = store.parent(node_handle(node));
    while let Some(ref parent_node) = parent {
        if !ast::is_qualified_name(store, *parent_node) {
            break;
        }
        node = parent_node.clone();
        parent = store.parent(node_handle(*parent_node));
    }

    if parent.as_ref().is_some_and(|parent| {
        store.kind(*parent) == ast::Kind::ImportType
            && store
                .qualifier(node_handle(*parent))
                .as_ref()
                .is_some_and(|qualifier| node_matches_name(store, *qualifier, node))
    }) {
        return parent;
    }

    None
}

pub(crate) fn is_in_name_of_expression_with_type_arguments(
    store: &ast::AstStore,
    mut node: ast::Node,
) -> bool {
    while store
        .parent(node_handle(node))
        .as_ref()
        .is_some_and(|parent| store.kind(*parent) == ast::Kind::PropertyAccessExpression)
    {
        node = store.parent(node_handle(node)).unwrap();
    }

    store
        .parent(node_handle(node))
        .as_ref()
        .is_some_and(|parent| store.kind(*parent) == ast::Kind::ExpressionWithTypeArguments)
}

pub(crate) fn get_index_symbol_from_symbol_table<'a>(
    symbol_table: &'a ast::SymbolHandleTable,
) -> Option<SymbolIdentity> {
    symbol_table
        .get(ast::INTERNAL_SYMBOL_NAME_INDEX)
        .copied()
        .map(SymbolIdentity::from_symbol_handle)
}

// Indicates whether the result of an `Expression` will be unused.
// NOTE: This requires a node with a valid `parent` pointer.
pub(crate) fn expression_result_is_unused(store: &ast::AstStore, mut node: ast::Node) -> bool {
    loop {
        let parent = store.parent(node_handle(node)).unwrap();
        // walk up parenthesized expressions, but keep a pointer to the top-most parenthesized expression
        if ast::is_parenthesized_expression(store, parent) {
            node = parent;
            continue;
        }
        // result is unused in an expression statement, `void` expression, or the initializer or incrementer of a `for` loop
        if ast::is_expression_statement(store, parent)
            || ast::is_void_expression(store, parent)
            || ast::is_for_statement(store, parent)
                && (store
                    .initializer(parent)
                    .is_some_and(|initializer| node_matches_name(store, initializer, node))
                    || store
                        .incrementor(parent)
                        .is_some_and(|incrementor| node_matches_name(store, incrementor, node)))
        {
            return true;
        }
        if ast::is_binary_expression(store, parent)
            && store
                .operator_token(parent)
                .is_some_and(|operator| store.kind(operator) == ast::Kind::CommaToken)
        {
            // left side of comma is always unused
            if store
                .left(parent)
                .is_some_and(|left| node_matches_name(store, left, node))
            {
                return true;
            }
            // right side of comma is unused if parent is unused
            node = parent;
            continue;
        }
        return false;
    }
}

pub(crate) fn pseudo_big_int_to_string(value: jsnum::PseudoBigInt) -> String {
    value.to_string()
}

pub(crate) fn get_super_container<'a>(
    store: &'a ast::AstStore,
    mut node: ast::Node,
    stop_on_functions: bool,
) -> Option<ast::Node> {
    loop {
        node = store.parent(node_handle(node))?;
        match store.kind(node) {
            ast::Kind::ComputedPropertyName => {
                node = store.parent(node_handle(node))?;
            }
            ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction => {
                if !stop_on_functions {
                    continue;
                }
                return Some(node);
            }
            ast::Kind::PropertyDeclaration
            | ast::Kind::PropertySignature
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::Constructor
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::ClassStaticBlockDeclaration => {
                return Some(node);
            }
            ast::Kind::Decorator => {
                // Decorators are always applied outside of the body of a class or method.
                if ast::is_parameter_declaration(store, store.parent(node_handle(node)).unwrap())
                    && ast::is_class_element(
                        store,
                        store
                            .parent(store.parent(node_handle(node)).unwrap())
                            .unwrap(),
                    )
                {
                    // If the decorator's parent is a Parameter, we resolve the this container from
                    // the grandparent class declaration.
                    node = store
                        .parent(store.parent(node_handle(node)).unwrap())
                        .unwrap();
                } else if ast::is_class_element(store, store.parent(node_handle(node)).unwrap()) {
                    // If the decorator's parent is a class element, we resolve the 'this' container
                    // from the parent class declaration.
                    node = store.parent(node_handle(node)).unwrap();
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn for_each_yield_expression(
    store: &ast::AstStore,
    body: ast::Node,
    mut visitor: impl FnMut(ast::Node) -> bool,
) -> bool {
    fn traverse(
        store: &ast::AstStore,
        node: ast::Node,
        visitor: &mut dyn FnMut(ast::Node) -> bool,
    ) -> bool {
        match store.kind(node) {
            ast::Kind::YieldExpression => {
                if visitor(node) {
                    return true;
                }
                let operand = store.expression(node_handle(node));
                if operand.is_none() {
                    return false;
                }
                traverse(store, operand.unwrap(), visitor)
            }
            ast::Kind::EnumDeclaration
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::ModuleDeclaration
            | ast::Kind::TypeAliasDeclaration => {
                // These are not allowed inside a generator now, but eventually they may be allowed
                // as local types. Regardless, skip them to avoid the work.
                false
            }
            _ => {
                if ast::is_function_like(store, Some(node)) {
                    if store
                        .name(node_handle(node))
                        .is_some_and(|name| ast::is_computed_property_name(store, name))
                    {
                        // Note that we will not include methods/accessors of a class because they would require
                        // first descending into the class. This is by design.
                        let name = store.name(node_handle(node)).unwrap();
                        return traverse(
                            store,
                            store.expression(node_handle(name)).unwrap(),
                            visitor,
                        );
                    }
                } else if !ast::is_part_of_type_node(store, node) {
                    // This is the general case, which should include mostly expressions and statements.
                    // Also includes NodeArrays.
                    return store
                        .for_each_present_child(node_handle(node), |child| {
                            if traverse(store, child, visitor) {
                                ControlFlow::Break(())
                            } else {
                                ControlFlow::Continue(())
                            }
                        })
                        .is_break();
                }
                false
            }
        }
    }
    traverse(store, body, &mut visitor)
}

pub(crate) fn get_enclosing_container<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    let parent = store.parent(node_handle(node));
    ast::find_ancestor(store, parent, |store, n| {
        binder::get_container_flags(store, n) & binder::CONTAINER_FLAGS_IS_CONTAINER != 0
    })
}

pub(crate) fn has_type(store: &ast::AstStore, node: ast::Node) -> bool {
    store.type_node(node_handle(node)).is_some()
}

pub(crate) fn get_non_rest_parameter_count<'a>(
    checker: &Checker<'a, '_>,
    sig: SignatureHandle,
) -> usize {
    checker.signature_record(sig).parameters.len()
        - if checker.signature_has_rest_parameter(sig) {
            1
        } else {
            0
        }
}

pub(crate) fn min_and_max<T>(
    slice: &[T],
    mut get_value: impl FnMut(&T) -> isize,
) -> (isize, isize) {
    let mut min_value = 0;
    let mut max_value = 0;
    for (i, element) in slice.iter().enumerate() {
        let value = get_value(element);
        if i == 0 {
            min_value = value;
            max_value = value;
        } else {
            min_value = min_value.min(value);
            max_value = max_value.max(value);
        }
    }
    (min_value, max_value)
}

pub struct FeatureMapEntry {
    pub(crate) lib: &'static str,
    pub(crate) props: &'static [&'static str],
}

static FEATURE_MAP: OnceLock<HashMap<&'static str, Vec<FeatureMapEntry>>> = OnceLock::new();

pub fn get_feature_map() -> &'static HashMap<&'static str, Vec<FeatureMapEntry>> {
    FEATURE_MAP.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert(
            "Array",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &[
                        "find",
                        "findIndex",
                        "fill",
                        "copyWithin",
                        "entries",
                        "keys",
                        "values",
                    ],
                },
                FeatureMapEntry {
                    lib: "es2016",
                    props: &["includes"],
                },
                FeatureMapEntry {
                    lib: "es2019",
                    props: &["flat", "flatMap"],
                },
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Iterator",
            vec![FeatureMapEntry {
                lib: "es2015",
                props: &[],
            }],
        );
        map.insert(
            "AsyncIterator",
            vec![FeatureMapEntry {
                lib: "es2015",
                props: &[],
            }],
        );
        map.insert(
            "ArrayBuffer",
            vec![FeatureMapEntry {
                lib: "es2024",
                props: &[
                    "maxByteLength",
                    "resizable",
                    "resize",
                    "detached",
                    "transfer",
                    "transferToFixedLength",
                ],
            }],
        );
        map.insert(
            "Atomics",
            vec![
                FeatureMapEntry {
                    lib: "es2017",
                    props: &[
                        "add",
                        "and",
                        "compareExchange",
                        "exchange",
                        "isLockFree",
                        "load",
                        "or",
                        "store",
                        "sub",
                        "wait",
                        "notify",
                        "xor",
                    ],
                },
                FeatureMapEntry {
                    lib: "es2024",
                    props: &["waitAsync"],
                },
            ],
        );
        map.insert(
            "SharedArrayBuffer",
            vec![
                FeatureMapEntry {
                    lib: "es2017",
                    props: &["byteLength", "slice"],
                },
                FeatureMapEntry {
                    lib: "es2024",
                    props: &["growable", "maxByteLength", "grow"],
                },
            ],
        );
        map.insert(
            "AsyncIterable",
            vec![FeatureMapEntry {
                lib: "es2018",
                props: &[],
            }],
        );
        map.insert(
            "AsyncIterableIterator",
            vec![FeatureMapEntry {
                lib: "es2018",
                props: &[],
            }],
        );
        map.insert(
            "AsyncGenerator",
            vec![FeatureMapEntry {
                lib: "es2018",
                props: &[],
            }],
        );
        map.insert(
            "AsyncGeneratorFunction",
            vec![FeatureMapEntry {
                lib: "es2018",
                props: &[],
            }],
        );
        map.insert(
            "RegExp",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &["flags", "sticky", "unicode"],
                },
                FeatureMapEntry {
                    lib: "es2018",
                    props: &["dotAll"],
                },
                FeatureMapEntry {
                    lib: "es2024",
                    props: &["unicodeSets"],
                },
            ],
        );
        map.insert(
            "RegExpConstructor",
            vec![FeatureMapEntry {
                lib: "es2025",
                props: &["escape"],
            }],
        );
        map.insert(
            "Reflect",
            vec![FeatureMapEntry {
                lib: "es2015",
                props: &[
                    "apply",
                    "construct",
                    "defineProperty",
                    "deleteProperty",
                    "get",
                    "getOwnPropertyDescriptor",
                    "getPrototypeOf",
                    "has",
                    "isExtensible",
                    "ownKeys",
                    "preventExtensions",
                    "set",
                    "setPrototypeOf",
                ],
            }],
        );
        map.insert(
            "ArrayConstructor",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &["from", "of"],
                },
                FeatureMapEntry {
                    lib: "esnext",
                    props: &["fromAsync"],
                },
            ],
        );
        map.insert(
            "ObjectConstructor",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &[
                        "assign",
                        "getOwnPropertySymbols",
                        "keys",
                        "is",
                        "setPrototypeOf",
                    ],
                },
                FeatureMapEntry {
                    lib: "es2017",
                    props: &["values", "entries", "getOwnPropertyDescriptors"],
                },
                FeatureMapEntry {
                    lib: "es2019",
                    props: &["fromEntries"],
                },
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["hasOwn"],
                },
                FeatureMapEntry {
                    lib: "es2024",
                    props: &["groupBy"],
                },
            ],
        );
        map.insert(
            "NumberConstructor",
            vec![FeatureMapEntry {
                lib: "es2015",
                props: &[
                    "isFinite",
                    "isInteger",
                    "isNaN",
                    "isSafeInteger",
                    "parseFloat",
                    "parseInt",
                ],
            }],
        );
        map.insert(
            "Math",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &[
                        "clz32", "imul", "sign", "log10", "log2", "log1p", "expm1", "cosh", "sinh",
                        "tanh", "acosh", "asinh", "atanh", "hypot", "trunc", "fround", "cbrt",
                    ],
                },
                FeatureMapEntry {
                    lib: "es2025",
                    props: &["f16round"],
                },
            ],
        );
        map.insert(
            "Map",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &["entries", "keys", "values"],
                },
                FeatureMapEntry {
                    lib: "esnext",
                    props: &["getOrInsert", "getOrInsertComputed"],
                },
            ],
        );
        map.insert(
            "MapConstructor",
            vec![FeatureMapEntry {
                lib: "es2024",
                props: &["groupBy"],
            }],
        );
        map.insert(
            "Set",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &["entries", "keys", "values"],
                },
                FeatureMapEntry {
                    lib: "es2025",
                    props: &[
                        "union",
                        "intersection",
                        "difference",
                        "symmetricDifference",
                        "isSubsetOf",
                        "isSupersetOf",
                        "isDisjointFrom",
                    ],
                },
            ],
        );
        map.insert(
            "PromiseConstructor",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &["all", "race", "reject", "resolve"],
                },
                FeatureMapEntry {
                    lib: "es2020",
                    props: &["allSettled"],
                },
                FeatureMapEntry {
                    lib: "es2021",
                    props: &["any"],
                },
                FeatureMapEntry {
                    lib: "es2024",
                    props: &["withResolvers"],
                },
                FeatureMapEntry {
                    lib: "es2025",
                    props: &["try"],
                },
            ],
        );
        map.insert(
            "Symbol",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &["for", "keyFor"],
                },
                FeatureMapEntry {
                    lib: "es2019",
                    props: &["description"],
                },
            ],
        );
        map.insert(
            "WeakMap",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &[],
                },
                FeatureMapEntry {
                    lib: "esnext",
                    props: &["getOrInsert", "getOrInsertComputed"],
                },
            ],
        );
        map.insert(
            "WeakSet",
            vec![FeatureMapEntry {
                lib: "es2015",
                props: &[],
            }],
        );
        map.insert(
            "String",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &[
                        "codePointAt",
                        "includes",
                        "endsWith",
                        "normalize",
                        "repeat",
                        "startsWith",
                        "anchor",
                        "big",
                        "blink",
                        "bold",
                        "fixed",
                        "fontcolor",
                        "fontsize",
                        "italics",
                        "link",
                        "small",
                        "strike",
                        "sub",
                        "sup",
                    ],
                },
                FeatureMapEntry {
                    lib: "es2017",
                    props: &["padStart", "padEnd"],
                },
                FeatureMapEntry {
                    lib: "es2019",
                    props: &["trimStart", "trimEnd", "trimLeft", "trimRight"],
                },
                FeatureMapEntry {
                    lib: "es2020",
                    props: &["matchAll"],
                },
                FeatureMapEntry {
                    lib: "es2021",
                    props: &["replaceAll"],
                },
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2024",
                    props: &["isWellFormed", "toWellFormed"],
                },
            ],
        );
        map.insert(
            "StringConstructor",
            vec![FeatureMapEntry {
                lib: "es2015",
                props: &["fromCodePoint", "raw"],
            }],
        );
        map.insert(
            "DateTimeFormat",
            vec![FeatureMapEntry {
                lib: "es2017",
                props: &["formatToParts"],
            }],
        );
        map.insert(
            "Promise",
            vec![
                FeatureMapEntry {
                    lib: "es2015",
                    props: &[],
                },
                FeatureMapEntry {
                    lib: "es2018",
                    props: &["finally"],
                },
            ],
        );
        map.insert(
            "RegExpMatchArray",
            vec![FeatureMapEntry {
                lib: "es2018",
                props: &["groups"],
            }],
        );
        map.insert(
            "RegExpExecArray",
            vec![FeatureMapEntry {
                lib: "es2018",
                props: &["groups"],
            }],
        );
        map.insert(
            "Intl",
            vec![
                FeatureMapEntry {
                    lib: "es2018",
                    props: &["PluralRules"],
                },
                FeatureMapEntry {
                    lib: "es2020",
                    props: &["RelativeTimeFormat", "Locale", "DisplayNames"],
                },
                FeatureMapEntry {
                    lib: "es2021",
                    props: &["ListFormat", "DateTimeFormat"],
                },
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["Segmenter"],
                },
                FeatureMapEntry {
                    lib: "es2025",
                    props: &["DurationFormat"],
                },
            ],
        );
        map.insert(
            "NumberFormat",
            vec![FeatureMapEntry {
                lib: "es2018",
                props: &["formatToParts"],
            }],
        );
        map.insert(
            "SymbolConstructor",
            vec![
                FeatureMapEntry {
                    lib: "es2020",
                    props: &["matchAll"],
                },
                FeatureMapEntry {
                    lib: "esnext",
                    props: &["metadata", "dispose", "asyncDispose"],
                },
            ],
        );
        map.insert(
            "DataView",
            vec![
                FeatureMapEntry {
                    lib: "es2020",
                    props: &["setBigInt64", "setBigUint64", "getBigInt64", "getBigUint64"],
                },
                FeatureMapEntry {
                    lib: "es2025",
                    props: &["setFloat16", "getFloat16"],
                },
            ],
        );
        map.insert(
            "BigInt",
            vec![FeatureMapEntry {
                lib: "es2020",
                props: &[],
            }],
        );
        map.insert(
            "RelativeTimeFormat",
            vec![FeatureMapEntry {
                lib: "es2020",
                props: &["format", "formatToParts", "resolvedOptions"],
            }],
        );
        map.insert(
            "Int8Array",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Uint8Array",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Uint8ClampedArray",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Int16Array",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Uint16Array",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Int32Array",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Uint32Array",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Float16Array",
            vec![FeatureMapEntry {
                lib: "es2025",
                props: &[],
            }],
        );
        map.insert(
            "Float32Array",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Float64Array",
            vec![
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "BigInt64Array",
            vec![
                FeatureMapEntry {
                    lib: "es2020",
                    props: &[],
                },
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "BigUint64Array",
            vec![
                FeatureMapEntry {
                    lib: "es2020",
                    props: &[],
                },
                FeatureMapEntry {
                    lib: "es2022",
                    props: &["at"],
                },
                FeatureMapEntry {
                    lib: "es2023",
                    props: &[
                        "findLastIndex",
                        "findLast",
                        "toReversed",
                        "toSorted",
                        "toSpliced",
                        "with",
                    ],
                },
            ],
        );
        map.insert(
            "Error",
            vec![FeatureMapEntry {
                lib: "es2022",
                props: &["cause"],
            }],
        );
        map.insert(
            "ErrorConstructor",
            vec![FeatureMapEntry {
                lib: "esnext",
                props: &["isError"],
            }],
        );
        map.insert(
            "Uint8ArrayConstructor",
            vec![FeatureMapEntry {
                lib: "esnext",
                props: &["fromBase64", "fromHex"],
            }],
        );
        map.insert(
            "DisposableStack",
            vec![FeatureMapEntry {
                lib: "esnext",
                props: &[],
            }],
        );
        map.insert(
            "AsyncDisposableStack",
            vec![FeatureMapEntry {
                lib: "esnext",
                props: &[],
            }],
        );
        map.insert(
            "Date",
            vec![FeatureMapEntry {
                lib: "esnext",
                props: &["toTemporalInstant"],
            }],
        );
        map
    })
}

pub(crate) fn range_of_type_parameters(
    source_file: &ast::SourceFile,
    type_parameters: ast::SourceNodeList<'_>,
) -> core::TextRange {
    core::TextRange::new(
        type_parameters.pos() - 1,
        std::cmp::min(
            source_file.text().len(),
            scanner::skip_trivia(source_file.text(), type_parameters.end() as usize) + 1,
        ) as i32,
    )
}

pub(crate) fn try_get_property_access_or_identifier_to_string(
    store: &ast::AstStore,
    expr: ast::Node,
) -> String {
    if ast::is_property_access_expression(store, expr) {
        let base_str = store
            .expression(node_handle(expr))
            .map(|expression| try_get_property_access_or_identifier_to_string(store, expression))
            .unwrap_or_default();
        if !base_str.is_empty() {
            return base_str
                + "."
                + &entity_name_to_string(store, store.name(node_handle(expr)).unwrap());
        }
    } else if ast::is_element_access_expression(store, expr) {
        let base_str = store
            .expression(node_handle(expr))
            .map(|expression| try_get_property_access_or_identifier_to_string(store, expression))
            .unwrap_or_default();
        let argument_expression = store.argument_expression(node_handle(expr));
        if !base_str.is_empty()
            && argument_expression
                .as_ref()
                .is_some_and(|argument| ast::is_property_name(store, argument))
        {
            return base_str
                + "."
                + &ast::get_property_name_for_property_name_node(
                    store,
                    argument_expression.unwrap(),
                );
        }
    } else if ast::is_identifier(store, expr) {
        return store.text(node_handle(expr));
    } else if ast::is_jsx_namespaced_name(store, expr) {
        return entity_name_to_string(store, expr);
    }
    String::new()
}

pub(crate) fn contains_non_missing_undefined_type<'a>(c: &Checker<'a, '_>, t: TypeHandle) -> bool {
    let candidate = if c.type_flags(t) & TYPE_FLAGS_UNION != 0 {
        c.type_types_slice(t)[0]
    } else {
        t
    };
    c.type_flags(candidate) & TYPE_FLAGS_UNDEFINED != 0
        && candidate != c.semantic_state.semantic_handles().missing_type
}

pub(crate) fn get_any_import_syntax<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    match store.kind(node) {
        ast::Kind::ImportEqualsDeclaration => Some(node),
        ast::Kind::ImportClause => store.parent(node_handle(node)),
        ast::Kind::NamespaceImport => store
            .parent(node_handle(node))
            .and_then(|p| store.parent(p)),
        ast::Kind::ImportSpecifier => store
            .parent(node_handle(node))
            .and_then(|p| store.parent(p))
            .and_then(|p| store.parent(p)),
        _ => None,
    }
}

// A reserved member name consists of the byte 0xFE (which is an invalid UTF-8 encoding) followed by one or more
// characters where the first character is not '@' or '#'. The '@' character indicates that the name is denoted by
// a well known ES Symbol instance and the '#' character indicates that the name is a PrivateIdentifier.
pub(crate) fn is_reserved_member_name(name: &str) -> bool {
    name.len() >= 2
        && name.as_bytes()[0] == 0xfe
        && name.as_bytes()[1] != b'@'
        && name.as_bytes()[1] != b'#'
}

pub(crate) fn introduces_arguments_exotic_object(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node) {
        ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression => true,
        _ => false,
    }
}

pub(crate) fn symbols_to_array(symbols: &ast::SymbolHandleTable) -> Vec<SymbolIdentity> {
    let mut result = Vec::new();
    for (id, &symbol) in symbols {
        if !is_reserved_member_name(id) {
            result.push(SymbolIdentity::from_symbol_handle(symbol));
        }
    }
    result
}

// True if the symbol is for an external module, as opposed to a namespace.
pub fn is_external_module_symbol(symbol_name: &str, symbol_flags: ast::SymbolFlags) -> bool {
    symbol_flags.intersects(ast::SYMBOL_FLAGS_MODULE) && symbol_name.starts_with('"')
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn is_canceled(&self) -> bool {
        self.context().err().is_some()
    }

    pub(crate) fn check_not_canceled(&self) {
        if self.was_canceled() {
            panic!("Checker was previously cancelled");
        }
    }

    fn ensure_packages_map(&mut self) {
        if self.packages_map_is_empty() {
            let resolved_modules = self.program.get_resolved_modules();
            for (_, resolved_modules_in_file) in resolved_modules {
                for (_, module) in resolved_modules_in_file {
                    if !module.package_id.name.is_empty() {
                        let current = self
                            .package_map_entry(&module.package_id.name)
                            .unwrap_or(false);
                        self.record_package_map_entry(
                            module.package_id.name.clone(),
                            current || module.extension == tspath::Extension::Dts,
                        );
                    }
                }
            }
        }
    }

    fn types_package_exists(&mut self, package_name: &str) -> bool {
        self.ensure_packages_map();
        self.package_map_contains(&module::get_types_package_name(package_name))
    }

    fn package_bundles_types(&mut self, package_name: &str) -> bool {
        self.ensure_packages_map();
        self.package_map_entry(package_name).unwrap_or(false)
    }
}

pub fn value_to_string(value: &Any) -> String {
    match value {
        LiteralValue::String(value) => {
            format!(
                "\"{}\"",
                printer::escape_string(value.clone(), printer::QuoteChar::DoubleQuote)
            )
        }
        LiteralValue::Number(value) => value.to_string(),
        LiteralValue::Bool(value) => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        LiteralValue::BigInt(value) | LiteralValue::PseudoBigInt(value) => value.to_string() + "n",
        _ => panic!("unhandled value type in valueToString"),
    }
}

pub(crate) fn node_starts_new_lexical_environment(store: &ast::AstStore, node: ast::Node) -> bool {
    match store.kind(node) {
        ast::Kind::Constructor
        | ast::Kind::FunctionExpression
        | ast::Kind::FunctionDeclaration
        | ast::Kind::ArrowFunction
        | ast::Kind::MethodDeclaration
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::ModuleDeclaration
        | ast::Kind::SourceFile => true,
        _ => false,
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    // Returns if a type is or consists of a JSLiteral object type
    // In addition to objects which are directly literals,
    // * unions where every element is a jsliteral
    // * intersections where at least one element is a jsliteral
    // * and instantiable types constrained to a jsliteral
    // Should all count as literals and not print errors on access or assignment of possibly existing properties.
    // This mirrors the behavior of the index signature propagation, to which this behaves similarly (but doesn't affect assignability or inference).
    pub(crate) fn is_js_literal_type(&mut self, t: TypeHandle) -> bool {
        if self.no_implicit_any() {
            return false;
            // Flag is meaningless under `noImplicitAny` mode
        }
        if self.object_flags(t) & OBJECT_FLAGS_JS_LITERAL != 0 {
            return true;
        }
        if self.type_flags(t) & TYPE_FLAGS_UNION != 0 {
            return self
                .type_types(t)
                .into_iter()
                .all(|t| self.is_js_literal_type(t));
        }
        if self.type_flags(t) & TYPE_FLAGS_INTERSECTION != 0 {
            return self
                .type_types(t)
                .into_iter()
                .any(|t| self.is_js_literal_type(t));
        }
        if self.type_flags(t) & TYPE_FLAGS_INSTANTIABLE != 0 {
            let constraint = self.get_resolved_base_constraint(t, Vec::new());
            return constraint != t && self.is_js_literal_type(constraint);
        }
        false
    }
}

// DiagnosticDetails holds a resolved diagnostic message and its arguments,
// used for sharing diagnostic chain computation between the checker and incremental builder.
pub struct DiagnosticDetails {
    pub message: &'static diagnostics::Message,
    pub args: Vec<Any>,
}

// CreateModuleNotFoundChain computes the diagnostic message and arguments for a module-not-found
// error chain entry. This is shared between the checker (initial diagnostic creation) and the
// incremental builder (repopulation of cached diagnostics).
// Mirrors createModuleNotFoundChain in the TypeScript compiler's utilities.ts.
pub fn create_module_not_found_chain(
    program: &dyn Program,
    file: &ast::SourceFile,
    module_reference: &str,
    mode: core::ResolutionMode,
    mut package_name: String,
) -> DiagnosticDetails {
    if let Some(resolved_module) = program.get_resolved_module(file, module_reference, mode)
        && !resolved_module.alternate_result.is_empty()
    {
        if resolved_module
            .alternate_result
            .contains("/node_modules/@types/")
        {
            package_name =
                "@types/".to_string() + &module::mangle_scoped_package_name(&package_name);
        }
        return DiagnosticDetails {
            message: &diagnostics::THERE_ARE_TYPES_AT_0_BUT_THIS_RESULT_COULD_NOT_BE_RESOLVED_WHEN_RESPECTING_PACKAGE_JSON_EXPORTS_THE_1_LIBRARY_MAY_NEED_TO_UPDATE_ITS_PACKAGE_JSON_OR_TYPINGS,
            args: vec![resolved_module.alternate_result.clone().into(), package_name.into()],
        };
    }

    let packages_map = program.get_packages_map();
    if packages_map.contains_key(&module::get_types_package_name(&package_name)) {
        return DiagnosticDetails {
            message: &diagnostics::IF_THE_0_PACKAGE_ACTUALLY_EXPOSES_THIS_MODULE_CONSIDER_SENDING_A_PULL_REQUEST_TO_AMEND_HTTPS_COLON_SLASH_SLASHGITHUB_COM_SLASHDEFINITELY_TYPED_SLASHDEFINITELY_TYPED_SLASH_TREE_SLASH_MASTER_SLASH_TYPES_SLASH_1,
            args: vec![package_name.clone().into(), module::mangle_scoped_package_name(&package_name).into()],
        };
    }
    if *packages_map.get(&package_name).unwrap_or(&false) {
        return DiagnosticDetails {
            message: &diagnostics::IF_THE_0_PACKAGE_ACTUALLY_EXPOSES_THIS_MODULE_TRY_ADDING_A_NEW_DECLARATION_D_TS_FILE_CONTAINING_DECLARE_MODULE_1,
            args: vec![package_name.into(), module_reference.to_string().into()],
        };
    }
    DiagnosticDetails {
        message: &diagnostics::TRY_NPM_I_SAVE_DEV_TYPES_SLASH_1_IF_IT_EXISTS_OR_ADD_A_NEW_DECLARATION_D_TS_FILE_CONTAINING_DECLARE_MODULE_0,
        args: vec![module_reference.to_string().into(), module::mangle_scoped_package_name(&package_name).into()],
    }
}

// CreateModeMismatchDetails computes the diagnostic message and arguments for a mode-mismatch
// error chain entry. This is shared between the checker (initial diagnostic creation) and the
// incremental builder (repopulation of cached diagnostics).
// Mirrors createModeMismatchDetails in the TypeScript compiler's utilities.ts.
pub fn create_mode_mismatch_details(
    program: &dyn Program,
    file: &ast::SourceFile,
) -> DiagnosticDetails {
    let ext = tspath::try_get_extension_from_path(&file.file_name());
    let target_ext = if ext == tspath::Extension::Ts {
        tspath::Extension::Mts
    } else if ext == tspath::Extension::Js {
        tspath::Extension::Mjs
    } else {
        tspath::Extension::None
    };
    let meta = program.get_source_file_meta_data(file.path());
    let package_json_type = meta.package_json_type;
    let package_json_directory = meta.package_json_directory;

    if !package_json_directory.is_empty() && package_json_type.is_empty() {
        if target_ext != tspath::Extension::None {
            return DiagnosticDetails {
                message: &diagnostics::TO_CONVERT_THIS_FILE_TO_AN_ECMA_SCRIPT_MODULE_CHANGE_ITS_FILE_EXTENSION_TO_0_OR_ADD_THE_FIELD_TYPE_COLON_MODULE_TO_1,
                args: vec![target_ext.to_string().into(), tspath::combine_paths(&package_json_directory, &["package.json"]).into()],
            };
        }
        return DiagnosticDetails {
                message: &diagnostics::TO_CONVERT_THIS_FILE_TO_AN_ECMA_SCRIPT_MODULE_ADD_THE_FIELD_TYPE_COLON_MODULE_TO_0,
            args: vec![tspath::combine_paths(&package_json_directory, &["package.json"]).into()],
        };
    }
    if target_ext != tspath::Extension::None {
        return DiagnosticDetails {
            message: &diagnostics::TO_CONVERT_THIS_FILE_TO_AN_ECMA_SCRIPT_MODULE_CHANGE_ITS_FILE_EXTENSION_TO_0_OR_CREATE_A_LOCAL_PACKAGE_JSON_FILE_WITH_TYPE_COLON_MODULE,
            args: vec![target_ext.to_string().into()],
        };
    }
    DiagnosticDetails {
        message: &diagnostics::TO_CONVERT_THIS_FILE_TO_AN_ECMA_SCRIPT_MODULE_CREATE_A_LOCAL_PACKAGE_JSON_FILE_WITH_TYPE_COLON_MODULE,
        args: Vec::new(),
    }
}

pub(crate) fn walk_up_outer_expressions(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    let mut parent = store.parent(node_handle(node));
    while parent
        .as_ref()
        .is_some_and(|parent| ast::is_outer_expression(store, *parent, ast::OEK_ALL))
    {
        parent = store.parent(parent.unwrap());
    }
    parent
}

pub fn get_set_accessor_value_parameter<'a>(
    store: &'a ast::AstStore,
    accessor: ast::Node,
) -> Option<ast::Node> {
    let parameters = store.parameters(node_handle(accessor));
    if let Some(parameters) = parameters
        && !parameters.is_empty()
    {
        let has_this =
            parameters.len() == 2 && ast::is_this_parameter(store, parameters.first().unwrap());
        return parameters.iter().nth(if has_this { 1 } else { 0 });
    }
    None
}
