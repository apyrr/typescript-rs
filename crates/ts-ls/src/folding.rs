use std::cmp;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_debug as debug;
use ts_lsproto as lsproto;
use ts_printer as printer;
use ts_scanner as scanner;

use crate::LanguageService;
use crate::completions::get_line_end_of_position;

impl LanguageService<'_> {
    pub fn provide_folding_range(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
    ) -> Result<lsproto::FoldingRangeResponse, core::Error> {
        let (_, source_file) = self.get_program_and_file(document_uri);
        let mut res = self.add_node_outlining_spans(ctx, source_file);
        res.extend(self.add_region_outlining_spans(ctx, source_file));
        if lsproto::get_client_capabilities(ctx)
            .text_document
            .folding_range
            .line_folding_only
        {
            res = self.adjust_folding_end(res, source_file);
        }
        res.sort_by(|a, b| {
            let c = cmp::Ord::cmp(&a.start_line, &b.start_line);
            if c != cmp::Ordering::Equal {
                return c;
            }
            cmp::Ord::cmp(
                &a.start_character.unwrap_or_default(),
                &b.start_character.unwrap_or_default(),
            )
        });
        Ok(lsproto::FoldingRangesOrNull {
            folding_ranges: Some(res.into_iter().map(Some).collect()),
            ..Default::default()
        })
    }

    // adjustFoldingEnd adjusts the end line of folding ranges when the client signals lineFoldingOnly.
    // This mirrors the behavior of VS Code's built-in TypeScript extension (workaround for vscode#47240).
    // When lineFoldingOnly is true, we hide lines from startLine+1 to endLine. And to keep closing
    // brackets/braces visible, we subtract 1 from endLine when the range ends with a closing pair character.
    pub fn adjust_folding_end(
        &self,
        ranges: Vec<lsproto::FoldingRange>,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::FoldingRange> {
        let source_text = source_file.text();
        let mut result = Vec::with_capacity(ranges.len());
        for mut range in ranges {
            if let Some(end_character) = range.end_character {
                if end_character > 0 {
                    let end_offset = self.converters.line_and_character_to_position(
                        source_file,
                        lsproto::Position {
                            line: range.end_line,
                            character: end_character,
                        },
                    ) as usize;
                    if end_offset > 0 && end_offset <= source_text.len() {
                        let fold_end_char = source_text.as_bytes()[end_offset - 1] as char;
                        if matches!(fold_end_char, '}' | ']' | ')' | '`' | '>')
                            && range.end_line > range.start_line
                        {
                            range.end_line -= 1;
                        }
                    }
                }
            }
            result.push(range);
        }
        result
    }

    pub fn add_node_outlining_spans(
        &self,
        ctx: &core::Context,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::FoldingRange> {
        let depth_remaining = 40;
        let mut current = 0;

        let store = source_file.store();
        let statements: Vec<_> = source_file.statements_view().iter().collect();
        let n = statements.len();
        let mut folding_range = Vec::with_capacity(40);
        while current < n {
            while current < n && !ast::is_any_import_syntax(store, statements[current]) {
                folding_range.extend(visit_node(
                    ctx,
                    statements[current],
                    depth_remaining,
                    source_file,
                    self,
                ));
                current += 1;
            }
            if current == n {
                break;
            }
            let first_import = current;
            while current < n && ast::is_any_import_syntax(store, statements[current]) {
                folding_range.extend(visit_node(
                    ctx,
                    statements[current],
                    depth_remaining,
                    source_file,
                    self,
                ));
                current += 1;
            }
            let last_import = current - 1;
            if last_import != first_import {
                let folding_range_kind = lsproto::FoldingRangeKind::Imports;
                folding_range.push(create_folding_range_from_bounds(
                    ctx,
                    astnav::get_start_of_node(
                        astnav::find_child_of_kind(
                            statements[first_import],
                            ast::Kind::ImportKeyword,
                            source_file,
                        )
                        .unwrap(),
                        source_file,
                    ),
                    store.loc(statements[last_import]).end(),
                    Some(folding_range_kind),
                    source_file,
                    self,
                ));
            }
        }

        // Visit the EOF Token so that comments which aren't attached to statements are included.
        if let Some(end_of_file_token) = source_file.data().end_of_file_token() {
            folding_range.extend(visit_node(
                ctx,
                end_of_file_token,
                depth_remaining,
                source_file,
                self,
            ));
        }
        folding_range
    }

    pub fn add_region_outlining_spans(
        &self,
        ctx: &core::Context,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::FoldingRange> {
        let mut regions = Vec::with_capacity(40);
        let mut out = Vec::with_capacity(40);
        let line_starts = scanner::get_ecma_line_starts(source_file);
        for current_line_start in line_starts.iter().copied() {
            let line_end = get_line_end_of_position(source_file, current_line_start as i32);
            let line_text = &source_file.text()[current_line_start as usize..line_end as usize];
            let result = parse_region_delimiter(line_text);
            if result.is_none()
                || is_in_comment(
                    source_file,
                    current_line_start as i32,
                    astnav::get_token_at_position(source_file, current_line_start as i32),
                )
                .is_some()
            {
                continue;
            }

            let result = result.unwrap();
            if result.is_start {
                let comment_start = self.create_lsp_position(
                    source_file.text()[current_line_start as usize..line_end as usize]
                        .find("//")
                        .unwrap_or_default() as i32
                        + current_line_start as i32,
                    source_file,
                );
                let folding_range_kind_region = lsproto::FoldingRangeKind::Region;
                let mut region = lsproto::FoldingRange {
                    start_line: comment_start.line,
                    start_character: Some(comment_start.character),
                    kind: Some(folding_range_kind_region),
                    ..Default::default()
                };
                if supports_collapsed_text(ctx) {
                    let mut collapsed_text = "#region".to_string();
                    if !result.name.is_empty() {
                        collapsed_text = result.name;
                    }
                    region.collapsed_text = Some(collapsed_text);
                }
                // Our spans start out with some initial data.
                // On every `#endregion`, we'll come back to these `FoldingRange`s
                // and fill in their EndLine/EndCharacter.
                regions.push(region);
            } else if let Some(mut region) = regions.pop() {
                let ending_position = self.create_lsp_position(line_end, source_file);
                region.end_line = ending_position.line;
                region.end_character = Some(ending_position.character);
                out.push(region);
            }
        }
        out
    }
}

pub fn visit_node(
    ctx: &core::Context,
    node: ast::Node,
    mut depth_remaining: i32,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Vec<lsproto::FoldingRange> {
    let store = source_file.store();
    if store.flags(node).intersects(ast::NodeFlags::Reparsed)
        || depth_remaining == 0
        || ctx.err().is_some()
    {
        return Vec::new();
    }
    let mut folding_range = Vec::with_capacity(40);
    if (!ast::is_binary_expression(store, node) && ast::is_declaration(store, node))
        || ast::is_variable_statement(store, node)
        || ast::is_return_statement(store, node)
        || ast::is_call_or_new_expression(store, &node)
        || store.kind(node) == ast::Kind::EndOfFile
    {
        folding_range.extend(add_outlining_for_leading_comments_for_node(
            ctx,
            node,
            source_file,
            ls,
        ));
    }
    let parent = store.parent(node);
    if ast::is_function_like(store, Some(node))
        && parent
            .as_ref()
            .is_some_and(|parent| ast::is_binary_expression(store, *parent))
        && parent
            .as_ref()
            .and_then(|parent| store.left(*parent))
            .is_some_and(|left| ast::is_property_access_expression(store, left))
    {
        let left = parent
            .as_ref()
            .and_then(|parent| store.left(*parent))
            .unwrap();
        folding_range.extend(add_outlining_for_leading_comments_for_node(
            ctx,
            left,
            source_file,
            ls,
        ));
    }
    if ast::is_block(store, node) {
        let statements = store.statements(node);
        if let Some(statements) = statements {
            folding_range.extend(add_outlining_for_leading_comments_for_pos(
                ctx,
                statements.end(),
                source_file,
                ls,
            ));
        }
    }
    if ast::is_module_block(store, node) {
        let statements = store.statements(node);
        if let Some(statements) = statements {
            folding_range.extend(add_outlining_for_leading_comments_for_pos(
                ctx,
                statements.end(),
                source_file,
                ls,
            ));
        }
    }
    if ast::is_class_like(store, node) || ast::is_interface_declaration(store, node) {
        let members = store.members(node);
        if let Some(members) = members {
            folding_range.extend(add_outlining_for_leading_comments_for_pos(
                ctx,
                members.end(),
                source_file,
                ls,
            ));
        }
    }

    let span = get_outlining_span_for_node(ctx, node, source_file, ls);
    if let Some(span) = span {
        folding_range.push(span);
    }

    depth_remaining -= 1;
    if ast::is_call_expression(store, node) {
        depth_remaining += 1;
        let expression = store.expression(node).unwrap();
        folding_range.extend(visit_node(
            ctx,
            expression,
            depth_remaining,
            source_file,
            ls,
        ));
        depth_remaining -= 1;
        for arg in store
            .arguments(node)
            .into_iter()
            .flat_map(|args| args.iter())
        {
            folding_range.extend(visit_node(ctx, arg, depth_remaining, source_file, ls));
        }
        for type_arg in store
            .type_arguments(node)
            .into_iter()
            .flat_map(|args| args.iter())
        {
            folding_range.extend(visit_node(ctx, type_arg, depth_remaining, source_file, ls));
        }
    } else if ast::is_if_statement(store, node)
        && store.else_statement(node).is_some()
        && ast::is_if_statement(store, store.else_statement(node).unwrap())
    {
        // Consider an 'else if' to be on the same depth as the 'if'.
        let expression = store.expression(node).unwrap();
        folding_range.extend(visit_node(
            ctx,
            expression,
            depth_remaining,
            source_file,
            ls,
        ));
        folding_range.extend(visit_node(
            ctx,
            store.then_statement(node).unwrap(),
            depth_remaining,
            source_file,
            ls,
        ));
        depth_remaining += 1;
        if let Some(else_statement) = store.else_statement(node) {
            folding_range.extend(visit_node(
                ctx,
                else_statement,
                depth_remaining,
                source_file,
                ls,
            ));
        }
        depth_remaining -= 1;
    } else {
        let _ = store.for_each_present_child(node, &mut |child| {
            folding_range.extend(visit_node(ctx, child, depth_remaining, source_file, ls));
            std::ops::ControlFlow::Continue(())
        });
    }
    depth_remaining += 1;
    folding_range
}

pub fn add_outlining_for_leading_comments_for_node(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Vec<lsproto::FoldingRange> {
    if ast::is_jsx_text(source_file.store(), node) {
        return Vec::new();
    }
    add_outlining_for_leading_comments_for_pos(
        ctx,
        source_file.store().loc(node).pos(),
        source_file,
        ls,
    )
}

pub fn add_outlining_for_leading_comments_for_pos(
    ctx: &core::Context,
    pos: i32,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Vec<lsproto::FoldingRange> {
    let mut folding_range = Vec::with_capacity(40);
    let mut first_single_line_comment_start = -1;
    let mut last_single_line_comment_end = -1;
    let mut single_line_comment_count = 0;
    let folding_range_kind_comment = lsproto::FoldingRangeKind::Comment;

    let combine_and_add_multiple_single_line_comments = |single_line_comment_count: i32,
                                                         first_single_line_comment_start: i32,
                                                         last_single_line_comment_end: i32|
     -> Option<lsproto::FoldingRange> {
        // Only outline spans of two or more consecutive single line comments
        if single_line_comment_count > 1 {
            return Some(create_folding_range_from_bounds(
                ctx,
                first_single_line_comment_start,
                last_single_line_comment_end,
                Some(folding_range_kind_comment.clone()),
                source_file,
                ls,
            ));
        }
        None
    };

    let source_text = source_file.text();
    for comment in scanner::get_leading_comment_ranges(source_text, pos) {
        let comment_pos = comment.pos();
        let comment_end = comment.end();

        if ctx.err().is_some() {
            return Vec::new();
        }
        match comment.kind {
            ast::Kind::SingleLineCommentTrivia => {
                // never fold region delimiters into single-line comment regions
                let comment_text = &source_text[comment_pos as usize..comment_end as usize];
                if parse_region_delimiter(comment_text).is_some() {
                    if let Some(comments) = combine_and_add_multiple_single_line_comments(
                        single_line_comment_count,
                        first_single_line_comment_start,
                        last_single_line_comment_end,
                    ) {
                        folding_range.push(comments);
                    }
                    single_line_comment_count = 0;
                    continue;
                }

                // For single line comments, combine consecutive ones (2 or more) into
                // a single span from the start of the first till the end of the last
                if single_line_comment_count == 0 {
                    first_single_line_comment_start = comment_pos;
                }
                last_single_line_comment_end = comment_end;
                single_line_comment_count += 1;
            }
            ast::Kind::MultiLineCommentTrivia => {
                if let Some(comments) = combine_and_add_multiple_single_line_comments(
                    single_line_comment_count,
                    first_single_line_comment_start,
                    last_single_line_comment_end,
                ) {
                    folding_range.push(comments);
                }
                folding_range.push(create_folding_range_from_bounds(
                    ctx,
                    comment_pos,
                    comment_end,
                    Some(folding_range_kind_comment.clone()),
                    source_file,
                    ls,
                ));
                single_line_comment_count = 0;
            }
            _ => debug::assert_never(&comment.kind, None),
        }
    }
    if let Some(added_comments) = combine_and_add_multiple_single_line_comments(
        single_line_comment_count,
        first_single_line_comment_start,
        last_single_line_comment_end,
    ) {
        folding_range.push(added_comments);
    }
    folding_range
}

pub struct RegionDelimiterResult {
    pub is_start: bool,
    pub name: String,
}

pub fn parse_region_delimiter(line_text: &str) -> Option<RegionDelimiterResult> {
    // We trim the leading whitespace and // without the regex since the
    // multiple potential whitespace matches can make for some gnarly backtracking behavior
    let mut line_text = line_text.trim_start_matches(char::is_whitespace);
    if !line_text.starts_with("//") {
        return None;
    }
    line_text = line_text[2..].trim();
    line_text = line_text.trim_end_matches('\r');
    if !line_text.starts_with('#') {
        return None;
    }
    line_text = &line_text[1..];
    let mut is_start = true;
    if line_text.starts_with("end") {
        is_start = false;
        line_text = &line_text[3..];
    }
    if !line_text.starts_with("region") {
        return None;
    }
    line_text = &line_text[6..];
    Some(RegionDelimiterResult {
        is_start,
        name: line_text.trim().to_string(),
    })
}

pub fn get_outlining_span_for_node(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let store = source_file.store();
    match store.kind(node) {
        ast::Kind::Block => {
            let parent = store.parent(node);
            if ast::is_function_like(store, parent) {
                return function_span(ctx, parent.unwrap(), node, source_file, ls);
            }
            // Check if the block is standalone, or 'attached' to some parent statement.
            // If the latter, we want to collapse the block, but consider its hint span
            // to be the entire span of the parent.
            let parent = store.parent(node).unwrap();
            match store.kind(parent) {
                ast::Kind::DoStatement
                | ast::Kind::ForInStatement
                | ast::Kind::ForOfStatement
                | ast::Kind::ForStatement
                | ast::Kind::IfStatement
                | ast::Kind::WhileStatement
                | ast::Kind::WithStatement
                | ast::Kind::CatchClause => {
                    span_for_node(ctx, node, ast::Kind::OpenBraceToken, true, source_file, ls)
                }
                ast::Kind::TryStatement => {
                    // Could be the try-block, or the finally-block.
                    if store
                        .try_block(parent)
                        .is_some_and(|try_block| try_block == node)
                    {
                        span_for_node(ctx, node, ast::Kind::OpenBraceToken, true, source_file, ls)
                    } else if store
                        .finally_block(parent)
                        .is_some_and(|finally_block| finally_block == node)
                    {
                        if let Some(span) = span_for_node(
                            ctx,
                            node,
                            ast::Kind::OpenBraceToken,
                            true,
                            source_file,
                            ls,
                        ) {
                            Some(span)
                        } else {
                            create_folding_range(
                                ctx,
                                ls.create_lsp_range_from_node(node, source_file),
                                None,
                                "",
                            )
                        }
                    } else {
                        create_folding_range(
                            ctx,
                            ls.create_lsp_range_from_node(node, source_file),
                            None,
                            "",
                        )
                    }
                }
                _ => {
                    // Block was a standalone block.  In this case we want to only collapse
                    // the span of the block, independent of any parent span.
                    create_folding_range(
                        ctx,
                        ls.create_lsp_range_from_node(node, source_file),
                        None,
                        "",
                    )
                }
            }
        }
        ast::Kind::ModuleBlock => {
            span_for_node(ctx, node, ast::Kind::OpenBraceToken, true, source_file, ls)
        }
        ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::CaseBlock
        | ast::Kind::TypeLiteral
        | ast::Kind::ObjectBindingPattern => {
            span_for_node(ctx, node, ast::Kind::OpenBraceToken, true, source_file, ls)
        }
        ast::Kind::TupleType => span_for_node(
            ctx,
            node,
            ast::Kind::OpenBracketToken,
            !store
                .parent(node)
                .as_ref()
                .is_some_and(|parent| ast::is_tuple_type_node(store, *parent)),
            source_file,
            ls,
        ),
        ast::Kind::CaseClause | ast::Kind::DefaultClause => {
            span_for_node_array(ctx, store.statements(node), source_file, ls)
        }
        ast::Kind::ObjectLiteralExpression => span_for_node(
            ctx,
            node,
            ast::Kind::OpenBraceToken,
            !store
                .parent(node)
                .as_ref()
                .is_some_and(|parent| ast::is_array_literal_expression(store, *parent))
                && !store
                    .parent(node)
                    .as_ref()
                    .is_some_and(|parent| ast::is_call_expression(store, *parent)),
            source_file,
            ls,
        ),
        ast::Kind::ArrayLiteralExpression => span_for_node(
            ctx,
            node,
            ast::Kind::OpenBracketToken,
            !store
                .parent(node)
                .as_ref()
                .is_some_and(|parent| ast::is_array_literal_expression(store, *parent))
                && !store
                    .parent(node)
                    .as_ref()
                    .is_some_and(|parent| ast::is_call_expression(store, *parent)),
            source_file,
            ls,
        ),
        ast::Kind::JsxElement | ast::Kind::JsxFragment => {
            span_for_jsx_element(ctx, node, source_file, ls)
        }
        ast::Kind::JsxSelfClosingElement | ast::Kind::JsxOpeningElement => {
            span_for_jsx_attributes(ctx, node, source_file, ls)
        }
        ast::Kind::TemplateExpression | ast::Kind::NoSubstitutionTemplateLiteral => {
            span_for_template_literal(ctx, node, source_file, ls)
        }
        ast::Kind::ArrayBindingPattern => span_for_node(
            ctx,
            node,
            ast::Kind::OpenBracketToken,
            !store
                .parent(node)
                .as_ref()
                .is_some_and(|parent| ast::is_binding_element(store, *parent)),
            source_file,
            ls,
        ),
        ast::Kind::ArrowFunction => span_for_arrow_function(ctx, node, source_file, ls),
        ast::Kind::CallExpression => span_for_call_expression(ctx, node, source_file, ls),
        ast::Kind::ParenthesizedExpression => {
            span_for_parenthesized_expression(ctx, node, source_file, ls)
        }
        ast::Kind::NamedImports | ast::Kind::NamedExports | ast::Kind::ImportAttributes => {
            span_for_import_export_elements(ctx, node, source_file, ls)
        }
        _ => None,
    }
}

pub fn span_for_import_export_elements(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let store = source_file.store();
    let elements = match store.kind(node) {
        ast::Kind::NamedImports | ast::Kind::NamedExports => store.elements(node),
        ast::Kind::ImportAttributes => store
            .attributes(node)
            .and_then(|attrs| store.properties(attrs)),
        _ => None,
    };
    let Some(elements) = elements else {
        return None;
    };
    if elements.is_empty() {
        return None;
    }
    let open_token = astnav::find_child_of_kind(node, ast::Kind::OpenBraceToken, source_file);
    let close_token = astnav::find_child_of_kind(node, ast::Kind::CloseBraceToken, source_file);
    let (Some(open_token), Some(close_token)) = (open_token, close_token) else {
        return None;
    };
    if printer::positions_are_on_same_line(
        store.loc(open_token).pos(),
        store.loc(close_token).pos(),
        source_file,
    ) {
        return None;
    }
    range_between_tokens(ctx, open_token, close_token, source_file, false, ls)
}

pub fn span_for_parenthesized_expression(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let start = astnav::get_start_of_node(node, source_file);
    let end = source_file.store().loc(node).end();
    if printer::positions_are_on_same_line(start, end, source_file) {
        return None;
    }
    let text_range = ls.create_lsp_range_from_bounds(start, end, source_file);
    create_folding_range(ctx, text_range, None, "")
}

pub fn span_for_call_expression(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let store = source_file.store();
    if store
        .arguments(node)
        .is_none_or(|arguments| arguments.is_empty())
    {
        return None;
    }
    let open_token = astnav::find_child_of_kind(node, ast::Kind::OpenParenToken, source_file);
    let close_token = astnav::find_child_of_kind(node, ast::Kind::CloseParenToken, source_file);
    let (Some(open_token), Some(close_token)) = (open_token, close_token) else {
        return None;
    };
    if printer::positions_are_on_same_line(
        store.loc(open_token).pos(),
        store.loc(close_token).pos(),
        source_file,
    ) {
        return None;
    }

    range_between_tokens(ctx, open_token, close_token, source_file, true, ls)
}

pub fn span_for_arrow_function(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let store = source_file.store();
    let body = store.body(node).unwrap();
    if ast::is_block(store, body)
        || ast::is_parenthesized_expression(store, body)
        || printer::positions_are_on_same_line(
            store.loc(body).pos(),
            store.loc(body).end(),
            source_file,
        )
    {
        return None;
    }
    let text_range =
        ls.create_lsp_range_from_bounds(store.loc(body).pos(), store.loc(body).end(), source_file);
    create_folding_range(ctx, text_range, None, "")
}

pub fn span_for_template_literal(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    if source_file.store().kind(node) == ast::Kind::NoSubstitutionTemplateLiteral
        && source_file.store().text(node).is_empty()
    {
        return None;
    }
    create_folding_range_from_bounds(
        ctx,
        astnav::get_start_of_node(node, source_file),
        source_file.store().loc(node).end(),
        None,
        source_file,
        ls,
    )
    .into()
}

pub fn span_for_jsx_element(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let store = source_file.store();
    if store.kind(node) == ast::Kind::JsxElement {
        let opening_element = store.opening_element(node).unwrap();
        let closing_element = store.closing_element(node).unwrap();
        let text_range = ls.create_lsp_range_from_bounds(
            astnav::get_start_of_node(opening_element, source_file),
            store.loc(closing_element).end(),
            source_file,
        );
        let tag_name =
            scanner::get_text_of_node(source_file, &store.tag_name(opening_element).unwrap());
        let banner_text = format!("<{tag_name}>...</{tag_name}>");
        return create_folding_range(ctx, text_range, None, &banner_text);
    }
    // JsxFragment
    let opening_fragment = store.opening_fragment(node).unwrap();
    let closing_fragment = store.closing_fragment(node).unwrap();
    let text_range = ls.create_lsp_range_from_bounds(
        astnav::get_start_of_node(opening_fragment, source_file),
        store.loc(closing_fragment).end(),
        source_file,
    );
    create_folding_range(ctx, text_range, None, "<>...</>")
}

pub fn span_for_jsx_attributes(
    ctx: &core::Context,
    node: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let store = source_file.store();
    let attributes = store.attributes(node);
    if attributes
        .and_then(|attributes| store.properties(attributes))
        .is_none_or(|properties| properties.is_empty())
    {
        return None;
    }
    create_folding_range_from_bounds(
        ctx,
        astnav::get_start_of_node(node, source_file),
        store.loc(node).end(),
        None,
        source_file,
        ls,
    )
    .into()
}

pub fn span_for_node_array(
    ctx: &core::Context,
    statements: Option<ast::SourceNodeList<'_>>,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    if let Some(statements) = statements {
        if !statements.is_empty() {
            return create_folding_range(
                ctx,
                ls.create_lsp_range_from_bounds(statements.pos(), statements.end(), source_file),
                None,
                "",
            );
        }
    }
    None
}

pub fn span_for_node(
    ctx: &core::Context,
    node: ast::Node,
    open: ast::Kind,
    use_full_start: bool,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let mut close_brace = ast::Kind::CloseBraceToken;
    if open != ast::Kind::OpenBraceToken {
        close_brace = ast::Kind::CloseBracketToken;
    }
    let open_token = astnav::find_child_of_kind(node, open, source_file);
    let close_token = astnav::find_child_of_kind(node, close_brace, source_file);
    if open_token.is_some() && close_token.is_some() {
        let open_token = open_token.unwrap();
        let close_token = close_token.unwrap();
        return range_between_tokens(
            ctx,
            open_token,
            close_token,
            source_file,
            use_full_start,
            ls,
        );
    }
    None
}

pub fn range_between_tokens(
    ctx: &core::Context,
    open_token: ast::Node,
    close_token: ast::Node,
    source_file: &ast::SourceFile,
    use_full_start: bool,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let text_range = if use_full_start {
        ls.create_lsp_range_from_bounds(
            source_file.store().loc(open_token).pos(),
            source_file.store().loc(close_token).end(),
            source_file,
        )
    } else {
        ls.create_lsp_range_from_bounds(
            astnav::get_start_of_node(open_token, source_file),
            source_file.store().loc(close_token).end(),
            source_file,
        )
    };
    create_folding_range(ctx, text_range, None, "")
}

pub fn supports_collapsed_text(ctx: &core::Context) -> bool {
    lsproto::get_client_capabilities(ctx)
        .text_document
        .folding_range
        .folding_range
        .collapsed_text
}

pub fn create_folding_range(
    ctx: &core::Context,
    text_range: lsproto::Range,
    folding_range_kind: Option<lsproto::FoldingRangeKind>,
    collapsed_text: &str,
) -> Option<lsproto::FoldingRange> {
    let mut result = lsproto::FoldingRange {
        start_line: text_range.start.line,
        start_character: Some(text_range.start.character),
        end_line: text_range.end.line,
        end_character: Some(text_range.end.character),
        kind: folding_range_kind,
        ..Default::default()
    };
    if !collapsed_text.is_empty() && supports_collapsed_text(ctx) {
        result.collapsed_text = Some(collapsed_text.to_string());
    }
    Some(result)
}

pub fn create_folding_range_from_bounds(
    ctx: &core::Context,
    pos: i32,
    end: i32,
    folding_range_kind: Option<lsproto::FoldingRangeKind>,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> lsproto::FoldingRange {
    create_folding_range(
        ctx,
        ls.create_lsp_range_from_bounds(pos, end, source_file),
        folding_range_kind,
        "",
    )
    .unwrap()
}

pub fn function_span(
    ctx: &core::Context,
    node: ast::Node,
    body: ast::Node,
    source_file: &ast::SourceFile,
    ls: &LanguageService<'_>,
) -> Option<lsproto::FoldingRange> {
    let open_token = try_get_function_open_token(node, body, source_file);
    let close_token = astnav::find_child_of_kind(body, ast::Kind::CloseBraceToken, source_file);
    if open_token.is_some() && close_token.is_some() {
        let open_token = open_token.unwrap();
        let close_token = close_token.unwrap();
        return range_between_tokens(
            ctx,
            open_token,
            close_token,
            source_file,
            true, /*use_full_start*/
            ls,
        );
    }
    None
}

pub fn try_get_function_open_token(
    node: ast::Node,
    body: ast::Node,
    source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    if is_node_array_multi_line(source_file.store().parameters(node), source_file) {
        let open_paren_token =
            astnav::find_child_of_kind(node, ast::Kind::OpenParenToken, source_file);
        if open_paren_token.is_some() {
            return open_paren_token;
        }
    }
    astnav::find_child_of_kind(body, ast::Kind::OpenBraceToken, source_file)
}

pub fn is_node_array_multi_line(
    list: Option<ast::SourceNodeList<'_>>,
    source_file: &ast::SourceFile,
) -> bool {
    let Some(list) = list else {
        return false;
    };
    if list.is_empty() {
        return false;
    }
    let first = list.iter().next().unwrap();
    let last = list.iter().last().unwrap();
    let store = source_file.store();
    !printer::positions_are_on_same_line(store.loc(first).pos(), store.loc(last).end(), source_file)
}

pub fn is_in_comment(
    file: &ast::SourceFile,
    position: i32,
    token_at_position: Option<ast::Node>,
) -> Option<ast::CommentRange> {
    get_range_of_enclosing_comment(
        file,
        position,
        astnav::find_preceding_token(file, position),
        token_at_position,
    )
}

// Unlike the TS implementation, this function *will not* compute default values for
// `precedingToken` and `tokenAtPosition`.
// It is the caller's responsibility to call `astnav.GetTokenAtPosition` to compute a default `tokenAtPosition`,
// or `astnav.FindPrecedingToken` to compute a default `precedingToken`.
pub fn get_range_of_enclosing_comment(
    file: &ast::SourceFile,
    position: i32,
    preceding_token: Option<ast::Node>,
    token_at_position: Option<ast::Node>,
) -> Option<ast::CommentRange> {
    let token_at_position = token_at_position?;
    let store = file.store();
    let token_start = astnav::get_start_of_node(token_at_position, file);
    if token_start <= position && position < store.loc(token_at_position).end() {
        return None;
    }

    // Between two consecutive tokens, all comments are either trailing on the former
    // or leading on the latter (and none are in both lists).
    let mut comment_ranges = Vec::new();
    if let Some(preceding_token) = preceding_token {
        comment_ranges.extend(scanner::get_trailing_comment_ranges(
            file.text(),
            store.loc(preceding_token).end(),
        ));
    }
    comment_ranges.extend(get_leading_comment_ranges_of_node(token_at_position, file));
    for comment_range in comment_ranges {
        // The end marker of a single-line comment does not include the newline character.
        // In the following case where the cursor is at `^`, we are inside a comment:
        //
        //    // asdf   ^\n
        //
        // But for closed multi-line comments, we don't want to be inside the comment in the following case:
        //
        //    /* asdf */^
        //
        // Internally, we represent the end of the comment prior to the newline and at the '/', respectively.
        //
        // However, unterminated multi-line comments lack a `/`, end at the end of the file, and *do* contain their end.
        //
        if comment_range.text_range.contains_exclusive(position)
            || position == comment_range.end()
                && (comment_range.kind == ast::Kind::SingleLineCommentTrivia
                    || position as usize == file.text().len())
        {
            return Some(comment_range);
        }
    }
    None
}

pub fn get_leading_comment_ranges_of_node(
    node: ast::Node,
    file: &ast::SourceFile,
) -> Vec<ast::CommentRange> {
    if file.store().kind(node) == ast::Kind::JsxText {
        return Vec::new();
    }
    scanner::get_leading_comment_ranges(file.text(), file.store().loc(node).pos())
}
