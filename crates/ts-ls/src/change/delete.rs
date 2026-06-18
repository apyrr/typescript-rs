use std::collections::HashMap;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_debug as debug;
use ts_format as format;
use ts_lsproto as lsproto;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use super::{
    LEADING_TRIVIA_OPTION_EXCLUDE, LEADING_TRIVIA_OPTION_INCLUDE_ALL,
    LEADING_TRIVIA_OPTION_START_LINE, LeadingTriviaOption, TRAILING_TRIVIA_OPTION_EXCLUDE,
    TRAILING_TRIVIA_OPTION_INCLUDE, TRAILING_TRIVIA_OPTION_NONE, Tracker, TrailingTriviaOption,
};

// deleteDeclaration deletes a node with smart handling for different node types.
// This handles special cases like import specifiers in lists, parameters, etc.
pub fn delete_declaration<'a>(
    t: &mut Tracker<'a>,
    deleted_nodes_in_lists: &mut HashMap<ast::Node, ast::Node>,
    source_file: &'a ast::SourceFile,
    node: ast::Node,
) {
    let store = source_file.store();
    match store.kind(node) {
        ast::Kind::Parameter => {
            let old_function = store
                .parent(node)
                .expect("parameter declaration should have parent");
            if store.kind(old_function) == ast::Kind::ArrowFunction
                && store
                    .parameters(old_function)
                    .expect("arrow function should have parameters")
                    .len()
                    == 1
                && !astnav::has_child_of_kind(old_function, ast::Kind::OpenParenToken, source_file)
            {
                // Lambdas with exactly one parameter are special because, after removal, there
                // must be an empty parameter list (i.e. `()`) and this won't necessarily be the
                // case if the parameter is simply removed (e.g. in `x => 1`).
                let range = t.get_adjusted_range(
                    source_file,
                    node,
                    node,
                    LEADING_TRIVIA_OPTION_INCLUDE_ALL,
                    TRAILING_TRIVIA_OPTION_INCLUDE,
                );
                t.replace_range_with_text(source_file, range, "()");
            } else {
                delete_node_in_list(t, deleted_nodes_in_lists, source_file, node);
            }
        }

        ast::Kind::ImportDeclaration | ast::Kind::ImportEqualsDeclaration => {
            let imports = source_file.imports();
            let first_statement_import = first_source_file_import_syntax_statement(source_file);
            let is_first_import = (!imports.is_empty()
                && store
                    .parent(imports[0])
                    .is_some_and(|parent| parent == node))
                || first_statement_import.is_some_and(|statement| statement == node);

            let leading_trivia = if is_first_import {
                LEADING_TRIVIA_OPTION_EXCLUDE
            } else {
                LEADING_TRIVIA_OPTION_START_LINE
            };
            delete_node(
                t,
                source_file,
                node,
                leading_trivia,
                TRAILING_TRIVIA_OPTION_INCLUDE,
            );
        }

        ast::Kind::BindingElement => {
            let pattern = store
                .parent(node)
                .expect("binding element should have binding pattern parent");
            let elements: Vec<_> = store
                .elements(pattern)
                .expect("binding pattern should have elements")
                .iter()
                .collect();
            let preserve_comma = store.kind(pattern) == ast::Kind::ArrayBindingPattern
                && node != elements[elements.len() - 1];
            if preserve_comma {
                delete_node(
                    t,
                    source_file,
                    node,
                    LEADING_TRIVIA_OPTION_INCLUDE_ALL,
                    TRAILING_TRIVIA_OPTION_EXCLUDE,
                );
            } else {
                delete_node_in_list(t, deleted_nodes_in_lists, source_file, node);
            }
        }

        ast::Kind::VariableDeclaration => {
            delete_variable_declaration(t, deleted_nodes_in_lists, source_file, node);
        }

        ast::Kind::TypeParameter => {
            delete_node_in_list(t, deleted_nodes_in_lists, source_file, node);
        }

        ast::Kind::ImportSpecifier => {
            let named_imports = store
                .parent(node)
                .expect("import specifier should have named imports parent");
            if store
                .elements(named_imports)
                .expect("named imports should have elements")
                .len()
                == 1
            {
                delete_import_binding(t, source_file, named_imports);
            } else {
                delete_node_in_list(t, deleted_nodes_in_lists, source_file, node);
            }
        }

        ast::Kind::NamespaceImport => {
            delete_import_binding(t, source_file, node);
        }

        ast::Kind::SemicolonToken => {
            delete_node(
                t,
                source_file,
                node,
                LEADING_TRIVIA_OPTION_INCLUDE_ALL,
                TRAILING_TRIVIA_OPTION_EXCLUDE,
            );
        }

        ast::Kind::TypeKeyword => {
            // For type keyword in import clauses, we need to delete the keyword and any trailing space
            // The trailing space is part of the next token's leading trivia, so we include it
            delete_node(
                t,
                source_file,
                node,
                LEADING_TRIVIA_OPTION_EXCLUDE,
                TRAILING_TRIVIA_OPTION_INCLUDE,
            );
        }

        ast::Kind::FunctionKeyword => {
            delete_node(
                t,
                source_file,
                node,
                LEADING_TRIVIA_OPTION_EXCLUDE,
                TRAILING_TRIVIA_OPTION_INCLUDE,
            );
        }

        ast::Kind::ClassDeclaration | ast::Kind::FunctionDeclaration => {
            delete_node(
                t,
                source_file,
                node,
                LEADING_TRIVIA_OPTION_START_LINE,
                TRAILING_TRIVIA_OPTION_INCLUDE,
            );
        }

        _ => {
            let parent = store.parent(node);
            let Some(parent) = parent else {
                // a misbehaving client can reach here with the SourceFile node
                delete_node(
                    t,
                    source_file,
                    node,
                    LEADING_TRIVIA_OPTION_INCLUDE_ALL,
                    TRAILING_TRIVIA_OPTION_INCLUDE,
                );
                return;
            };
            if store.kind(parent) == ast::Kind::ImportClause
                && store.name(parent).is_some_and(|name| name == node)
            {
                delete_default_import(t, source_file, parent);
            } else if store.kind(parent) == ast::Kind::CallExpression
                && store
                    .arguments(parent)
                    .expect("call expression should have arguments")
                    .iter()
                    .any(|argument| argument == node)
            {
                delete_node_in_list(t, deleted_nodes_in_lists, source_file, node);
            } else {
                delete_node(
                    t,
                    source_file,
                    node,
                    LEADING_TRIVIA_OPTION_INCLUDE_ALL,
                    TRAILING_TRIVIA_OPTION_INCLUDE,
                );
            }
        }
    }
}

pub fn delete_default_import<'a>(
    t: &mut Tracker<'a>,
    source_file: &'a ast::SourceFile,
    import_clause: ast::Node,
) {
    let store = source_file.store();
    if store.named_bindings(import_clause).is_none() {
        // Delete the whole import
        let import_declaration = store
            .parent(import_clause)
            .expect("import clause should have import declaration parent");
        delete_node(
            t,
            source_file,
            import_declaration,
            LEADING_TRIVIA_OPTION_INCLUDE_ALL,
            TRAILING_TRIVIA_OPTION_INCLUDE,
        );
    } else {
        // import |d,| * as ns from './file'
        let name = store.name(import_clause).unwrap();
        let start = astnav::get_start_of_node(name, source_file);
        let next_token = astnav::get_token_at_position(source_file, store.loc(name).end());
        if let Some(next_token) =
            next_token.filter(|token| store.kind(*token) == ast::Kind::CommaToken)
        {
            // shift first non-whitespace position after comma to the start position of the node
            let options = scanner::SkipTriviaOptions {
                stop_after_line_break: false,
                stop_at_comments: true,
            };
            let end = scanner::skip_trivia_ex(
                source_file.text(),
                store.loc(next_token).end() as usize,
                Some(&options),
            );
            let start_pos = t
                .converters
                .position_to_line_and_character(source_file, core::TextPos(start));
            let end_pos = t
                .converters
                .position_to_line_and_character(source_file, core::TextPos(end as i32));
            t.replace_range_with_text(
                source_file,
                lsproto::Range {
                    start: start_pos,
                    end: end_pos,
                },
                "",
            );
        } else {
            delete_node(
                t,
                source_file,
                name,
                LEADING_TRIVIA_OPTION_INCLUDE_ALL,
                TRAILING_TRIVIA_OPTION_INCLUDE,
            );
        }
    }
}

pub fn delete_import_binding<'a>(
    t: &mut Tracker<'a>,
    source_file: &'a ast::SourceFile,
    node: ast::Node,
) {
    let store = source_file.store();
    let parent = store
        .parent(node)
        .expect("import binding should have import clause parent");
    if store.name(parent).is_some() {
        // Delete named imports while preserving the default import
        // import d|, * as ns| from './file'
        // import d|, { a }| from './file'
        let previous_token = astnav::get_token_at_position(source_file, store.loc(node).pos() - 1);
        debug::assert(
            previous_token.is_some(),
            Some("previousToken should not be nil".to_string()),
        );
        let previous_token = previous_token.unwrap();
        let start_pos = t.converters.position_to_line_and_character(
            source_file,
            core::TextPos(astnav::get_start_of_node(previous_token, source_file)),
        );
        let end_pos = t
            .converters
            .position_to_line_and_character(source_file, core::TextPos(store.loc(node).end()));
        t.replace_range_with_text(
            source_file,
            lsproto::Range {
                start: start_pos,
                end: end_pos,
            },
            "",
        );
    } else {
        // Delete the entire import declaration
        // |import * as ns from './file'|
        // |import { a } from './file'|
        let import_decl = find_ancestor_kind_ref(store, Some(node), ast::Kind::ImportDeclaration);
        debug::assert(
            import_decl.is_some(),
            Some("importDecl should not be nil".to_string()),
        );
        delete_node(
            t,
            source_file,
            import_decl.unwrap(),
            LEADING_TRIVIA_OPTION_INCLUDE_ALL,
            TRAILING_TRIVIA_OPTION_INCLUDE,
        );
    }
}

pub fn delete_variable_declaration<'a>(
    t: &mut Tracker<'a>,
    deleted_nodes_in_lists: &mut HashMap<ast::Node, ast::Node>,
    source_file: &'a ast::SourceFile,
    node: ast::Node,
) {
    let store = source_file.store();
    let parent = store
        .parent(node)
        .expect("variable declaration should have parent");

    if store.kind(parent) == ast::Kind::CatchClause {
        // TODO: There's currently no unused diagnostic for this, could be a suggestion
        let open_paren =
            astnav::find_child_of_kind_info(parent, ast::Kind::OpenParenToken, source_file);
        let close_paren =
            astnav::find_child_of_kind_info(parent, ast::Kind::CloseParenToken, source_file);
        debug::assert(
            open_paren.is_some() && close_paren.is_some(),
            Some("catch clause should have parens".to_string()),
        );
        t.delete_token_info_range(
            source_file,
            open_paren.unwrap(),
            close_paren.unwrap(),
            LEADING_TRIVIA_OPTION_INCLUDE_ALL,
            TRAILING_TRIVIA_OPTION_INCLUDE,
        );
        return;
    }

    if store
        .declarations(parent)
        .expect("variable declaration list should have declarations")
        .len()
        != 1
    {
        delete_node_in_list(t, deleted_nodes_in_lists, source_file, node);
        return;
    }

    let gp = store
        .parent(parent)
        .expect("variable declaration list should have parent");
    match store.kind(gp) {
        ast::Kind::ForOfStatement | ast::Kind::ForInStatement => {
            let synthetic = core::new_text_range(-1, -1);
            let empty = t
                .node_factory
                .new_node_list(synthetic, synthetic, Vec::new());
            let replacement = t.node_factory.new_object_literal_expression(empty, false);
            t.replace_node(source_file, node, replacement, None);
        }

        ast::Kind::ForStatement => {
            delete_node(
                t,
                source_file,
                parent,
                LEADING_TRIVIA_OPTION_INCLUDE_ALL,
                TRAILING_TRIVIA_OPTION_INCLUDE,
            );
        }

        ast::Kind::VariableStatement => {
            delete_node(
                t,
                source_file,
                gp,
                LEADING_TRIVIA_OPTION_START_LINE,
                TRAILING_TRIVIA_OPTION_INCLUDE,
            );
        }

        _ => debug::fail(&format!("Unexpected grandparent kind: {}", store.kind(gp))),
    }
}

// deleteNode deletes a node with the specified trivia options.
// Warning: This deletes comments too.
pub fn delete_node<'a>(
    t: &mut Tracker<'a>,
    source_file: &'a ast::SourceFile,
    node: ast::Node,
    leading_trivia: LeadingTriviaOption,
    trailing_trivia: TrailingTriviaOption,
) {
    let start_position = t.get_adjusted_start_position(source_file, node, leading_trivia, false);
    let end_position = t.get_adjusted_end_position(source_file, node, trailing_trivia);
    let start_pos = t
        .converters
        .position_to_line_and_character(source_file, core::TextPos(start_position));
    let end_pos = t
        .converters
        .position_to_line_and_character(source_file, core::TextPos(end_position));
    t.replace_range_with_text(
        source_file,
        lsproto::Range {
            start: start_pos,
            end: end_pos,
        },
        "",
    );
}

pub fn delete_node_in_list<'a>(
    t: &mut Tracker<'a>,
    deleted_nodes_in_lists: &mut HashMap<ast::Node, ast::Node>,
    source_file: &'a ast::SourceFile,
    node: ast::Node,
) {
    let containing_list = format::get_containing_list(&node, source_file);
    debug::assert(
        containing_list.is_some(),
        Some("containingList should not be nil".to_string()),
    );
    let containing_list = containing_list.unwrap();
    let nodes: Vec<_> = containing_list.iter().collect();
    let index = nodes.iter().position(|list_node| *list_node == node);
    debug::assert(
        index.is_some(),
        Some("node should be in containing list".to_string()),
    );
    let index = index.unwrap();

    if nodes.len() == 1 {
        delete_node(
            t,
            source_file,
            node,
            LEADING_TRIVIA_OPTION_INCLUDE_ALL,
            TRAILING_TRIVIA_OPTION_INCLUDE,
        );
        return;
    }

    // Note: We will only delete a comma *after* a node. This will leave a trailing comma if we delete the last node.
    // That's handled in the end by finishTrailingCommaAfterDeletingNodesInList.
    debug::assert(
        !deleted_nodes_in_lists.contains_key(&node),
        Some("Deleting a node twice".to_string()),
    );
    deleted_nodes_in_lists.insert(node, node);

    let start_pos = t.start_position_to_delete_node_in_list(source_file, node);
    let end_pos = if index == nodes.len() - 1 {
        t.get_adjusted_end_position(source_file, node, TRAILING_TRIVIA_OPTION_NONE)
    } else {
        let prev_node = if index > 0 {
            Some(nodes[index - 1])
        } else {
            None
        };
        t.end_position_to_delete_node_in_list(source_file, node, prev_node, nodes[index + 1])
    };

    let start_ls_pos = t
        .converters
        .position_to_line_and_character(source_file, core::TextPos(start_pos));
    let end_ls_pos = t
        .converters
        .position_to_line_and_character(source_file, core::TextPos(end_pos));
    t.replace_range_with_text(
        source_file,
        lsproto::Range {
            start: start_ls_pos,
            end: end_ls_pos,
        },
        "",
    );
}

impl<'a> Tracker<'a> {
    // startPositionToDeleteNodeInList finds the first non-whitespace position in the leading trivia of the node
    pub fn start_position_to_delete_node_in_list(
        &self,
        source_file: &ast::SourceFile,
        node: ast::Node,
    ) -> i32 {
        let start = self.get_adjusted_start_position(
            source_file,
            node,
            LEADING_TRIVIA_OPTION_INCLUDE_ALL,
            false,
        );
        let options = scanner::SkipTriviaOptions {
            stop_after_line_break: false,
            stop_at_comments: true,
        };
        scanner::skip_trivia_ex(source_file.text(), start as usize, Some(&options)) as i32
    }

    pub fn end_position_to_delete_node_in_list(
        &self,
        source_file: &ast::SourceFile,
        node: ast::Node,
        prev_node: Option<ast::Node>,
        next_node: ast::Node,
    ) -> i32 {
        let end = self.start_position_to_delete_node_in_list(source_file, next_node);
        if prev_node.is_none()
            || positions_are_on_same_line(
                self.get_adjusted_end_position(source_file, node, TRAILING_TRIVIA_OPTION_INCLUDE),
                end,
                source_file,
            )
        {
            return end;
        }
        let token = astnav::find_preceding_token(
            source_file,
            astnav::get_start_of_node(next_node, source_file),
        );
        if is_separator(source_file.store(), node, token) {
            let prev_token = astnav::find_preceding_token(
                source_file,
                astnav::get_start_of_node(node, source_file),
            );
            if is_separator(source_file.store(), prev_node.unwrap(), prev_token) {
                let options = scanner::SkipTriviaOptions {
                    stop_after_line_break: true,
                    stop_at_comments: true,
                };
                let pos = scanner::skip_trivia_ex(
                    source_file.text(),
                    source_file.store().loc(token.unwrap()).end() as usize,
                    Some(&options),
                ) as i32;
                if positions_are_on_same_line(
                    astnav::get_start_of_node(prev_token.unwrap(), source_file),
                    astnav::get_start_of_node(token.unwrap(), source_file),
                    source_file,
                ) {
                    if pos > 0
                        && stringutil::is_line_break(
                            source_file.text().as_bytes()[pos as usize - 1] as char,
                        )
                    {
                        return pos - 1;
                    }
                    return pos;
                }
                if stringutil::is_line_break(source_file.text().as_bytes()[pos as usize] as char) {
                    return pos;
                }
            }
        }
        end
    }
}

fn first_source_file_import_syntax_statement(source_file: &ast::SourceFile) -> Option<ast::Node> {
    let store = source_file.store();
    source_file
        .statements_view()
        .iter()
        .find(|statement| ast::is_any_import_syntax(store, *statement))
}

pub(crate) fn positions_are_on_same_line(
    pos1: i32,
    pos2: i32,
    source_file: &ast::SourceFile,
) -> bool {
    format::get_line_start_position_for_position(pos1, source_file)
        == format::get_line_start_position_for_position(pos2, source_file)
}

fn is_separator(store: &ast::AstStore, node: ast::Node, token: Option<ast::Node>) -> bool {
    token.is_some_and(|token| {
        store.kind(token) == ast::Kind::CommaToken
            || store.kind(token) == ast::Kind::SemicolonToken
            || store.kind(token) == ast::Kind::BarToken && ast::is_type_node(store, node)
    })
}

fn find_ancestor_kind_ref(
    store: &ast::AstStore,
    node: Option<ast::Node>,
    kind: ast::Kind,
) -> Option<ast::Node> {
    let mut current = node.and_then(|node| store.parent(node));
    while let Some(node) = current {
        if store.kind(node) == kind {
            return Some(node);
        }
        current = store.parent(node);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_options(path: &str) -> ast::SourceFileParseOptions {
        ast::SourceFileParseOptions {
            file_name: path.to_string(),
            path: path.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn first_source_file_import_syntax_statement_should_return_first_top_level_import() {
        let file = ts_parser::parse_source_file(
            parse_options("/delete.ts"),
            "sideEffect();\nimport 'esm';\nimport legacy = require('legacy');",
            core::ScriptKind::TS,
        );
        let statements = file.statements_view().iter().collect::<Vec<_>>();

        assert_eq!(
            first_source_file_import_syntax_statement(&file),
            Some(statements[1])
        );
    }
}
