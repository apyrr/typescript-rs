use std::{collections::HashMap, ops::ControlFlow};

use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_format as format;
use ts_lsproto as lsproto;
use ts_printer as printer;
use ts_scanner as scanner;
use ts_stringutil as stringutil;

use crate::lsutil;

use super::{
    LEADING_TRIVIA_OPTION_EXCLUDE, LEADING_TRIVIA_OPTION_INCLUDE_ALL,
    LEADING_TRIVIA_OPTION_START_LINE, LeadingTriviaOption, NodeOptions,
    TRAILING_TRIVIA_OPTION_EXCLUDE, TRAILING_TRIVIA_OPTION_EXCLUDE_WHITESPACE,
    TRAILING_TRIVIA_OPTION_INCLUDE, Tracker, TrackerEdit, TrackerEditKind, TrailingTriviaOption,
};

fn unset_originals_for_tree(emit_context: &mut printer::EmitContext, node: ast::Node) {
    emit_context.unset_original(&node);
    let children = {
        let store = emit_context.factory.node_factory.store();
        let mut children = Vec::new();
        let _ = store.for_each_child(node, |child| {
            if let Some(child) = child {
                children.push(child);
            }
            ControlFlow::Continue(())
        });
        children
    };
    for child in children {
        unset_originals_for_tree(emit_context, child);
    }
}

impl<'a> Tracker<'a> {
    pub fn get_text_changes_from_changes(&mut self) -> HashMap<String, Vec<lsproto::TextEdit>> {
        let mut changes = HashMap::new();
        let mut changes_map = std::mem::take(&mut self.changes.m);
        for changes_in_file in changes_map.values_mut() {
            let Some(source_file) = changes_in_file.first().map(|change| change.source_file) else {
                continue;
            };
            // order changes by start position
            // If the start position is the same, put the shorter range first, since an empty range (x, x) may precede (x, y) but not vice-versa.
            changes_in_file.sort_by(|a, b| lsproto::compare_ranges(a.range, b.range));
            // verify that change intervals do not overlap, except possibly at end points.
            for i in 0..changes_in_file.len().saturating_sub(1) {
                if lsproto::compare_positions(
                    changes_in_file[i].range.end,
                    changes_in_file[i + 1].range.start,
                ) == std::cmp::Ordering::Greater
                {
                    // assert change[i].End <= change[i + 1].Start
                    panic!(
                        "changes overlap: {:?} and {:?}",
                        changes_in_file[i].range,
                        changes_in_file[i + 1].range
                    );
                }
            }

            let text_changes: Vec<_> = changes_in_file
                .iter()
                .filter_map(|change| {
                    // !!! targetSourceFile

                    let new_text = self.compute_new_text(change, source_file, source_file);
                    // span := createTextSpanFromRange(c.Range)
                    // !!!
                    // Filter out redundant changes.
                    // if (span.length == newText.length && stringContainsAt(targetSourceFile.text, newText, span.start)) { return nil }

                    Some(lsproto::TextEdit {
                        new_text,
                        range: change.range,
                    })
                })
                .collect();

            if !text_changes.is_empty() {
                changes.insert(source_file.file_name().to_string(), text_changes);
            }
        }
        self.changes.m = changes_map;
        changes
    }

    pub fn compute_new_text(
        &mut self,
        change: &TrackerEdit<'a>,
        target_source_file: &'a ast::SourceFile,
        source_file: &'a ast::SourceFile,
    ) -> String {
        match change.kind {
            TrackerEditKind::Remove => return String::new(),
            TrackerEditKind::Text => return change.new_text.clone(),
            _ => {}
        }

        let pos =
            self.converters
                .line_and_character_to_position(source_file, change.range.start) as i32;
        let new_line = self.new_line.clone();
        let mut format_node = |n: ast::Node| {
            self.get_formatted_text_of_node(
                n,
                target_source_file,
                source_file,
                pos,
                change.options.clone(),
            )
        };

        let text = match change.kind {
            TrackerEditKind::ReplaceWithMultipleNodes => {
                let joiner = if change.options.joiner.is_empty() {
                    new_line.clone()
                } else {
                    change.options.joiner.clone()
                };
                change
                    .nodes
                    .iter()
                    .map(|n| format_node(*n).trim_end_matches(&new_line).to_string())
                    .collect::<Vec<_>>()
                    .join(&joiner)
            }
            TrackerEditKind::ReplaceWithSingleNode => format_node(change.node.unwrap()),
            _ => panic!(
                "change kind {:?} should have been handled earlier",
                change.kind
            ),
        };
        // strip initial indentation (spaces or tabs) if text will be inserted in the middle of the line
        let mut no_indent = text.clone();
        if !(change.options.indentation.is_some()
            || format::get_line_start_position_for_position(pos, target_source_file) == pos)
        {
            no_indent = text.trim_start_matches(char::is_whitespace).to_string();
        }
        change.options.prefix.clone()
            + &no_indent
            + if no_indent.ends_with(&change.options.suffix) {
                ""
            } else {
                &change.options.suffix
            }
    }

    /** Note: this may mutate `nodeIn`. */
    pub fn get_formatted_text_of_node(
        &mut self,
        node_in: ast::Node,
        target_source_file: &'a ast::SourceFile,
        source_file: &'a ast::SourceFile,
        pos: i32,
        options: NodeOptions,
    ) -> String {
        let (text, source_file_like, source_file_like_node) =
            self.get_nonformatted_text(node_in, target_source_file);
        // !!! if (validate) validate(node, text);
        let format_options =
            get_format_code_settings_for_writing(self.format_settings.clone(), target_source_file);

        let initial_indentation = if options.indentation.is_none() {
            format::get_indentation(
                pos,
                source_file,
                format_options.clone(),
                options.prefix == self.new_line
                    || format::get_line_start_position_for_position(pos, target_source_file) == pos,
            )
        } else {
            options.indentation.unwrap()
        };

        let delta = if let Some(delta) = options.delta {
            delta
        } else if format_options.editor_settings.indent_size != 0
            && format::should_indent_child_node(
                format_options.clone(),
                &source_file_like_node,
                None,
                &source_file_like,
                false,
            )
        {
            format_options.editor_settings.indent_size
        } else {
            0
        };

        let source_file_like_node = source_file_like.as_node();
        let changes = format::format_node_given_indentation(
            &self.ctx,
            &source_file_like_node,
            &source_file_like,
            target_source_file.language_variant(),
            initial_indentation,
            delta,
        );
        core::apply_bulk_edits(&text, &changes)
    }

    pub fn get_nonformatted_text(
        &mut self,
        node: ast::Node,
        source_file: &'a ast::SourceFile,
    ) -> (String, ast::SourceFile, ast::Node) {
        let mut writer = printer::new_change_tracker_writer(
            self.new_line.clone(),
            self.format_settings.editor_settings.indent_size,
        );
        let shared_writer = printer::share_text_writer(Box::new(writer.clone()));
        let mut emit_context = self.emit_context.fork();
        let tracker_factory_store_id = self.node_factory.store().store_id();
        let (node_to_print, printed_from_emit_factory) =
            if node.store_id() == tracker_factory_store_id {
                let source_store = self.node_factory.store();
                let cloned = emit_context
                    .factory
                    .clone_node_with_hooks(source_store, node);
                emit_context.copy_emit_metadata_for_cloned_tree(source_store, node, cloned);
                unset_originals_for_tree(&mut emit_context, cloned);
                (cloned, true)
            } else {
                (node, false)
            };
        let mut printer = printer::new_printer(
            printer::PrinterOptions {
                new_line: core::get_new_line_kind(&self.new_line),
                never_ascii_escape: true,
                preserve_source_newlines: true,
                terminate_unterminated_literals: true,
                ..Default::default()
            },
            writer.get_print_handlers(),
            Some(emit_context),
        );
        printer.write_node(Some(&node_to_print), Some(source_file), shared_writer, None);
        let emit_context = printer.into_emit_context();

        let mut text = printer::EmitTextWriter::string(&mut writer);
        if text.ends_with(&self.new_line) {
            text.truncate(text.len() - self.new_line.len());
        }
        let position_source = if printed_from_emit_factory {
            emit_context.factory.node_factory.store()
        } else {
            source_file.store()
        };
        let mut source_file_like_factory = ast::NodeFactory::default();
        let node_out = writer.assign_positions_to_node(
            position_source,
            &node_to_print,
            &mut source_file_like_factory,
        );
        let node_loc = source_file_like_factory.loc(node_out);
        let eof_token = source_file_like_factory.new_token(ast::Kind::EndOfFile);
        let node_list = source_file_like_factory.new_node_list(node_loc, node_loc, vec![node_out]);
        let eof_loc = core::new_text_range(node_loc.end(), node_loc.end());
        source_file_like_factory.place_change_tracker_node(eof_token, eof_loc);
        let source_file_like_node = source_file_like_factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: source_file.file_name().to_string(),
                path: source_file.path().to_owned(),
                ..Default::default()
            },
            text.clone(),
            node_list,
            Some(eof_token),
        );
        source_file_like_factory.place_change_tracker_node(source_file_like_node, node_loc);
        let source_file_like = source_file_like_factory.finish_parsed_source_file(
            source_file_like_node,
            ast::ParsedSourceFileMetadata::default(),
        );
        (text, source_file_like, node_out)
    }

    // method on the changeTracker because use of converters
    // GetAdjustedRange computes the adjusted range for a node in a source file, accounting for trivia.
    pub fn get_adjusted_range(
        &self,
        source_file: &ast::SourceFile,
        start_node: ast::Node,
        end_node: ast::Node,
        leading_option: LeadingTriviaOption,
        trailing_option: TrailingTriviaOption,
    ) -> lsproto::Range {
        self.converters.to_lsp_range(
            source_file,
            core::new_text_range(
                self.get_adjusted_start_position(source_file, start_node, leading_option, false),
                self.get_adjusted_end_position(source_file, end_node, trailing_option),
            ),
        )
    }

    // method on the changeTracker because use of converters
    pub fn get_adjusted_start_position(
        &self,
        source_file: &ast::SourceFile,
        node: ast::Node,
        leading_option: LeadingTriviaOption,
        has_trailing_comment: bool,
    ) -> i32 {
        let start = astnav::get_start_of_node(node, source_file);
        let start_of_line_pos = format::get_line_start_position_for_position(start, source_file);

        match leading_option {
            LEADING_TRIVIA_OPTION_EXCLUDE => return start,
            LEADING_TRIVIA_OPTION_START_LINE => {
                if source_file
                    .store()
                    .loc(node)
                    .contains_inclusive(start_of_line_pos)
                {
                    return start_of_line_pos;
                }
                return start;
            }
            _ => {}
        }

        let full_start = source_file.store().loc(node).pos();
        if full_start == start {
            return start;
        }
        let line_starts = core::compute_ecma_line_starts(source_file.text());
        let full_start_line_index =
            scanner::compute_line_of_position(&line_starts, full_start as usize);
        let full_start_line_pos = line_starts[full_start_line_index] as i32;
        if start_of_line_pos == full_start_line_pos {
            // full start and start of the node are on the same line
            //   a,     b;
            //    ^     ^
            //    |   start
            // fullstart
            // when b is replaced - we usually want to keep the leading trvia
            // when b is deleted - we delete it
            if leading_option == LEADING_TRIVIA_OPTION_INCLUDE_ALL {
                return full_start;
            }
            return start;
        }

        // if node has a trailing comments, use comment end position as the text has already been included.
        if has_trailing_comment {
            // Check first for leading comments as if the node is the first import, we want to exclude the trivia;
            // otherwise we get the trailing comments.
            let mut comments: Vec<_> =
                scanner::get_leading_comment_ranges(source_file.text(), full_start);
            if comments.is_empty() {
                comments = scanner::get_trailing_comment_ranges(source_file.text(), full_start);
            }
            if !comments.is_empty() {
                let options = scanner::SkipTriviaOptions {
                    stop_after_line_break: true,
                    stop_at_comments: true,
                };
                return scanner::skip_trivia_ex(
                    source_file.text(),
                    comments[0].end() as usize,
                    Some(&options),
                ) as i32;
            }
        }

        // get start position of the line following the line that contains fullstart position
        // (but only if the fullstart isn't the very beginning of the file)
        let next_line_start = if full_start > 0 { 1 } else { 0 };
        let mut adjusted_start_position =
            line_starts[full_start_line_index + next_line_start] as i32;
        // skip whitespaces/newlines
        let options = scanner::SkipTriviaOptions {
            stop_after_line_break: false,
            stop_at_comments: true,
        };
        adjusted_start_position = scanner::skip_trivia_ex(
            source_file.text(),
            adjusted_start_position as usize,
            Some(&options),
        ) as i32;
        line_starts
            [scanner::compute_line_of_position(&line_starts, adjusted_start_position as usize)]
            as i32
    }

    // method on the changeTracker because of converters
    // Return the end position of a multiline comment of it is on another line; otherwise returns `undefined`;
    pub fn get_end_position_of_multiline_trailing_comment(
        &self,
        source_file: &ast::SourceFile,
        node: ast::Node,
        trailing_opt: TrailingTriviaOption,
    ) -> i32 {
        if trailing_opt == TRAILING_TRIVIA_OPTION_INCLUDE {
            // If the trailing comment is a multiline comment that extends to the next lines,
            // return the end of the comment and track it for the next nodes to adjust.
            let line_starts = core::compute_ecma_line_starts(source_file.text());
            let node_end_line = scanner::compute_line_of_position(
                &line_starts,
                source_file.store().loc(node).end() as usize,
            );
            for comment in scanner::get_trailing_comment_ranges(
                source_file.text(),
                source_file.store().loc(node).end(),
            ) {
                // Single line can break the loop as trivia will only be this line.
                // Comments on subsequent lines are also ignored.
                if comment.kind() == ast::Kind::SingleLineCommentTrivia
                    || scanner::compute_line_of_position(&line_starts, comment.pos() as usize)
                        > node_end_line
                {
                    break;
                }

                // Get the end line of the comment and compare against the end line of the node.
                // If the comment end line position and the multiline comment extends to multiple lines,
                // then is safe to return the end position.
                if scanner::compute_line_of_position(&line_starts, comment.end() as usize)
                    > node_end_line
                {
                    let options = scanner::SkipTriviaOptions {
                        stop_after_line_break: true,
                        stop_at_comments: true,
                    };
                    return scanner::skip_trivia_ex(
                        source_file.text(),
                        comment.end() as usize,
                        Some(&options),
                    ) as i32;
                }
            }
        }

        0
    }

    // method on the changeTracker because of converters
    pub fn get_adjusted_end_position(
        &self,
        source_file: &'a ast::SourceFile,
        node: ast::Node,
        trailing_trivia_option: TrailingTriviaOption,
    ) -> i32 {
        if trailing_trivia_option == TRAILING_TRIVIA_OPTION_EXCLUDE {
            return source_file.store().loc(node).end();
        }
        if trailing_trivia_option == TRAILING_TRIVIA_OPTION_EXCLUDE_WHITESPACE {
            let mut comments: Vec<_> = scanner::get_trailing_comment_ranges(
                source_file.text(),
                source_file.store().loc(node).end(),
            );
            comments.extend(scanner::get_leading_comment_ranges(
                source_file.text(),
                source_file.store().loc(node).end(),
            ));
            if !comments.is_empty() {
                let real_end = comments[comments.len() - 1].end();
                if real_end != 0 {
                    return real_end;
                }
            }
            return source_file.store().loc(node).end();
        }

        let multiline_end_position = self.get_end_position_of_multiline_trailing_comment(
            source_file,
            node,
            trailing_trivia_option,
        );
        if multiline_end_position != 0 {
            return multiline_end_position;
        }

        let options = scanner::SkipTriviaOptions {
            stop_after_line_break: true,
            stop_at_comments: false,
        };
        let new_end = scanner::skip_trivia_ex(
            source_file.text(),
            source_file.store().loc(node).end() as usize,
            Some(&options),
        ) as i32;

        if new_end != source_file.store().loc(node).end()
            && (trailing_trivia_option == TRAILING_TRIVIA_OPTION_INCLUDE
                || stringutil::is_line_break(
                    source_file.text().as_bytes()[new_end as usize - 1] as char,
                ))
        {
            return new_end;
        }
        source_file.store().loc(node).end()
    }

    pub fn get_insertion_position_at_source_file_top(
        &self,
        source_file: &'a ast::SourceFile,
    ) -> i32 {
        let mut last_prologue = None;
        let store = source_file.store();
        let statements: Vec<_> = source_file.statements_view().iter().collect();
        for node in &statements {
            if ast::is_prologue_directive(store, *node) {
                last_prologue = Some(node);
            } else {
                break;
            }
        }

        let mut position = 0;
        let text = source_file.text();
        fn advance_past_line_break(position: &mut i32, text: &str) {
            if *position >= text.len() as i32 {
                return;
            }
            let ch = text.as_bytes()[*position as usize] as char;
            if stringutil::is_line_break(ch) {
                *position += 1;
                if *position < text.len() as i32
                    && ch == '\r'
                    && text.as_bytes()[*position as usize] as char == '\n'
                {
                    *position += 1;
                }
            }
        }
        if let Some(last_prologue) = last_prologue {
            position = store.loc(*last_prologue).end();
            advance_past_line_break(&mut position, text);
            return position;
        }

        let shebang = scanner::get_shebang(text);
        if !shebang.is_empty() {
            position = shebang.len() as i32;
            advance_past_line_break(&mut position, text);
        }

        let ranges: Vec<_> = scanner::get_leading_comment_ranges(text, position);
        if ranges.is_empty() {
            return position;
        }
        // Find the first attached comment to the first node and add before it
        let mut last_comment = None;
        let mut pinned_or_triple_slash = false;
        let mut first_node_line = -1;

        let len_statements = statements.len();
        let line_map = core::compute_ecma_line_starts(source_file.text());
        for r in ranges {
            if r.kind() == ast::Kind::MultiLineCommentTrivia {
                if printer::is_pinned_comment(text, r) {
                    last_comment = Some(r);
                    pinned_or_triple_slash = true;
                    continue;
                }
            } else if printer::is_recognized_triple_slash_comment(text, r) {
                last_comment = Some(r);
                pinned_or_triple_slash = true;
                continue;
            }

            if let Some(last_comment_value) = last_comment {
                // Always insert after pinned or triple slash comments
                if pinned_or_triple_slash {
                    break;
                }

                // There was a blank line between the last comment and this comment.
                // This comment is not part of the copyright comments
                let comment_line = scanner::compute_line_of_position(&line_map, r.pos() as usize);
                let last_comment_end_line =
                    scanner::compute_line_of_position(&line_map, last_comment_value.end() as usize);
                if comment_line >= last_comment_end_line + 2 {
                    break;
                }
            }

            if len_statements > 0 {
                if first_node_line == -1 {
                    first_node_line = scanner::compute_line_of_position(
                        &line_map,
                        astnav::get_start_of_node(statements[0], source_file) as usize,
                    ) as i32;
                }
                let comment_end_line =
                    scanner::compute_line_of_position(&line_map, r.end() as usize) as i32;
                if first_node_line < comment_end_line + 2 {
                    break;
                }
            }
            last_comment = Some(r);
            pinned_or_triple_slash = false;
        }

        if let Some(last_comment) = last_comment {
            position = last_comment.end();
            advance_past_line_break(&mut position, text);
        }
        position
    }
}

pub fn get_format_code_settings_for_writing(
    mut options: lsutil::FormatCodeSettings,
    source_file: &ast::SourceFile,
) -> lsutil::FormatCodeSettings {
    let should_auto_detect_semicolon_preference =
        options.semicolons == lsutil::SemicolonPreference::Ignore;
    let should_remove_semicolons = options.semicolons == lsutil::SemicolonPreference::Remove
        || should_auto_detect_semicolon_preference
            && !lsutil::probably_uses_semicolons(source_file);
    if should_remove_semicolons {
        options.semicolons = lsutil::SemicolonPreference::Remove;
    }

    options
}

// ============= utilities =============

pub fn has_comments_before_line_break(text: &str, start: i32) -> bool {
    for ch in text[start as usize..].chars() {
        if !stringutil::is_white_space_single_line(ch) {
            return ch == '/';
        }
    }
    false
}

pub(crate) fn need_semicolon_between(
    a_store: &ast::AstStore,
    a: ast::Node,
    b_store: &ast::AstStore,
    b: ast::Node,
) -> bool {
    (ast::is_property_signature_declaration(a_store, a) || ast::is_property_declaration(a_store, a))
        && ast::is_class_or_type_element(b_store, &b)
        && b_store
            .name(b)
            .is_some_and(|name| b_store.kind(name) == ast::Kind::ComputedPropertyName)
        || ast::is_statement_but_not_declaration(a_store, a)
            && ast::is_statement_but_not_declaration(b_store, b)
    // TODO: only if b would start with a `(` or `[`
}
