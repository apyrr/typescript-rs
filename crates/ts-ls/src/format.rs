use ts_ast as ast;
use ts_astnav as astnav;
use ts_core as core;
use ts_format as format;
use ts_lsproto as lsproto;
use ts_scanner as scanner;

use crate::LanguageService;
use crate::lsutil;

impl LanguageService<'_> {
    pub fn to_lsproto_text_edits(
        &self,
        file: &ast::SourceFile,
        changes: Vec<core::TextChange>,
    ) -> Vec<lsproto::TextEdit> {
        let mut result = Vec::with_capacity(changes.len());
        for change in changes {
            result.push(lsproto::TextEdit {
                new_text: change.new_text,
                range: self.create_lsp_range_from_bounds(
                    change.text_range.pos(),
                    change.text_range.end(),
                    file,
                ),
                ..Default::default()
            });
        }
        result
    }

    pub fn provide_format_document(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        options: &lsproto::FormattingOptions,
    ) -> Result<lsproto::DocumentFormattingResponse, core::Error> {
        let (_, file) = self.get_program_and_file(document_uri);
        let format_opts = lsutil::from_ls_format_options(self.format_options(), options);
        let edits = self.to_lsproto_text_edits(
            file,
            self.get_formatting_edits_for_document(ctx.clone(), file, format_opts),
        );
        Ok(lsproto::TextEditsOrNull {
            text_edits: Some(edits.into_iter().map(Some).collect()),
            ..Default::default()
        })
    }

    pub fn provide_format_document_range(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        options: &lsproto::FormattingOptions,
        range: lsproto::Range,
    ) -> Result<lsproto::DocumentRangeFormattingResponse, core::Error> {
        let (_, file) = self.get_program_and_file(document_uri);
        let format_opts = lsutil::from_ls_format_options(self.format_options(), options);
        let edits = self.to_lsproto_text_edits(
            file,
            self.get_formatting_edits_for_range(
                ctx.clone(),
                file,
                format_opts,
                self.converters.from_lsp_range(file, range),
            ),
        );
        Ok(lsproto::TextEditsOrNull {
            text_edits: Some(edits.into_iter().map(Some).collect()),
            ..Default::default()
        })
    }

    pub fn provide_format_document_on_type(
        &self,
        ctx: &core::Context,
        document_uri: lsproto::DocumentUri,
        options: &lsproto::FormattingOptions,
        position: lsproto::Position,
        character: String,
    ) -> Result<lsproto::DocumentOnTypeFormattingResponse, core::Error> {
        let (_, file) = self.get_program_and_file(document_uri);
        let format_opts = lsutil::from_ls_format_options(self.format_options(), options);
        let edits = self.to_lsproto_text_edits(
            file,
            self.get_formatting_edits_after_keystroke(
                ctx.clone(),
                file,
                format_opts,
                self.converters
                    .line_and_character_to_position(file, position) as i32,
                &character,
            ),
        );
        Ok(lsproto::TextEditsOrNull {
            text_edits: Some(edits.into_iter().map(Some).collect()),
            ..Default::default()
        })
    }

    pub fn get_formatting_edits_for_range(
        &self,
        _ctx: core::Context,
        file: &ast::SourceFile,
        options: lsutil::FormatCodeSettings,
        range: core::TextRange,
    ) -> Vec<core::TextChange> {
        let ctx = format::with_format_code_settings(
            format::Context::new(),
            options.clone(),
            options.new_line_character.clone(),
        );
        format::format_selection(&ctx, file, range.pos(), range.end())
    }

    pub fn get_formatting_edits_for_document(
        &self,
        _ctx: core::Context,
        file: &ast::SourceFile,
        options: lsutil::FormatCodeSettings,
    ) -> Vec<core::TextChange> {
        let ctx = format::with_format_code_settings(
            format::Context::new(),
            options.clone(),
            options.new_line_character.clone(),
        );
        format::format_document(&ctx, file)
    }

    pub fn get_formatting_edits_after_keystroke(
        &self,
        _ctx: core::Context,
        file: &ast::SourceFile,
        options: lsutil::FormatCodeSettings,
        position: i32,
        key: &str,
    ) -> Vec<core::TextChange> {
        let ctx = format::with_format_code_settings(
            format::Context::new(),
            options.clone(),
            options.new_line_character.clone(),
        );

        let token_at_position = astnav::get_token_at_position(file, position);
        if is_in_comment(file, position, token_at_position.as_ref()).is_none() {
            match key {
                "{" => return format::format_on_opening_curly(&ctx, file, position),
                "}" => return format::format_on_closing_curly(&ctx, file, position),
                ";" => return format::format_on_semicolon(&ctx, file, position),
                "\n" => return format::format_on_enter(&ctx, file, position),
                _ => return Vec::new(),
            }
        }
        Vec::new()
    }
}

pub fn is_in_comment(
    file: &ast::SourceFile,
    position: i32,
    token_at_position: Option<&ast::Node>,
) -> Option<ast::CommentRange> {
    let preceding_token =
        astnav::find_preceding_token_info(file, position).map(CommentTokenInfo::from_token_info);
    let token_at_position = get_token_at_position_info_for_comment(file, position, preceding_token)
        .or_else(|| {
            token_at_position.map(|token| CommentTokenInfo::from_node(file.store(), *token))
        });
    get_range_of_enclosing_comment_worker(file, position, preceding_token, token_at_position)
}

#[derive(Clone, Copy)]
struct CommentTokenInfo {
    node: Option<ast::Node>,
    kind: ast::Kind,
    loc: core::TextRange,
}

impl CommentTokenInfo {
    fn from_node(store: &ast::AstStore, node: ast::Node) -> Self {
        Self {
            node: Some(node),
            kind: store.kind(node),
            loc: store.loc(node),
        }
    }

    fn from_token_info(token: astnav::TokenInfo) -> Self {
        Self {
            node: token.node,
            kind: token.kind,
            loc: token.loc,
        }
    }
}

fn get_token_at_position_info_for_comment(
    file: &ast::SourceFile,
    position: i32,
    preceding_token: Option<CommentTokenInfo>,
) -> Option<CommentTokenInfo> {
    let left = preceding_token.map_or(0, |token| token.loc.end().max(0) as usize);
    let scanner = scanner::get_scanner_for_source_file(file, left);
    let kind = scanner.token();
    if ast::is_token_kind(kind) {
        let loc = core::new_text_range(scanner.token_full_start(), scanner.token_end());
        if loc.pos() <= position && position < loc.end() {
            return Some(CommentTokenInfo {
                node: None,
                kind,
                loc,
            });
        }
    }
    astnav::get_token_at_position_info(file, position).map(CommentTokenInfo::from_token_info)
}

fn get_token_start_for_comment(file: &ast::SourceFile, token: CommentTokenInfo) -> i32 {
    token.node.map_or_else(
        || scanner::skip_trivia(file.text(), token.loc.pos().max(0) as usize) as i32,
        |node| astnav::get_start_of_node(node, file),
    )
}

fn get_leading_comment_ranges_of_token(
    file: &ast::SourceFile,
    token: CommentTokenInfo,
) -> Vec<ast::CommentRange> {
    if token.kind == ast::Kind::JsxText {
        return Vec::new();
    }
    scanner::get_leading_comment_ranges(file.text(), token.loc.pos())
}

fn get_range_of_enclosing_comment_worker(
    file: &ast::SourceFile,
    position: i32,
    preceding_token: Option<CommentTokenInfo>,
    token_at_position: Option<CommentTokenInfo>,
) -> Option<ast::CommentRange> {
    let token_at_position = token_at_position?;
    let token_start = get_token_start_for_comment(file, token_at_position);
    if token_start <= position && position < token_at_position.loc.end() {
        return None;
    }

    // Between two consecutive tokens, all comments are either trailing on the former
    // or leading on the latter (and none are in both lists).
    let mut comment_ranges = Vec::new();
    if let Some(preceding_token) = preceding_token {
        comment_ranges.extend(scanner::get_trailing_comment_ranges(
            file.text(),
            preceding_token.loc.end(),
        ));
    }
    comment_ranges.extend(get_leading_comment_ranges_of_token(file, token_at_position));
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
