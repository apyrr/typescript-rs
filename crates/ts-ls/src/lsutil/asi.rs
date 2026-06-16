use ts_ast as ast;
use ts_astnav as astnav;
use ts_scanner as scanner;

use super::{get_last_child, get_last_token_info};

pub(crate) fn position_is_asi_candidate(
    pos: i32,
    context: ast::Node,
    file: &ast::SourceFile,
) -> bool {
    let store = file.store();
    let mut current = Some(context);
    let mut context_ancestor = None;
    while let Some(ancestor) = current {
        if store.loc(ancestor).end() != pos {
            break;
        }
        if syntax_may_be_asi_candidate(store.kind(ancestor)) {
            context_ancestor = Some(ancestor);
            break;
        }
        current = store.parent(ancestor);
    }

    context_ancestor.is_some_and(|ancestor| node_is_asi_candidate(ancestor, file))
}

pub(crate) fn syntax_may_be_asi_candidate(kind: ast::Kind) -> bool {
    syntax_requires_trailing_comma_or_semicolon_or_asi(kind)
        || syntax_requires_trailing_function_block_or_semicolon_or_asi(kind)
        || syntax_requires_trailing_module_block_or_semicolon_or_asi(kind)
        || syntax_requires_trailing_semicolon_or_asi(kind)
}

pub(crate) fn syntax_requires_trailing_comma_or_semicolon_or_asi(kind: ast::Kind) -> bool {
    kind == ast::Kind::CallSignature
        || kind == ast::Kind::ConstructSignature
        || kind == ast::Kind::IndexSignature
        || kind == ast::Kind::PropertySignature
        || kind == ast::Kind::MethodSignature
}

pub(crate) fn syntax_requires_trailing_function_block_or_semicolon_or_asi(kind: ast::Kind) -> bool {
    kind == ast::Kind::FunctionDeclaration
        || kind == ast::Kind::Constructor
        || kind == ast::Kind::MethodDeclaration
        || kind == ast::Kind::GetAccessor
        || kind == ast::Kind::SetAccessor
}

pub(crate) fn syntax_requires_trailing_module_block_or_semicolon_or_asi(kind: ast::Kind) -> bool {
    kind == ast::Kind::ModuleDeclaration
}

pub(crate) fn syntax_requires_trailing_semicolon_or_asi(kind: ast::Kind) -> bool {
    kind == ast::Kind::VariableStatement
        || kind == ast::Kind::ExpressionStatement
        || kind == ast::Kind::DoStatement
        || kind == ast::Kind::ContinueStatement
        || kind == ast::Kind::BreakStatement
        || kind == ast::Kind::ReturnStatement
        || kind == ast::Kind::ThrowStatement
        || kind == ast::Kind::DebuggerStatement
        || kind == ast::Kind::PropertyDeclaration
        || kind == ast::Kind::TypeAliasDeclaration
        || kind == ast::Kind::ImportDeclaration
        || kind == ast::Kind::ImportEqualsDeclaration
        || kind == ast::Kind::ExportDeclaration
        || kind == ast::Kind::NamespaceExportDeclaration
        || kind == ast::Kind::ExportAssignment
}

pub(crate) fn node_is_asi_candidate(node: ast::Node, file: &ast::SourceFile) -> bool {
    let store = file.store();
    let last_token = get_last_token_info(Some(node), file);
    if last_token.is_some_and(|token| token.kind == ast::Kind::SemicolonToken) {
        return false;
    }

    let kind = store.kind(node);
    if syntax_requires_trailing_comma_or_semicolon_or_asi(kind) {
        if last_token.is_some_and(|token| token.kind == ast::Kind::CommaToken) {
            return false;
        }
    } else if syntax_requires_trailing_module_block_or_semicolon_or_asi(kind) {
        let last_child = get_last_child(node, file);
        if last_child.is_some_and(|last_child| ast::is_module_block(store, last_child)) {
            return false;
        }
    } else if syntax_requires_trailing_function_block_or_semicolon_or_asi(kind) {
        let last_child = get_last_child(node, file);
        if last_child.is_some_and(|last_child| ast::is_function_block(store, Some(last_child))) {
            return false;
        }
    } else if !syntax_requires_trailing_semicolon_or_asi(kind) {
        return false;
    }

    // See comment in parser's `parseDoStatement`.
    if kind == ast::Kind::DoStatement {
        return true;
    }

    let mut top_node = node;
    while let Some(parent) = store.parent(top_node) {
        top_node = parent;
    }

    let next_token = astnav::find_next_token(node, top_node, file);
    if next_token.is_none()
        || next_token.is_some_and(|token| store.kind(token) == ast::Kind::CloseBraceToken)
    {
        return true;
    }

    let start_line = scanner::get_ecma_line_of_position(file, store.loc(node).end());
    let end_line = scanner::get_ecma_line_of_position(
        file,
        astnav::get_start_of_node(next_token.unwrap(), file),
    );
    start_line != end_line
}
