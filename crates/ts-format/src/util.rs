use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_scanner as scanner;

pub fn range_is_on_one_line(node: core::TextRange, file: &ast::SourceFile) -> bool {
    let start_line = scanner::get_ecma_line_of_position(file, node.pos());
    let end_line = scanner::get_ecma_line_of_position(file, node.end());
    start_line == end_line
}

pub fn get_open_token_for_list(
    store: &ast::AstStore,
    node: &ast::Node,
    list: ast::SourceNodeList<'_>,
) -> ast::Kind {
    get_open_token_for_list_worker(store, node, |candidate| candidate.same_list(list))
}

pub fn get_open_token_for_list_input(
    store: &ast::AstStore,
    node: &ast::Node,
    list: &ast::SourceNodeListInput,
) -> ast::Kind {
    get_open_token_for_list_worker(store, node, |candidate| {
        candidate.source_ref() == list.source_ref()
    })
}

fn get_open_token_for_list_worker(
    store: &ast::AstStore,
    node: &ast::Node,
    mut is_target_list: impl FnMut(ast::SourceNodeList<'_>) -> bool,
) -> ast::Kind {
    match store.kind(*node) {
        ast::Kind::Constructor
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::ArrowFunction
        | ast::Kind::CallSignature
        | ast::Kind::ConstructSignature
        | ast::Kind::FunctionType
        | ast::Kind::ConstructorType
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor => {
            if store
                .type_parameters(*node)
                .is_some_and(&mut is_target_list)
            {
                return ast::Kind::LessThanToken;
            } else if store.parameters(*node).is_some_and(&mut is_target_list) {
                return ast::Kind::OpenParenToken;
            }
        }
        ast::Kind::CallExpression | ast::Kind::NewExpression => {
            if store.type_arguments(*node).is_some_and(&mut is_target_list) {
                return ast::Kind::LessThanToken;
            } else if store.arguments(*node).is_some_and(&mut is_target_list) {
                return ast::Kind::OpenParenToken;
            }
        }
        ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::TypeAliasDeclaration => {
            if store
                .type_parameters(*node)
                .is_some_and(&mut is_target_list)
            {
                return ast::Kind::LessThanToken;
            }
        }
        ast::Kind::TypeReference
        | ast::Kind::TaggedTemplateExpression
        | ast::Kind::TypeQuery
        | ast::Kind::ExpressionWithTypeArguments
        | ast::Kind::ImportType => {
            if store.type_arguments(*node).is_some_and(&mut is_target_list) {
                return ast::Kind::LessThanToken;
            }
        }
        ast::Kind::TypeLiteral => return ast::Kind::OpenBraceToken,
        _ => {}
    }

    ast::Kind::Unknown
}

pub fn get_close_token_for_open_token(kind: ast::Kind) -> ast::Kind {
    // TODO: matches strada - seems like it could handle more pairs of braces, though? [] notably missing
    match kind {
        ast::Kind::OpenParenToken => ast::Kind::CloseParenToken,
        ast::Kind::LessThanToken => ast::Kind::GreaterThanToken,
        ast::Kind::OpenBraceToken => ast::Kind::CloseBraceToken,
        _ => ast::Kind::Unknown,
    }
}

pub fn get_line_start_position_for_position(position: i32, source_file: &ast::SourceFile) -> i32 {
    let line_starts = scanner::get_ecma_line_starts(source_file);
    let line = scanner::get_ecma_line_of_position(source_file, position);
    line_starts[line as usize] as i32
}

/**
 * Tests whether `child` is a grammar error on `parent`.
 * In strada, this also checked node arrays, but it is never actually called with one in practice.
 */
pub fn is_grammar_error(store: &ast::AstStore, parent: &ast::Node, child: &ast::Node) -> bool {
    if ast::is_type_parameter_declaration(store, *parent) {
        return store.expression(*parent) == Some(*child);
    }
    if ast::is_property_signature_declaration(store, *parent) {
        return store.initializer(*parent) == Some(*child);
    }
    if ast::is_property_declaration(store, *parent) {
        return ast::is_auto_accessor_property_declaration(store, *parent)
            && store.postfix_token(*parent) == Some(*child)
            && store.kind(*child) == ast::Kind::QuestionToken;
    }
    if ast::is_property_assignment(store, *parent) {
        let mods = store.modifiers(*parent);
        return store.postfix_token(*parent) == Some(*child)
            || mods.is_some_and(|mods| {
                is_grammar_error_element(store, mods.nodes(), *child, ast::is_modifier_like)
            });
    }
    if ast::is_shorthand_property_assignment(store, *parent) {
        let mods = store.modifiers(*parent);
        return store.equals_token(*parent) == Some(*child)
            || store.postfix_token(*parent) == Some(*child)
            || mods.is_some_and(|mods| {
                is_grammar_error_element(store, mods.nodes(), *child, ast::is_modifier_like)
            });
    }
    if ast::is_method_declaration(store, *parent) {
        return store.postfix_token(*parent) == Some(*child)
            && store.kind(*child) == ast::Kind::ExclamationToken;
    }
    if ast::is_constructor_declaration(store, *parent) {
        return store.r#type(*parent) == Some(*child)
            || store.type_parameters(*parent).is_some_and(|list| {
                is_grammar_error_element(store, list, *child, ast::is_type_parameter_declaration)
            });
    }
    if ast::is_get_accessor_declaration(store, *parent) {
        return store.type_parameters(*parent).is_some_and(|list| {
            is_grammar_error_element(store, list, *child, ast::is_type_parameter_declaration)
        });
    }
    if ast::is_set_accessor_declaration(store, *parent) {
        return store.r#type(*parent) == Some(*child)
            || store.type_parameters(*parent).is_some_and(|list| {
                is_grammar_error_element(store, list, *child, ast::is_type_parameter_declaration)
            });
    }
    if ast::is_namespace_export_declaration(store, *parent) {
        let mods = store.modifiers(*parent);
        return mods.is_some_and(|mods| {
            is_grammar_error_element(store, mods.nodes(), *child, ast::is_modifier_like)
        });
    }
    false
}

pub fn is_grammar_error_element(
    store: &ast::AstStore,
    list: ast::SourceNodeList<'_>,
    child: ast::Node,
    is_possible_element: fn(&ast::AstStore, ast::Node) -> bool,
) -> bool {
    if list.is_empty() {
        return false;
    }
    if !is_possible_element(store, child) {
        return false;
    }
    list.iter().any(|node| node == child)
}

/**
 * Validating `expectedTokenKind` ensures the token was typed in the context we expect (eg: not a comment).
 * @param expectedTokenKind The kind of the last token constituting the desired parent node.
 */
pub fn find_immediately_preceding_token_of_kind(
    end: i32,
    expected_token_kind: ast::Kind,
    source_file: &ast::SourceFile,
) -> Option<astnav::TokenInfo> {
    let preceding_token = astnav::find_preceding_token_info(source_file, end)?;
    if preceding_token.kind != expected_token_kind || preceding_token.loc.end() != end {
        return None;
    }
    Some(preceding_token)
}

/**
 * Finds the highest node enclosing `node` at the same list level as `node`
 * and whose end does not exceed `node.end`.
 *
 * Consider typing the following
 * ```text
 * let x = 1;
 * while (true) {
 * }
 * ```
 * Upon typing the closing curly, we want to format the entire `while`-statement, but not the preceding
 * variable declaration.
 */
pub fn find_outermost_node_within_list_level(store: &ast::AstStore, node: &ast::Node) -> ast::Node {
    let mut current = *node;
    loop {
        let Some(parent) = store.parent(current) else {
            break;
        };
        if store.loc(parent).end() != store.loc(*node).end()
            || is_list_element(store, &parent, &current)
        {
            break;
        }
        current = parent;
    }

    current
}

// Returns true if node is a element in some list in parent
// i.e. parent is class declaration with the list of members and node is one of members.
pub fn is_list_element(store: &ast::AstStore, parent: &ast::Node, node: &ast::Node) -> bool {
    let node_loc = store.loc(*node);
    match store.kind(*parent) {
        ast::Kind::ClassDeclaration | ast::Kind::InterfaceDeclaration => {
            node_loc.contained_by(store.members(*parent).unwrap().loc())
        }
        ast::Kind::ModuleDeclaration => {
            let body = store.body(*parent);
            body.is_some_and(|body| {
                store.kind(body) == ast::Kind::ModuleBlock
                    && node_loc.contained_by(store.statements(body).unwrap().loc())
            })
        }
        ast::Kind::SourceFile => {
            node_loc.contained_by(store.parser_access().source_file_statement_loc(*parent))
        }
        ast::Kind::Block | ast::Kind::ModuleBlock => {
            node_loc.contained_by(store.statements(*parent).unwrap().loc())
        }
        ast::Kind::CatchClause => store
            .block(*parent)
            .is_some_and(|block| node_loc.contained_by(store.statements(block).unwrap().loc())),
        _ => false,
    }
}
