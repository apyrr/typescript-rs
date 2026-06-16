use std::{cell::RefCell, ops::ControlFlow};

use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_scanner as scanner;

use crate::LanguageService;

impl LanguageService<'_> {
    pub fn provide_selection_ranges(
        &self,
        _ctx: &core::Context,
        params: &lsproto::SelectionRangeParams,
    ) -> Result<lsproto::SelectionRangeResponse, core::Error> {
        let file_name = params.text_document.uri.to_string();
        let (_, source_file) = self.try_get_program_and_file(&file_name);
        let Some(source_file) = source_file else {
            return Ok(lsproto::SelectionRangesOrNull::default());
        };

        let mut results = Vec::new();
        for position in &params.positions {
            let position = lsproto::Position {
                line: position.line,
                character: position.character,
            };
            let pos = self
                .converters
                .line_and_character_to_position(source_file, position) as i32;
            if let Some(selection_range) = get_smart_selection_range(self, source_file, pos) {
                results.push(selection_range);
            }
        }

        Ok(lsproto::SelectionRangesOrNull {
            selection_ranges: Some(results.into_iter().map(Some).collect()),
            ..Default::default()
        })
    }
}

fn get_smart_selection_range(
    service: &LanguageService<'_>,
    source_file: &ast::SourceFile,
    pos: i32,
) -> Option<lsproto::SelectionRange> {
    let node_contains_position = |node: &ast::Node| {
        let start = scanner::get_token_pos_of_node(node, source_file, false);
        let end = source_file.store().loc(*node).end();
        start as i32 <= pos && pos < end
    };

    let push_selection_range = |current: Option<lsproto::SelectionRange>,
                                start: i32,
                                end: i32|
     -> Option<lsproto::SelectionRange> {
        if start == end {
            return current;
        }

        if !(start <= pos && pos <= end) {
            return current;
        }

        let lsp_range = service
            .converters
            .to_lsp_range(source_file, core::new_text_range(start, end));

        if current
            .as_ref()
            .is_some_and(|current| current.range == lsp_range)
        {
            return current;
        }

        Some(lsproto::SelectionRange {
            range: lsp_range,
            parent: current.map(Box::new),
        })
    };

    let push_selection_comment_range = |current: Option<lsproto::SelectionRange>,
                                        start: i32,
                                        end: i32|
     -> Option<lsproto::SelectionRange> {
        let current = push_selection_range(current, start, end);

        let mut comment_pos = start as usize;
        let text = source_file.text();
        while comment_pos < end as usize
            && comment_pos < text.len()
            && text.as_bytes()[comment_pos] == b'/'
        {
            comment_pos += 1;
        }

        push_selection_range(current, comment_pos as i32, end)
    };

    let positions_are_on_same_line = |pos1: i32, pos2: i32| {
        if pos1 == pos2 {
            return true;
        }
        let lsp_pos1 = service
            .converters
            .position_to_line_and_character(source_file, pos1);
        let lsp_pos2 = service
            .converters
            .position_to_line_and_character(source_file, pos2);
        lsp_pos1.line == lsp_pos2.line
    };

    let should_skip_node = |node: &ast::Node, parent: Option<&ast::Node>| {
        if ast::is_block(source_file.store(), *node) {
            return true;
        }

        if matches!(
            source_file.store().kind(*node),
            ast::Kind::TemplateSpan | ast::Kind::TemplateHead | ast::Kind::TemplateTail
        ) {
            return true;
        }

        if parent.is_some_and(|parent| ast::is_variable_statement(source_file.store(), *parent))
            && ast::is_variable_declaration_list(source_file.store(), *node)
        {
            return true;
        }

        // Skip lone variable declarations
        if parent
            .is_some_and(|parent| ast::is_variable_declaration_list(source_file.store(), *parent))
            && ast::is_variable_declaration(source_file.store(), *node)
        {
            let parent = source_file.store().parent(*node).unwrap();
            if source_file
                .store()
                .declarations(parent)
                .is_some_and(|declarations| declarations.len() == 1)
            {
                return true;
            }
        }

        false
    };

    let full_range = service.converters.to_lsp_range(
        source_file,
        core::new_text_range(source_file.pos(), source_file.end()),
    );
    let result = RefCell::new(Some(lsproto::SelectionRange {
        range: full_range,
        parent: None,
    }));

    let store = source_file.store();
    let mut current = Some(source_file.as_node());
    while let Some(node) = current {
        let parent = current;
        let mut next = None;

        let mut visit_node = |child: Option<ast::Node>| -> Option<ast::Node> {
            let Some(child) = child else {
                return None;
            };
            if next.is_some() {
                return None;
            }

            if let Some(found_comment) =
                scanner::get_trailing_comment_ranges(source_file.text(), store.loc(child).end())
                    .into_iter()
                    .next()
            {
                if found_comment.kind == ast::Kind::SingleLineCommentTrivia {
                    let current = result.borrow_mut().take();
                    *result.borrow_mut() = push_selection_comment_range(
                        current,
                        found_comment.text_range.pos(),
                        found_comment.text_range.end(),
                    );
                }
            }

            if node_contains_position(&child) {
                if ast::is_block(store, child)
                    && parent.is_some_and(|parent| {
                        ast::is_function_like_declaration(store, Some(parent))
                    })
                    && !positions_are_on_same_line(
                        astnav::get_start_of_node(child, source_file),
                        store.loc(child).end(),
                    )
                {
                    let start = astnav::get_start_of_node(child, source_file);
                    let end = store.loc(child).end();
                    let current = result.borrow_mut().take();
                    *result.borrow_mut() = push_selection_range(current, start, end);
                }

                // Synthesize a stop for '${ ... }' since '${' and '}' actually belong to siblings.
                if let Some(parent) = parent
                    && ast::is_template_span(store, parent)
                {
                    if let Some(literal) = store.literal(parent) {
                        // Start from just before the '${' and end after the '}'
                        // The '${' is 2 characters before the expression start
                        let span_start = store.loc(child).pos() - 2;
                        // The '}' is the first character of the template literal (middle or tail)
                        let span_end = astnav::get_start_of_node(literal, source_file) + 1;
                        // Validate the positions are reasonable
                        let text = source_file.text();
                        if span_start >= 0 && span_end <= text.len() as i32 && span_start < span_end
                        {
                            let current = result.borrow_mut().take();
                            *result.borrow_mut() =
                                push_selection_range(current, span_start, span_end);
                        }
                    }
                }

                if !should_skip_node(&child, parent.as_ref()) {
                    let start = astnav::get_start_of_node(child, source_file);
                    let end = store.loc(child).end();
                    let current = result.borrow_mut().take();
                    *result.borrow_mut() = push_selection_range(current, start, end);

                    if ast::is_string_literal(store, child)
                        || matches!(
                            store.kind(child),
                            ast::Kind::TemplateExpression
                                | ast::Kind::NoSubstitutionTemplateLiteral
                        )
                    {
                        // Only add inner content range if there's actually content (handles unterminated literals)
                        if start + 1 < end - 1 {
                            let current = result.borrow_mut().take();
                            *result.borrow_mut() =
                                push_selection_range(current, start + 1, end - 1);
                        }
                    }
                }

                next = Some(child);
            }
            None
        };

        let visit_nodes = |nodes: Option<Vec<ast::Node>>| -> Option<Vec<ast::Node>> {
            let Some(nodes) = nodes else {
                return None;
            };
            if nodes.is_empty() {
                return None;
            }

            let should_skip_list = parent.is_some_and(|parent| {
                ast::is_variable_declaration_list(store, parent)
                    || store.kind(parent) == ast::Kind::TemplateExpression
            });

            if !should_skip_list {
                let start = astnav::get_start_of_node(nodes[0], source_file);
                let end = store.loc(nodes[nodes.len() - 1]).end();
                if start <= pos && pos < end {
                    let current = result.borrow_mut().take();
                    *result.borrow_mut() = push_selection_range(current, start, end);
                }
            }

            None
        };

        let mut children = Vec::new();
        let _ = store.for_each_child(node, |child| {
            if let Some(child) = child {
                children.push(child);
            }
            ControlFlow::Continue(())
        });
        visit_nodes(Some(children.clone()));
        for child in children {
            visit_node(Some(child));
        }
        current = next;
    }

    result.into_inner()
}

#[allow(dead_code)]
fn _selection_range_type_check(_: &lsproto::SelectionRange) {}
