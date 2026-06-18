use std::collections::HashMap;

use ts_ast as ast;
use ts_astnav as astnav;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_lsproto::DocumentUriExt;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::LanguageService;
use crate::findallreferences::{ENTRY_KIND_RANGE, REFERENCE_USE_NONE, RefOptions, ReferenceEntry};
use crate::lsconv;
use crate::lsutil;
use crate::utilities::{
    SyntaxChild, get_children_with_tokens, source_node_symbol_declarations_snapshot_from_program,
};

impl LanguageService<'_> {
    pub fn provide_document_highlights(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        document_position: lsproto::Position,
    ) -> Result<lsproto::DocumentHighlightResponse, core::Error> {
        let result = self.provide_document_highlights_worker(
            ctx,
            document_uri.clone(),
            document_position,
            Vec::new(),
        )?;
        // Extract highlights for the current file only.
        let mut document_highlights = Vec::new();
        if let Some(multi_document_highlights) = result.multi_document_highlights {
            for mh in multi_document_highlights {
                let Some(mh) = mh else {
                    continue;
                };
                if mh.uri == document_uri {
                    document_highlights.extend(mh.highlights);
                }
            }
        }
        Ok(lsproto::DocumentHighlightsOrNull {
            document_highlights: Some(document_highlights.into_iter().map(Some).collect()),
            ..Default::default()
        })
    }

    pub fn provide_multi_document_highlights(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        document_position: lsproto::Position,
        files_to_search: Vec<lsproto::DocumentUri>,
    ) -> Result<lsproto::CustomMultiDocumentHighlightResponse, core::Error> {
        self.provide_document_highlights_worker(
            ctx,
            document_uri,
            document_position,
            files_to_search,
        )
    }

    pub fn provide_document_highlights_worker(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        document_position: lsproto::Position,
        files_to_search: Vec<lsproto::DocumentUri>,
    ) -> Result<lsproto::MultiDocumentHighlightsOrNull, core::Error> {
        let (program, source_file) = self.get_program_and_file(document_uri.clone());
        let position =
            self.converters
                .line_and_character_to_position(source_file, document_position) as i32;
        let Some(node) = astnav::get_touching_property_name(source_file, position) else {
            return Ok(lsproto::MultiDocumentHighlightsOrNull::default());
        };

        // Cheap JSX check before resolving files to search.
        let store = source_file.store();
        if let Some(parent) = store.parent(node)
            && (store.kind(parent) == ast::Kind::JsxClosingElement
                || (store.kind(parent) == ast::Kind::JsxOpeningElement
                    && store
                        .tag_name(parent)
                        .is_some_and(|tag_name| tag_name == node)))
        {
            let mut opening_element: Option<ast::Node> = None;
            let mut closing_element: Option<ast::Node> = None;
            if let Some(parent_parent) = store.parent(parent)
                && ast::is_jsx_element(store, parent_parent)
            {
                opening_element = store.opening_element(parent_parent);
                closing_element = store.closing_element(parent_parent);
            }
            let mut highlights = Vec::new();
            let kind = lsproto::DocumentHighlightKind::Read;
            if let Some(opening_element) = opening_element {
                highlights.push(lsproto::DocumentHighlight {
                    range: self.create_lsp_range_from_node(opening_element, source_file),
                    kind: Some(kind),
                });
            }
            if let Some(closing_element) = closing_element {
                highlights.push(lsproto::DocumentHighlight {
                    range: self.create_lsp_range_from_node(closing_element, source_file),
                    kind: Some(kind),
                });
            }
            let multi_highlights = vec![lsproto::MultiDocumentHighlight {
                uri: document_uri,
                highlights,
            }];
            return Ok(lsproto::MultiDocumentHighlightsOrNull {
                multi_document_highlights: Some(multi_highlights.into_iter().map(Some).collect()),
                ..Default::default()
            });
        }

        // Resolve the source files to search, deduplicating by file name.
        let mut source_files: Vec<&ast::SourceFile> = Vec::new();
        let mut seen_files = collections::Set::new();
        for uri in files_to_search {
            let file_name = uri.file_name();
            if !seen_files.add_if_absent(file_name.clone()) {
                continue;
            }
            if let Some(sf) = program.get_source_file_ref(&file_name) {
                source_files.push(sf);
            }
        }
        if source_files.is_empty() {
            source_files = vec![source_file];
        }

        let mut multi_highlights =
            self.get_semantic_document_highlights(ctx, position, node, program, &source_files)?;
        if multi_highlights.is_empty() {
            // Fall back to syntactic highlights for the current file only.
            let syntactic_highlights = self.get_syntactic_document_highlights(node, source_file);
            if !syntactic_highlights.is_empty() {
                multi_highlights = vec![lsproto::MultiDocumentHighlight {
                    uri: document_uri,
                    highlights: syntactic_highlights,
                }];
            }
        }
        Ok(lsproto::MultiDocumentHighlightsOrNull {
            multi_document_highlights: Some(multi_highlights.into_iter().map(Some).collect()),
            ..Default::default()
        })
    }

    pub fn get_semantic_document_highlights(
        &self,
        ctx: &core::Context,
        position: i32,
        node: ast::Node,
        program: &compiler::Program,
        source_files: &[&ast::SourceFile],
    ) -> Result<Vec<lsproto::MultiDocumentHighlight>, core::Error> {
        let options = RefOptions {
            use_: REFERENCE_USE_NONE,
            ..Default::default()
        };
        let reference_entries = self.get_referenced_symbols_for_node(
            ctx,
            position,
            node,
            program,
            source_files,
            options,
        )?;
        // Group highlights by file
        let mut file_highlights: HashMap<String, Vec<lsproto::DocumentHighlight>> = HashMap::new();
        for entry in reference_entries {
            for reference in entry.references {
                let (file_name, highlight) = self.to_document_highlight(&reference);
                file_highlights
                    .entry(file_name)
                    .or_default()
                    .push(highlight);
            }
        }

        let mut result = Vec::new();
        for sf in source_files {
            if let Some(highlights) = file_highlights.remove(&sf.file_name()) {
                result.push(lsproto::MultiDocumentHighlight {
                    uri: lsconv::file_name_to_document_uri(&sf.file_name()),
                    highlights,
                });
            }
        }
        Ok(result)
    }

    pub(crate) fn to_document_highlight(
        &self,
        entry: &ReferenceEntry,
    ) -> (String, lsproto::DocumentHighlight) {
        let entry = self.resolve_entry(entry);

        let mut kind = lsproto::DocumentHighlightKind::Read;
        if entry.kind == ENTRY_KIND_RANGE {
            return (
                entry.file_name.clone(),
                lsproto::DocumentHighlight {
                    range: self.get_range_of_entry(&entry),
                    kind: Some(kind),
                },
            );
        }

        // Determine write access for node references.
        if let Some(source_file) = self.get_program().get_source_file_ref(&entry.file_name) {
            if ast::is_write_access_for_reference(source_file.store(), entry.node) {
                kind = lsproto::DocumentHighlightKind::Write;
            }
        }

        let dh = lsproto::DocumentHighlight {
            range: self.get_range_of_entry(&entry),
            kind: Some(kind),
        };

        (entry.file_name.clone(), dh)
    }

    pub fn get_syntactic_document_highlights(
        &self,
        node: ast::Node,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::DocumentHighlight> {
        let store = source_file.store();
        match store.kind(node) {
            ast::Kind::IfKeyword | ast::Kind::ElseKeyword => {
                if let Some(parent) = store.parent(node)
                    && ast::is_if_statement(store, parent)
                {
                    return self.get_if_else_occurrences(parent, source_file);
                }
                Vec::new()
            }
            ast::Kind::ReturnKeyword => self.use_parent_ranges(
                source_file.store().parent(node),
                ast::is_return_statement,
                get_return_occurrences,
                source_file,
            ),
            ast::Kind::ThrowKeyword => self.use_parent_ranges(
                source_file.store().parent(node),
                ast::is_throw_statement,
                get_throw_occurrences,
                source_file,
            ),
            ast::Kind::TryKeyword | ast::Kind::CatchKeyword | ast::Kind::FinallyKeyword => {
                let try_statement = if store.kind(node) == ast::Kind::CatchKeyword {
                    store.parent(node).and_then(|parent| store.parent(parent))
                } else {
                    store.parent(node)
                };
                self.use_parent_ranges(
                    try_statement,
                    ast::is_try_statement,
                    get_try_catch_finally_occurrence_ranges,
                    source_file,
                )
            }
            ast::Kind::SwitchKeyword => self.use_parent_ranges(
                source_file.store().parent(node),
                ast::is_switch_statement,
                get_switch_case_default_occurrence_ranges,
                source_file,
            ),
            ast::Kind::CaseKeyword | ast::Kind::DefaultKeyword => {
                if store
                    .parent(node)
                    .as_ref()
                    .is_some_and(|parent| ast::is_default_clause(store, *parent))
                    || store
                        .parent(node)
                        .as_ref()
                        .is_some_and(|parent| ast::is_case_clause(store, *parent))
                {
                    return self.use_parent_ranges(
                        store
                            .parent(node)
                            .and_then(|parent| store.parent(parent))
                            .and_then(|parent| store.parent(parent)),
                        ast::is_switch_statement,
                        get_switch_case_default_occurrence_ranges,
                        source_file,
                    );
                }
                Vec::new()
            }
            ast::Kind::BreakKeyword | ast::Kind::ContinueKeyword => self.use_parent_ranges(
                source_file.store().parent(node),
                ast::is_break_or_continue_statement,
                get_break_or_continue_statement_occurrence_ranges,
                source_file,
            ),
            ast::Kind::ForKeyword | ast::Kind::WhileKeyword | ast::Kind::DoKeyword => self
                .use_parent_ranges(
                    source_file.store().parent(node),
                    |store, n| ast::is_iteration_statement(store, n, true),
                    get_loop_break_continue_occurrence_ranges,
                    source_file,
                ),
            ast::Kind::ConstructorKeyword => self.get_from_all_declarations(
                |store, node| ast::is_constructor_declaration(store, node),
                &[ast::Kind::ConstructorKeyword],
                node,
                source_file,
            ),
            ast::Kind::GetKeyword | ast::Kind::SetKeyword => self.get_from_all_declarations(
                |store, node| ast::is_accessor(store, node),
                &[ast::Kind::GetKeyword, ast::Kind::SetKeyword],
                node,
                source_file,
            ),
            ast::Kind::AwaitKeyword => self.use_parent_ranges(
                source_file.store().parent(node),
                ast::is_await_expression,
                get_async_and_await_occurrence_ranges,
                source_file,
            ),
            ast::Kind::AsyncKeyword => self.highlight_ranges(
                get_async_and_await_occurrence_ranges(node, source_file),
                source_file,
            ),
            ast::Kind::YieldKeyword => {
                self.highlight_ranges(get_yield_occurrence_ranges(node, source_file), source_file)
            }
            ast::Kind::InKeyword | ast::Kind::OutKeyword => Vec::new(),
            _ => {
                let node_kind = store.kind(node);
                if ast::is_modifier_kind(node_kind)
                    && (store
                        .parent(node)
                        .as_ref()
                        .is_some_and(|parent| ast::is_declaration(store, *parent))
                        || store
                            .parent(node)
                            .as_ref()
                            .is_some_and(|parent| ast::is_variable_statement(store, *parent)))
                {
                    if let Some(parent) = store.parent(node) {
                        return self.highlight_spans(
                            get_modifier_occurrences(node_kind, parent, source_file),
                            source_file,
                        );
                    }
                }
                Vec::new()
            }
        }
    }

    pub fn use_parent<F, G>(
        &self,
        node: Option<ast::Node>,
        node_test: F,
        get_nodes: G,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::DocumentHighlight>
    where
        F: Fn(&ast::AstStore, ast::Node) -> bool,
        G: Fn(ast::Node, &ast::SourceFile) -> Vec<ast::Node>,
    {
        if let Some(node) = node
            && node_test(source_file.store(), node)
        {
            return self.highlight_spans(get_nodes(node, source_file), source_file);
        }
        Vec::new()
    }

    pub fn highlight_spans(
        &self,
        nodes: Vec<ast::Node>,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::DocumentHighlight> {
        if nodes.is_empty() {
            return Vec::new();
        }
        let mut highlights = Vec::new();
        let kind = lsproto::DocumentHighlightKind::Read;
        for node in &nodes {
            highlights.push(lsproto::DocumentHighlight {
                range: self.create_lsp_range_from_node(*node, source_file),
                kind: Some(kind),
            });
        }
        highlights
    }

    pub fn use_parent_ranges<F, G>(
        &self,
        node: Option<ast::Node>,
        node_test: F,
        get_ranges: G,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::DocumentHighlight>
    where
        F: Fn(&ast::AstStore, ast::Node) -> bool,
        G: Fn(ast::Node, &ast::SourceFile) -> Vec<core::TextRange>,
    {
        if let Some(node) = node
            && node_test(source_file.store(), node)
        {
            return self.highlight_ranges(get_ranges(node, source_file), source_file);
        }
        Vec::new()
    }

    pub fn highlight_ranges(
        &self,
        ranges: Vec<core::TextRange>,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::DocumentHighlight> {
        if ranges.is_empty() {
            return Vec::new();
        }
        let mut highlights = Vec::new();
        let kind = lsproto::DocumentHighlightKind::Read;
        for range in ranges {
            highlights.push(lsproto::DocumentHighlight {
                range: self.create_lsp_range_from_bounds(range.pos(), range.end(), source_file),
                kind: Some(kind),
            });
        }
        highlights
    }

    pub fn get_from_all_declarations(
        &self,
        node_test: impl Fn(&ast::AstStore, ast::Node) -> bool + Copy,
        keywords: &[ast::Kind],
        node: ast::Node,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::DocumentHighlight> {
        self.use_parent_ranges(
            source_file.store().parent(node),
            node_test,
            |decl, sf| {
                let mut symbol_decls = Vec::new();
                if ast::can_have_symbol(sf.store(), &decl) {
                    let declarations = source_node_symbol_declarations_snapshot_from_program(
                        self.get_program(),
                        sf,
                        decl,
                    );
                    if !declarations.is_empty() {
                        for d in declarations {
                            if node_test(sf.store(), d) {
                                'outer: for c in get_children_with_tokens(&d, sf) {
                                    for k in keywords {
                                        if c.kind == *k {
                                            symbol_decls.push(c.loc);
                                            break 'outer;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                symbol_decls
            },
            source_file,
        )
    }

    pub fn get_if_else_occurrences(
        &self,
        if_statement: ast::Node,
        source_file: &ast::SourceFile,
    ) -> Vec<lsproto::DocumentHighlight> {
        let keywords = get_if_else_keywords(if_statement, source_file);
        let kind = lsproto::DocumentHighlightKind::Read;
        let mut highlights = Vec::new();

        // We'd like to highlight else/ifs together if they are only separated by whitespace
        // (i.e. the keywords are separated by no comments, no newlines).
        let mut i = 0;
        while i < keywords.len() {
            if keywords[i].kind == ast::Kind::ElseKeyword && i < keywords.len() - 1 {
                let else_keyword = &keywords[i];
                let if_keyword = &keywords[i + 1]; // this *should* always be an 'if' keyword.
                let mut should_combine = true;

                // Avoid recalculating getStart() by iterating backwards.
                let if_token_start = if_keyword.loc.pos().max(0) as usize;
                for j in (else_keyword.loc.end().max(0) as usize..if_token_start).rev() {
                    if !stringutil::is_white_space_single_line(
                        source_file.text().as_bytes()[j] as char,
                    ) {
                        should_combine = false;
                        break;
                    }
                }
                if should_combine {
                    highlights.push(lsproto::DocumentHighlight {
                        range: self.create_lsp_range_from_bounds(
                            scanner::skip_trivia(
                                source_file.text(),
                                else_keyword.loc.pos().max(0) as usize,
                            ) as i32,
                            if_keyword.loc.end(),
                            source_file,
                        ),
                        kind: Some(kind),
                    });
                    i += 2; // skip the next keyword
                    continue;
                }
            }
            // Ordinary case: just highlight the keyword.
            highlights.push(lsproto::DocumentHighlight {
                range: self.create_lsp_range_from_bounds(
                    keywords[i].loc.pos(),
                    keywords[i].loc.end(),
                    source_file,
                ),
                kind: Some(kind),
            });
            i += 1;
        }
        highlights
    }
}

pub fn get_if_else_keywords(
    mut if_statement: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<SyntaxChild> {
    let store = source_file.store();
    // We may be at an if statement like those in the range below:
    //
    //   ```
    //   if (...) {
    //   } else [|if (...) {}|]
    //   ````
    //
    // Traverse upwards through all parent if-statements linked by their else-branches.
    while store
        .parent(if_statement)
        .as_ref()
        .is_some_and(|parent| ast::is_if_statement(store, *parent))
    {
        // See if the parent's `else` is actually the current `if` statement.
        let parenting_if_node = store.parent(if_statement).unwrap();
        let else_statement = store.else_statement(parenting_if_node);
        if !else_statement
            .as_ref()
            .is_some_and(|else_statement| *else_statement == if_statement)
        {
            break;
        }
        if_statement = parenting_if_node.clone();
    }

    let mut keywords = Vec::new();

    // Traverse back down through the else branches, aggregating if/else keywords of if-statements.
    loop {
        let children = get_children_with_tokens(&if_statement, source_file);
        if !children.is_empty() && children[0].kind == ast::Kind::IfKeyword {
            keywords.push(children[0]);
        }
        // Generally the 'else' keyword is second-to-last, so traverse backwards.
        for child in children.iter().rev() {
            if child.kind == ast::Kind::ElseKeyword {
                keywords.push(*child);
                break;
            }
        }
        let else_statement = store.else_statement(if_statement);
        if else_statement.is_none() || !ast::is_if_statement(store, else_statement.unwrap()) {
            break;
        }
        if_statement = else_statement.unwrap();
    }
    keywords
}

pub(crate) fn get_return_occurrences(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<core::TextRange> {
    let store = source_file.store();
    let parent = store.parent(node);
    let func_node = ast::find_ancestor(store, parent, |store, node| {
        ast::is_function_like(store, Some(node))
    });
    let Some(func_node) = func_node else {
        return Vec::new();
    };

    let mut keywords = Vec::new();
    let body = store.body(func_node);
    if let Some(body) = body {
        ast::for_each_return_statement(store, &body, |ret| {
            let keyword =
                astnav::find_child_of_kind_info(ret, ast::Kind::ReturnKeyword, source_file);
            if let Some(keyword) = keyword {
                keywords.push(astnav::range_from_token_info(keyword, source_file));
            }
            false // continue traversal
        });

        // Get all throw statements not in a try block
        let throw_statements = aggregate_owned_throw_statements(body, source_file);
        for throw in throw_statements {
            let keyword =
                astnav::find_child_of_kind_info(throw, ast::Kind::ThrowKeyword, source_file);
            if let Some(keyword) = keyword {
                keywords.push(astnav::range_from_token_info(keyword, source_file));
            }
        }
    }
    keywords
}

pub fn aggregate_owned_throw_statements<'a>(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<ast::Node> {
    let store = source_file.store();
    if ast::is_throw_statement(store, node) {
        return vec![node];
    }
    if ast::is_try_statement(store, node) {
        // Exceptions thrown within a try block lacking a catch clause are "owned" in the current context.
        let try_block = store.try_block(node);
        let catch_clause = store.catch_clause(node);
        let finally_block = store.finally_block(node);

        let mut result = Vec::new();
        if let Some(catch_clause) = catch_clause {
            result = aggregate_owned_throw_statements(catch_clause, source_file);
        } else if let Some(try_block) = try_block {
            result = aggregate_owned_throw_statements(try_block, source_file);
        }
        if let Some(finally_block) = finally_block {
            result.extend(aggregate_owned_throw_statements(finally_block, source_file));
        }
        return result;
    }
    // Do not cross function boundaries.
    if ast::is_function_like(store, Some(node)) {
        return Vec::new();
    }
    flat_map_children(node, source_file, aggregate_owned_throw_statements)
}

pub fn flat_map_children<T>(
    node: ast::Node,
    source_file: &ast::SourceFile,
    cb: fn(ast::Node, &ast::SourceFile) -> Vec<T>,
) -> Vec<T> {
    let mut result = Vec::new();

    let _ = source_file
        .store()
        .for_each_present_child(node, &mut |child| {
            let value = cb(child, source_file);
            result.extend(value);
            std::ops::ControlFlow::Continue(())
        });
    result
}

pub(crate) fn get_throw_occurrences(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<core::TextRange> {
    let owner = get_throw_statement_owner(source_file.store(), node);
    let Some(owner) = owner else {
        return Vec::new();
    };

    let mut keywords = Vec::new();

    // Aggregate all throw statements "owned" by this owner.
    let throw_statements = aggregate_owned_throw_statements(owner, source_file);
    for throw in throw_statements {
        let keyword = astnav::find_child_of_kind_info(throw, ast::Kind::ThrowKeyword, source_file);
        if let Some(keyword) = keyword {
            keywords.push(astnav::range_from_token_info(keyword, source_file));
        }
    }

    // If the "owner" is a function, then we equate 'return' and 'throw' statements in their
    // ability to "jump out" of the function, and include occurrences for both
    if ast::is_function_block(source_file.store(), Some(owner)) {
        ast::for_each_return_statement(source_file.store(), &owner, |ret| {
            let keyword =
                astnav::find_child_of_kind_info(ret, ast::Kind::ReturnKeyword, source_file);
            if let Some(keyword) = keyword {
                keywords.push(astnav::range_from_token_info(keyword, source_file));
            }
            false // continue traversal
        });
    }

    keywords
}

// For lack of a better name, this function takes a throw statement and returns the
// nearest ancestor that is a try-block (whose try statement has a catch clause),
// function-block, or source file.
pub(crate) fn get_throw_statement_owner(
    store: &ast::AstStore,
    child: ast::Node,
) -> Option<ast::Node> {
    let mut child = child;
    while let Some(parent) = store.parent(child) {
        if ast::is_function_block(store, Some(parent))
            || store.kind(parent) == ast::Kind::SourceFile
        {
            return Some(parent);
        }

        // A throw-statement is only owned by a try-statement if the try-statement has
        // a catch clause, and if the throw-statement occurs within the try block.
        if ast::is_try_statement(store, parent) {
            if store
                .try_block(parent)
                .as_ref()
                .is_some_and(|try_block| *try_block == child)
                && store.catch_clause(parent).is_some()
            {
                return Some(child);
            }
        }

        child = parent;
    }
    None
}

pub fn get_try_catch_finally_occurrence_ranges(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<core::TextRange> {
    let store = source_file.store();

    let mut keywords = Vec::new();
    if let Some(token) = lsutil::get_first_token_info(Some(node), source_file)
        && token.kind == ast::Kind::TryKeyword
    {
        keywords.push(token.loc);
    }

    if store.catch_clause(node).is_some() {
        if let Some(catch_token) =
            astnav::find_child_of_kind_info(node, ast::Kind::CatchKeyword, source_file)
        {
            keywords.push(astnav::range_from_token_info(catch_token, source_file));
        }
    }

    if store.finally_block(node).is_some() {
        if let Some(finally_keyword) =
            astnav::find_child_of_kind_info(node, ast::Kind::FinallyKeyword, source_file)
        {
            keywords.push(astnav::range_from_token_info(finally_keyword, source_file));
        }
    }

    keywords
}

pub fn get_switch_case_default_occurrence_ranges(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<core::TextRange> {
    let store = source_file.store();

    let mut keywords = Vec::new();
    if let Some(token) = lsutil::get_first_token_info(Some(node), source_file)
        && token.kind == ast::Kind::SwitchKeyword
    {
        keywords.push(token.loc);
    }

    let Some(case_block) = store.case_block(node) else {
        return keywords;
    };
    let Some(clauses) = store.clauses(case_block) else {
        return keywords;
    };
    for clause in clauses.iter() {
        if let Some(clause_token) = lsutil::get_first_token_info(Some(clause), source_file)
            && matches!(
                clause_token.kind,
                ast::Kind::CaseKeyword | ast::Kind::DefaultKeyword
            )
        {
            keywords.push(clause_token.loc);
        }

        let break_and_continue_statements =
            aggregate_all_break_and_continue_statements(clause, source_file);
        for statement in break_and_continue_statements {
            if store.kind(statement) == ast::Kind::BreakStatement
                && owns_break_or_continue_statement(source_file.store(), node, statement)
            {
                if let Some(token) = lsutil::get_first_token_info(Some(statement), source_file) {
                    keywords.push(token.loc);
                }
            }
        }
    }

    keywords
}

pub fn aggregate_all_break_and_continue_statements(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<ast::Node> {
    if ast::is_break_or_continue_statement(source_file.store(), node) {
        return vec![node];
    }
    if ast::is_function_like(source_file.store(), Some(node)) {
        return Vec::new();
    }
    flat_map_children(
        node,
        source_file,
        aggregate_all_break_and_continue_statements,
    )
}

pub fn owns_break_or_continue_statement(
    store: &ast::AstStore,
    owner: ast::Node,
    statement: ast::Node,
) -> bool {
    let actual_owner = get_break_or_continue_owner(store, statement);
    if actual_owner.is_none() {
        return false;
    }
    actual_owner.unwrap() == owner
}

pub fn get_break_or_continue_owner(
    store: &ast::AstStore,
    statement: ast::Node,
) -> Option<ast::Node> {
    ast::find_ancestor_or_quit(store, Some(statement), |store, node| {
        match store.kind(node) {
            ast::Kind::SwitchStatement => {
                if store.kind(statement) == ast::Kind::ContinueStatement {
                    return ast::FindAncestorResult::False;
                }
                let label = store.label(statement);
                if label.is_none() || is_labeled_by(store, node, &store.text(label.unwrap())) {
                    return ast::FindAncestorResult::True;
                }
                ast::FindAncestorResult::False
            }
            ast::Kind::ForStatement
            | ast::Kind::ForInStatement
            | ast::Kind::ForOfStatement
            | ast::Kind::WhileStatement
            | ast::Kind::DoStatement => {
                // If the statement is labeled, check if the node is labeled by the statement's label.
                let label = store.label(statement);
                if label.is_none() || is_labeled_by(store, node, &store.text(label.unwrap())) {
                    return ast::FindAncestorResult::True;
                }
                ast::FindAncestorResult::False
            }
            _ => {
                // Don't cross function boundaries.
                if ast::is_function_like(store, Some(node)) {
                    return ast::FindAncestorResult::Quit;
                }
                ast::FindAncestorResult::False
            }
        }
    })
}

// Whether or not a 'node' is preceded by a label of the given string.
// Note: 'node' cannot be a SourceFile.
pub(crate) fn is_labeled_by(store: &ast::AstStore, node: ast::Node, label_name: &str) -> bool {
    ast::find_ancestor_or_quit(store, store.parent(node), |store, owner| {
        if !ast::is_labeled_statement(store, owner) {
            return ast::FindAncestorResult::Quit;
        }
        if store
            .label(owner)
            .is_some_and(|label| store.text(label) == label_name)
        {
            return ast::FindAncestorResult::True;
        }
        ast::FindAncestorResult::False
    })
    .is_some()
}

fn range_from_node_token(node: ast::Node, source_file: &ast::SourceFile) -> core::TextRange {
    core::new_text_range(
        scanner::get_token_pos_of_node(&node, source_file, false) as i32,
        source_file.store().loc(node).end(),
    )
}

pub fn get_break_or_continue_statement_occurrence_ranges(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<core::TextRange> {
    let store = source_file.store();
    if let Some(owner) = get_break_or_continue_owner(source_file.store(), node) {
        match store.kind(owner) {
            ast::Kind::ForStatement
            | ast::Kind::ForInStatement
            | ast::Kind::ForOfStatement
            | ast::Kind::DoStatement
            | ast::Kind::WhileStatement => {
                return get_loop_break_continue_occurrence_ranges(owner, source_file);
            }
            ast::Kind::SwitchStatement => {
                return get_switch_case_default_occurrence_ranges(owner, source_file);
            }
            _ => {}
        }
    }
    Vec::new()
}

pub fn get_loop_break_continue_occurrence_ranges(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<core::TextRange> {
    let mut keywords = Vec::new();

    let loop_tokens = get_children_with_tokens(&node, source_file);
    if let Some(token) = loop_tokens.first()
        && matches!(
            token.kind,
            ast::Kind::ForKeyword | ast::Kind::DoKeyword | ast::Kind::WhileKeyword
        )
    {
        keywords.push(token.loc);
        if source_file.store().kind(node) == ast::Kind::DoStatement {
            for token in loop_tokens.iter().rev() {
                if token.kind == ast::Kind::WhileKeyword {
                    keywords.push(token.loc);
                    break;
                }
            }
        }
    }

    let break_and_continue_statements =
        aggregate_all_break_and_continue_statements(node, source_file);
    for statement in break_and_continue_statements {
        if owns_break_or_continue_statement(source_file.store(), node, statement) {
            let statement_children = get_children_with_tokens(&statement, source_file);
            if let Some(token) = statement_children.first()
                && matches!(
                    token.kind,
                    ast::Kind::BreakKeyword | ast::Kind::ContinueKeyword
                )
            {
                keywords.push(token.loc);
            }
        }
    }

    keywords
}

pub fn get_async_and_await_occurrence_ranges(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<core::TextRange> {
    let store = source_file.store();
    let fun = ast::get_containing_function(store, &node);
    let Some(fun) = fun else {
        return Vec::new();
    };

    let mut keywords = Vec::new();

    for modifier in store.modifier_nodes(fun) {
        if store.kind(modifier) == ast::Kind::AsyncKeyword {
            keywords.push(range_from_node_token(modifier, source_file));
        }
    }

    let _ = store.for_each_present_child(fun, &mut |child| {
        traverse_without_crossing_function(child, source_file, &mut |child| {
            if ast::is_await_expression(store, child) {
                if let Some(token) = lsutil::get_first_token_info(Some(child), source_file)
                    && token.kind == ast::Kind::AwaitKeyword
                {
                    keywords.push(token.loc);
                }
            }
        });
        std::ops::ControlFlow::Continue(())
    });

    keywords
}

pub fn get_yield_occurrence_ranges(
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<core::TextRange> {
    let store = source_file.store();
    let parent = store.parent(node);
    let parent_func = ast::find_ancestor(store, parent, |store, node| {
        ast::is_function_like(store, Some(node))
    });
    let Some(parent_func) = parent_func else {
        return Vec::new();
    };

    let mut keywords = Vec::new();

    let _ = store.for_each_present_child(parent_func, &mut |child| {
        traverse_without_crossing_function(child, source_file, &mut |child| {
            if ast::is_yield_expression(store, child) {
                if let Some(token) = lsutil::get_first_token_info(Some(child), source_file)
                    && token.kind == ast::Kind::YieldKeyword
                {
                    keywords.push(token.loc);
                }
            }
        });
        std::ops::ControlFlow::Continue(())
    });

    keywords
}

pub fn traverse_without_crossing_function(
    node: ast::Node,
    source_file: &ast::SourceFile,
    cb: &mut dyn FnMut(ast::Node),
) {
    let store = source_file.store();
    cb(node);
    if !ast::is_function_like(store, Some(node))
        && !ast::is_class_like(store, node)
        && !ast::is_interface_declaration(store, node)
        && !ast::is_module_declaration(store, node)
        && !ast::is_type_alias_declaration(store, node)
        && !ast::is_type_node(store, node)
    {
        let _ = store.for_each_present_child(node, &mut |child| {
            traverse_without_crossing_function(child, source_file, cb);
            std::ops::ControlFlow::Continue(())
        });
    }
}

pub fn get_modifier_occurrences(
    kind: ast::Kind,
    node: ast::Node,
    source_file: &ast::SourceFile,
) -> Vec<ast::Node> {
    let mut result = Vec::new();

    let nodes_to_search =
        get_nodes_to_search_for_modifier(source_file.store(), node, ast::modifier_to_flag(kind));
    for n in nodes_to_search {
        let modifier = find_modifier(source_file.store(), n, kind);
        if let Some(modifier) = modifier {
            result.push(modifier);
        }
    }
    result
}

pub fn get_nodes_to_search_for_modifier(
    store: &ast::AstStore,
    declaration: ast::Node,
    modifier_flag: ast::ModifierFlags,
) -> Vec<ast::Node> {
    let mut result = Vec::new();

    let container = store.parent(declaration);
    let Some(container) = container else {
        return Vec::new();
    };

    // Types of node whose children might have modifiers.
    match store.kind(container) {
        ast::Kind::ModuleBlock
        | ast::Kind::SourceFile
        | ast::Kind::Block
        | ast::Kind::CaseClause
        | ast::Kind::DefaultClause => {
            // Container is either a class declaration or the declaration is a classDeclaration
            if (modifier_flag & ast::ModifierFlags::Abstract) != ast::ModifierFlags::None
                && ast::is_class_declaration(store, declaration)
            {
                if let Some(members) = store.members(declaration) {
                    result.extend(members.iter());
                }
                result.push(declaration);
                result
            } else {
                if let Some(statements) = store.statements(container) {
                    result.extend(statements.iter());
                }
                result
            }
        }
        ast::Kind::Constructor | ast::Kind::MethodDeclaration | ast::Kind::FunctionDeclaration => {
            // Parameters and, if inside a class, also class members
            if let Some(parameters) = store.parameters(container) {
                result.extend(parameters.iter());
            }
            if let Some(parent) = store.parent(container)
                && ast::is_class_like(store, parent)
            {
                if let Some(members) = store.members(parent) {
                    result.extend(members.iter());
                }
            }
            result
        }
        ast::Kind::ClassDeclaration
        | ast::Kind::ClassExpression
        | ast::Kind::InterfaceDeclaration
        | ast::Kind::TypeLiteral => {
            let nodes: Vec<_> = store
                .members(container)
                .map(|nodes| nodes.iter().collect())
                .unwrap_or_default();
            result.extend(nodes.iter().copied());
            // If we're an accessibility modifier, we're in an instance member and should search
            // the constructor's parameter list for instance members as well.
            if (modifier_flag
                & (ast::ModifierFlags::AccessibilityModifier | ast::ModifierFlags::Readonly))
                != ast::ModifierFlags::None
            {
                let mut constructor = None;

                for member in nodes {
                    if ast::is_constructor_declaration(store, member) {
                        constructor = Some(member);
                        break;
                    }
                }
                if let Some(constructor) = constructor {
                    if let Some(parameters) = store.parameters(constructor) {
                        result.extend(parameters.iter());
                    }
                }
            } else if (modifier_flag & ast::ModifierFlags::Abstract) != ast::ModifierFlags::None {
                result.push(container);
            }
            result
        }
        _ => {
            // Syntactically invalid positions or unsupported containers
            Vec::new()
        }
    }
}

pub fn find_modifier(store: &ast::AstStore, node: ast::Node, kind: ast::Kind) -> Option<ast::Node> {
    for modifier in store.modifier_nodes(node) {
        if store.kind(modifier) == kind {
            return Some(modifier);
        }
    }
    None
}
