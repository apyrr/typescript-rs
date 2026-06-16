use std::{fmt::Write as _, sync::Arc};

use ts_ast as ast;
use ts_bundled as bundled;
use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_diagnosticwriter as diagnosticwriter;
use ts_locale as locale;
use ts_lsproto as lsproto;
use ts_tspath as tspath;

use crate::lsconv::LspLineMap;

#[derive(Clone)]
pub struct Converters {
    get_line_map: Arc<dyn Fn(&str) -> LspLineMap + Send + Sync>,
    position_encoding: lsproto::PositionEncodingKind,
}

pub trait Script {
    fn file_name(&self) -> &str;
    fn text(&self) -> &str;
}

impl Script for ast::SourceFile {
    fn file_name(&self) -> &str {
        self.file_name_ref()
    }

    fn text(&self) -> &str {
        ast::SourceFile::text(self)
    }
}

impl Script for ast::DiagnosticFileView {
    fn file_name(&self) -> &str {
        self.file_name()
    }

    fn text(&self) -> &str {
        self.text()
    }
}

impl Script for ast::DiagnosticFile {
    fn file_name(&self) -> &str {
        self.file_name()
    }

    fn text(&self) -> &str {
        self.text()
    }
}

pub fn new_converters(
    position_encoding: lsproto::PositionEncodingKind,
    get_line_map: impl Fn(&str) -> LspLineMap + Send + Sync + 'static,
) -> Converters {
    Converters {
        get_line_map: Arc::new(get_line_map),
        position_encoding,
    }
}

impl Converters {
    pub fn to_lsp_range(&self, script: &dyn Script, text_range: core::TextRange) -> lsproto::Range {
        lsproto::Range {
            start: self.position_to_line_and_character(script, text_range.pos()),
            end: self.position_to_line_and_character(script, text_range.end()),
        }
    }

    pub fn from_lsp_range(
        &self,
        script: &dyn Script,
        text_range: lsproto::Range,
    ) -> core::TextRange {
        core::new_text_range(
            self.line_and_character_to_position(script, text_range.start),
            self.line_and_character_to_position(script, text_range.end),
        )
    }

    pub fn from_lsp_text_change(
        &self,
        script: &dyn Script,
        change: &lsproto::TextDocumentContentChangePartial,
    ) -> core::TextChange {
        core::TextChange {
            text_range: self.from_lsp_range(script, change.range),
            new_text: change.text.clone(),
        }
    }

    pub fn to_lsp_location(
        &self,
        script: &dyn Script,
        range: core::TextRange,
    ) -> lsproto::Location {
        lsproto::Location {
            uri: file_name_to_document_uri(script.file_name()),
            range: self.to_lsp_range(script, range),
        }
    }

    pub fn line_and_character_to_position(
        &self,
        script: &dyn Script,
        line_and_character: lsproto::Position,
    ) -> core::TextPos {
        // UTF-8/16 0-indexed line and character to UTF-8 offset

        let line_map = (self.get_line_map)(script.file_name());

        let line = line_and_character.line as core::TextPos;
        let ch = line_and_character.character as core::TextPos;

        let text_len = script.text().len() as core::TextPos;

        // Clamp line to valid range.
        if line as usize >= line_map.line_starts.len() {
            return text_len;
        }

        let start = line_map.line_starts[line as usize];

        // Determine the end of this line (start of next line, or end of text).
        let line_end = if line as usize + 1 < line_map.line_starts.len() {
            line_map.line_starts[line as usize + 1]
        } else {
            text_len
        };

        if line_map.ascii_only || self.position_encoding == lsproto::PositionEncodingKind::Utf8 {
            return std::cmp::max(start, std::cmp::min(start + ch, line_end));
        }

        // Scan from line start counting UTF-16 code units to find the byte position.
        // Uses chars (not range + RuneLen) so that invalid UTF-8 bytes
        // advance by their actual size (1) rather than RuneLen(RuneError) == 3.
        // This matches the approach in scanner.ComputePositionOfLineAndUTF16Character.
        let mut utf16_char = 0 as core::TextPos;
        let mut pos = start as usize;
        let end = line_end as usize;
        let text = script.text();
        while pos < end {
            let r = text[pos..].chars().next().unwrap_or('\0');
            let size = r.len_utf8();
            let u16_len = r.len_utf16() as core::TextPos;
            if utf16_char + u16_len > ch {
                break;
            }
            utf16_char += u16_len;
            pos += size;
        }

        pos as core::TextPos
    }

    pub fn position_to_line_and_character(
        &self,
        script: &dyn Script,
        mut position: core::TextPos,
    ) -> lsproto::Position {
        // UTF-8 offset to UTF-8/16 0-indexed line and character

        position = std::cmp::max(
            0,
            std::cmp::min(position, script.text().len() as core::TextPos),
        );

        let line_map = (self.get_line_map)(script.file_name());

        let line = line_map.compute_index_of_line_start(position);

        // The current line ranges from lineMap.LineStarts[line] (or 0) to lineMap.LineStarts[line+1] (or len(text)).
        let start = line_map.line_starts[line];

        let character = if line_map.ascii_only
            || self.position_encoding == lsproto::PositionEncodingKind::Utf8
        {
            position - start
        } else {
            // We need to rescan the text as UTF-16 to find the character offset.
            script.text()[start as usize..position as usize]
                .chars()
                .map(|r| r.len_utf16() as core::TextPos)
                .sum()
        };

        lsproto::Position {
            line: line as u32,
            character: character as u32,
        }
    }
}

pub fn language_kind_to_script_kind(language_id: lsproto::LanguageKind) -> core::ScriptKind {
    match language_id.as_str() {
        "typescript" => core::ScriptKind::TS,
        "typescriptreact" => core::ScriptKind::TSX,
        "javascript" => core::ScriptKind::JS,
        "javascriptreact" => core::ScriptKind::JSX,
        "json" => core::ScriptKind::JSON,
        _ => core::ScriptKind::Unknown,
    }
}

// https://github.com/microsoft/vscode-uri/blob/edfdccd976efaf4bb8fdeca87e97c47257721729/src/uri.ts#L455
pub fn file_name_to_document_uri(file_name: &str) -> lsproto::DocumentUri {
    if bundled::is_bundled(file_name) {
        return file_name.to_string();
    }
    if tspath::is_dynamic_file_name(file_name) {
        let rest = &file_name[2..];
        let Some((scheme, rest)) = rest.split_once('/') else {
            panic!("invalid file name: {file_name}");
        };
        let Some((authority, path)) = rest.split_once('/') else {
            panic!("invalid file name: {file_name}");
        };
        if authority == "ts-nul-authority" {
            return format!("{scheme}:{path}");
        }
        return format!("{scheme}://{authority}/{path}");
    }

    let (mut volume, mut file_name) = tspath::split_volume_path(file_name)
        .unwrap_or_else(|| (String::new(), file_name.to_string()));
    if !volume.is_empty() {
        volume = format!("/{}", extra_escape_replace(&volume));
    }

    file_name = file_name.trim_start_matches("//").to_string();

    let parts = file_name
        .split('/')
        .map(path_escape)
        .map(|part| extra_escape_replace(&part))
        .collect::<Vec<_>>();

    format!("file://{}{}", volume, parts.join("/"))
}

pub fn diagnostic_to_lsp_pull(
    ctx: &core::Context,
    converters: &Converters,
    diagnostic: &ast::Diagnostic,
    report_style_checks_as_warnings: bool,
) -> lsproto::Diagnostic {
    let client_caps = lsproto::get_client_capabilities(ctx);
    let client_diagnostic_caps = client_caps.text_document.diagnostic;
    diagnostic_to_lsp(
        ctx,
        converters,
        diagnostic,
        DiagnosticOptions {
            report_style_checks_as_warnings, // !!! get through context UserPreferences
            related_information: client_diagnostic_caps.related_information,
            tag_value_set: client_diagnostic_caps.tag_support.value_set,
            visual_studio: client_caps.vs_supports_visual_studio_extensions,
        },
    )
}

pub fn diagnostic_to_lsp_push(
    ctx: &core::Context,
    converters: &Converters,
    diagnostic: &ast::Diagnostic,
) -> lsproto::Diagnostic {
    let client_caps = lsproto::get_client_capabilities(ctx);
    let client_diagnostic_caps = client_caps.text_document.publish_diagnostics;
    diagnostic_to_lsp(
        ctx,
        converters,
        diagnostic,
        DiagnosticOptions {
            report_style_checks_as_warnings: false,
            related_information: client_diagnostic_caps.related_information,
            tag_value_set: client_diagnostic_caps.tag_support.value_set,
            visual_studio: client_caps.vs_supports_visual_studio_extensions,
        },
    )
}

#[derive(Default)]
struct DiagnosticOptions {
    report_style_checks_as_warnings: bool,
    related_information: bool,
    tag_value_set: Vec<lsproto::DiagnosticTag>,
    visual_studio: bool,
}

// https://github.com/microsoft/vscode/blob/93e08afe0469712706ca4e268f778cfadf1a43ef/extensions/typescript-language-features/src/typeScriptServiceClientHost.ts#L40C7-L40C29
fn style_check_diagnostics() -> collections::Set<i32> {
    collections::new_set_from_items([
        diagnostics::X_0_IS_DECLARED_BUT_NEVER_USED.code(),
        diagnostics::X_0_IS_DECLARED_BUT_ITS_VALUE_IS_NEVER_READ.code(),
        diagnostics::PROPERTY_0_IS_DECLARED_BUT_ITS_VALUE_IS_NEVER_READ.code(),
        diagnostics::ALL_IMPORTS_IN_IMPORT_DECLARATION_ARE_UNUSED.code(),
        diagnostics::UNREACHABLE_CODE_DETECTED.code(),
        diagnostics::UNUSED_LABEL.code(),
        diagnostics::FALLTHROUGH_CASE_IN_SWITCH.code(),
        diagnostics::NOT_ALL_CODE_PATHS_RETURN_A_VALUE.code(),
    ])
}

fn diagnostic_to_lsp(
    _ctx: &core::Context,
    converters: &Converters,
    diagnostic: &ast::Diagnostic,
    opts: DiagnosticOptions,
) -> lsproto::Diagnostic {
    let locale = locale::und();
    let mut severity = match diagnostic.category() {
        diagnostics::Category::Suggestion => lsproto::DiagnosticSeverity::Hint,
        diagnostics::Category::Message => lsproto::DiagnosticSeverity::Information,
        diagnostics::Category::Warning => lsproto::DiagnosticSeverity::Warning,
        _ => lsproto::DiagnosticSeverity::Error,
    };

    if opts.report_style_checks_as_warnings
        && severity == lsproto::DiagnosticSeverity::Error
        && style_check_diagnostics().has(&diagnostic.code())
    {
        severity = lsproto::DiagnosticSeverity::Warning;
    }

    let mut related_information = Vec::new();
    if opts.related_information {
        related_information.reserve(diagnostic.related_information().len());
        for related in diagnostic.related_information() {
            if let Some(file_ref) = related.file() {
                related_information.push(lsproto::DiagnosticRelatedInformation {
                    location: diagnostic_location_to_lsp(converters, file_ref, related.loc()),
                    message: related.localize(locale.clone()),
                });
            }
        }
    }

    let mut tags = Vec::new();
    if !opts.tag_value_set.is_empty()
        && (diagnostic.reports_unnecessary() || diagnostic.reports_deprecated())
    {
        if diagnostic.reports_unnecessary()
            && opts
                .tag_value_set
                .contains(&lsproto::DiagnosticTag::Unnecessary)
        {
            tags.push(lsproto::DiagnosticTag::Unnecessary);
        }
        if diagnostic.reports_deprecated()
            && opts
                .tag_value_set
                .contains(&lsproto::DiagnosticTag::Deprecated)
        {
            tags.push(lsproto::DiagnosticTag::Deprecated);
        }
    }

    // For diagnostics without a file (e.g., program diagnostics), use a zero range
    let mut lsp_range = lsproto::Range::default();
    if let Some(file_ref) = diagnostic.file() {
        lsp_range = diagnostic_range_to_lsp(converters, file_ref, diagnostic.loc());
    }

    let (code, source) = if opts.visual_studio {
        (
            Some(lsproto::IntegerOrString {
                string: Some(format!("TS{}", diagnostic.code())),
                ..Default::default()
            }),
            None,
        )
    } else {
        (
            Some(lsproto::IntegerOrString {
                integer: Some(diagnostic.code()),
                ..Default::default()
            }),
            Some("ts".to_string()),
        )
    };

    lsproto::Diagnostic {
        range: lsp_range,
        code,
        severity: Some(severity),
        message: message_chain_to_string(diagnostic, locale),
        source,
        related_information: ptr_to_vec_if_non_empty(related_information),
        tags: ptr_to_vec_if_non_empty(tags),
        ..Default::default()
    }
}

fn diagnostic_range_to_lsp(
    converters: &Converters,
    file: &ast::DiagnosticFile,
    range: core::TextRange,
) -> lsproto::Range {
    converters.to_lsp_range(file, range)
}

fn diagnostic_location_to_lsp(
    converters: &Converters,
    file: &ast::DiagnosticFile,
    range: core::TextRange,
) -> lsproto::Location {
    lsproto::Location {
        uri: file_name_to_document_uri(file.file_name()),
        range: converters.to_lsp_range(file, range),
    }
}

fn message_chain_to_string(diagnostic: &ast::Diagnostic, locale: locale::Locale) -> String {
    if diagnostic.message_chain().is_empty() {
        return diagnostic.localize(locale);
    }
    let mut output = Vec::new();
    diagnosticwriter::write_flattened_ast_diagnostic_message(
        &mut output,
        diagnostic.clone(),
        "\n",
        locale,
    );
    String::from_utf8(output).expect("diagnostic message should be valid UTF-8")
}

fn ptr_to_vec_if_non_empty<T>(value: Vec<T>) -> Option<Vec<T>> {
    if value.is_empty() {
        return None;
    }
    Some(value)
}

fn extra_escape_replace(value: &str) -> String {
    value
        .replace(':', "%3A")
        .replace('/', "%2F")
        .replace('?', "%3F")
        .replace('#', "%23")
        .replace('[', "%5B")
        .replace(']', "%5D")
        .replace('@', "%40")
        .replace('!', "%21")
        .replace('$', "%24")
        .replace('&', "%26")
        .replace('\'', "%27")
        .replace('(', "%28")
        .replace(')', "%29")
        .replace('*', "%2A")
        .replace('+', "%2B")
        .replace(',', "%2C")
        .replace(';', "%3B")
        .replace('=', "%3D")
        .replace(' ', "%20")
}

fn path_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            escaped.push(ch);
        } else {
            let mut buf = [0; 4];
            for byte in ch.encode_utf8(&mut buf).as_bytes() {
                write!(&mut escaped, "%{:02X}", byte).ok();
            }
        }
    }
    escaped
}
