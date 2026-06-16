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
    let preceding_token = astnav::find_preceding_token(file, position);
    get_range_of_enclosing_comment(file, position, preceding_token.as_ref(), token_at_position)
}

// Unlike the TS implementation, this function *will not* compute default values for
// `precedingToken` and `tokenAtPosition`.
// It is the caller's responsibility to call `astnav.GetTokenAtPosition` to compute a default `tokenAtPosition`,
// or `astnav.FindPrecedingToken` to compute a default `precedingToken`.
pub fn get_range_of_enclosing_comment(
    file: &ast::SourceFile,
    position: i32,
    preceding_token: Option<&ast::Node>,
    token_at_position: Option<&ast::Node>,
) -> Option<ast::CommentRange> {
    let token_at_position = token_at_position?;
    let store = file.store();
    let token_start = astnav::get_start_of_node(*token_at_position, file);
    if token_start <= position && position < store.loc(*token_at_position).end() {
        return None;
    }

    // Between two consecutive tokens, all comments are either trailing on the former
    // or leading on the latter (and none are in both lists).
    let mut comment_ranges = Vec::new();
    if let Some(preceding_token) = preceding_token {
        comment_ranges.extend(scanner::get_trailing_comment_ranges(
            file.text(),
            store.loc(*preceding_token).end(),
        ));
    }
    comment_ranges.extend(format::get_leading_comment_ranges_of_node(
        token_at_position,
        file,
    ));
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
