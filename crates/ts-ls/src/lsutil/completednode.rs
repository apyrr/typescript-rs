use ts_ast as ast;
use ts_astnav as astnav;
use ts_scanner as scanner;

use super::get_last_visited_child;

// PositionBelongsToNode returns true if the position belongs to the node.
// Assumes `candidate.pos() <= position` holds.
pub(crate) fn position_belongs_to_node(
    candidate: ast::Node,
    position: i32,
    file: &ast::SourceFile,
) -> bool {
    let loc = file.store().loc(candidate);
    if loc.pos() > position {
        panic!("Expected candidate.pos <= position");
    }
    position < loc.end() || !is_completed_node(Some(candidate), file)
}

pub(crate) fn is_completed_node(n: Option<ast::Node>, source_file: &ast::SourceFile) -> bool {
    let Some(n) = n else {
        return false;
    };
    let store = source_file.store();
    if ast::node_is_missing(store, Some(n)) {
        return false;
    }

    match store.kind(n) {
        ast::Kind::ClassDeclaration
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::ObjectLiteralExpression
        | ast::Kind::ObjectBindingPattern
        | ast::Kind::TypeLiteral
        | ast::Kind::Block
        | ast::Kind::ModuleBlock
        | ast::Kind::CaseBlock
        | ast::Kind::NamedImports
        | ast::Kind::NamedExports => node_ends_with(n, ast::Kind::CloseBraceToken, source_file),

        ast::Kind::CatchClause => is_completed_node(store.block(n), source_file),

        ast::Kind::NewExpression => {
            if store.arguments(n).is_none() {
                true
            } else {
                node_ends_with(n, ast::Kind::CloseParenToken, source_file)
            }
        }

        ast::Kind::CallExpression
        | ast::Kind::ParenthesizedExpression
        | ast::Kind::ParenthesizedType => {
            node_ends_with(n, ast::Kind::CloseParenToken, source_file)
        }

        ast::Kind::FunctionType | ast::Kind::ConstructorType => {
            is_completed_node(store.r#type(n), source_file)
        }

        ast::Kind::Constructor
        | ast::Kind::GetAccessor
        | ast::Kind::SetAccessor
        | ast::Kind::FunctionDeclaration
        | ast::Kind::FunctionExpression
        | ast::Kind::MethodDeclaration
        | ast::Kind::MethodSignature
        | ast::Kind::ConstructSignature
        | ast::Kind::CallSignature
        | ast::Kind::ArrowFunction => {
            if store.body(n).is_some() {
                return is_completed_node(store.body(n), source_file);
            }
            if store.r#type(n).is_some() {
                return is_completed_node(store.r#type(n), source_file);
            }
            // Even though type parameters can be unclosed, we can get away with
            // having at least a closing paren.
            has_child_of_kind(n, ast::Kind::CloseParenToken, source_file)
        }

        ast::Kind::ModuleDeclaration => {
            store.body(n).is_some() && is_completed_node(store.body(n), source_file)
        }

        ast::Kind::IfStatement => {
            if store.else_statement(n).is_some() {
                return is_completed_node(store.else_statement(n), source_file);
            }
            is_completed_node(store.then_statement(n), source_file)
        }

        ast::Kind::ExpressionStatement => {
            is_completed_node(store.expression(n), source_file)
                || has_child_of_kind(n, ast::Kind::SemicolonToken, source_file)
        }

        ast::Kind::ArrayLiteralExpression
        | ast::Kind::ArrayBindingPattern
        | ast::Kind::ElementAccessExpression
        | ast::Kind::ComputedPropertyName
        | ast::Kind::TupleType => node_ends_with(n, ast::Kind::CloseBracketToken, source_file),

        ast::Kind::IndexSignature => {
            if store.r#type(n).is_some() {
                return is_completed_node(store.r#type(n), source_file);
            }
            has_child_of_kind(n, ast::Kind::CloseBracketToken, source_file)
        }

        ast::Kind::CaseClause | ast::Kind::DefaultClause => {
            // there is no such thing as terminator token for CaseClause/DefaultClause so for simplicity always consider them non-completed
            false
        }

        ast::Kind::ForStatement
        | ast::Kind::ForInStatement
        | ast::Kind::ForOfStatement
        | ast::Kind::WhileStatement => is_completed_node(store.statement(n), source_file),

        ast::Kind::DoStatement => {
            // rough approximation: if DoStatement has While keyword - then if node is completed is checking the presence of ')';
            if has_child_of_kind(n, ast::Kind::WhileKeyword, source_file) {
                return node_ends_with(n, ast::Kind::CloseParenToken, source_file);
            }
            is_completed_node(store.statement(n), source_file)
        }

        ast::Kind::TypeQuery => is_completed_node(store.expr_name(n), source_file),

        ast::Kind::TypeOfExpression
        | ast::Kind::DeleteExpression
        | ast::Kind::VoidExpression
        | ast::Kind::YieldExpression
        | ast::Kind::SpreadElement => is_completed_node(store.expression(n), source_file),

        ast::Kind::TaggedTemplateExpression => is_completed_node(store.template(n), source_file),

        ast::Kind::TemplateExpression => {
            if store.template_spans(n).is_none() {
                return false;
            }
            let span = store.template_spans(n).unwrap().last();
            is_completed_node(span, source_file)
        }

        ast::Kind::TemplateSpan => ast::node_is_present(store, store.literal(n)),

        ast::Kind::ExportDeclaration | ast::Kind::ImportDeclaration => {
            ast::node_is_present(store, store.module_specifier(n))
        }

        ast::Kind::PrefixUnaryExpression => is_completed_node(store.operand(n), source_file),

        ast::Kind::BinaryExpression => is_completed_node(store.right(n), source_file),

        ast::Kind::ConditionalExpression => is_completed_node(store.when_false(n), source_file),

        _ => true,
    }
}

// Checks if node ends with `expected_last_token`.
// If child at position `length - 1` is `SemicolonToken` it is skipped and `expected_last_token` is compared with child at position `length - 2`.
fn node_ends_with(
    n: ast::Node,
    expected_last_token: ast::Kind,
    source_file: &ast::SourceFile,
) -> bool {
    let last_child_node = get_last_visited_child(n, source_file);
    let mut last_node_and_tokens = Vec::new();
    let token_start_pos = if let Some(last_child_node) = last_child_node {
        last_node_and_tokens.push(source_file.store().kind(last_child_node));
        source_file.store().loc(last_child_node).end()
    } else {
        source_file.store().loc(n).pos()
    };

    let mut scan = scanner::get_scanner_for_source_file(source_file, token_start_pos as usize);
    let mut start_pos = token_start_pos;
    while start_pos < source_file.store().loc(n).end() {
        let token_kind = scan.token();
        let token_end = scan.token_end();
        last_node_and_tokens.push(token_kind);
        start_pos = token_end;
        scan.scan();
    }

    if last_node_and_tokens.is_empty() {
        return false;
    }

    let last_child = last_node_and_tokens[last_node_and_tokens.len() - 1];
    if last_child == expected_last_token {
        true
    } else if last_child == ast::Kind::SemicolonToken && last_node_and_tokens.len() > 1 {
        last_node_and_tokens[last_node_and_tokens.len() - 2] == expected_last_token
    } else {
        false
    }
}

fn has_child_of_kind(
    containing_node: ast::Node,
    kind: ast::Kind,
    source_file: &ast::SourceFile,
) -> bool {
    astnav::has_child_of_kind(containing_node, kind, source_file)
}
