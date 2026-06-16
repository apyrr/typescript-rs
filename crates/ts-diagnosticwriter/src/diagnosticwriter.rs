use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_scanner as scanner;
use ts_tspath as tspath;

pub trait FileLike {
    fn file_name(&self) -> String;
    fn text(&self) -> String;
    fn ecma_line_map(&self) -> Arc<[core::TextPos]>;
}

impl FileLike for ast::SourceFile {
    fn file_name(&self) -> String {
        self.file_name()
    }

    fn text(&self) -> String {
        self.text().to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        ast::SourceFileLike::ecma_line_map(self)
    }
}

impl FileLike for ast::DiagnosticFile {
    fn file_name(&self) -> String {
        self.file_name().to_string()
    }

    fn text(&self) -> String {
        ast::DiagnosticFile::text(self).to_string()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        ast::DiagnosticFile::ecma_line_map(self)
    }
}

impl ast::SourceFileLike for &dyn FileLike {
    fn text(&self) -> String {
        FileLike::text(*self)
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        FileLike::ecma_line_map(*self)
    }
}

// Diagnostic interface abstracts over ast.Diagnostic and LSP diagnostics
pub trait Diagnostic {
    fn file(&self) -> Option<Box<dyn FileLike>>;
    fn pos(&self) -> i32;
    fn end(&self) -> i32;
    fn len(&self) -> i32;
    fn is_empty(&self) -> bool;
    fn code(&self) -> i32;
    fn category(&self) -> diagnostics::Category;
    fn localize(&self, locale: locale::Locale) -> String;
    fn message_chain(&self) -> Vec<Box<dyn Diagnostic>>;
    fn related_information(&self) -> Vec<Box<dyn Diagnostic>>;
}

// ASTDiagnostic wraps ast.Diagnostic to implement the Diagnostic interface
pub struct ASTDiagnostic {
    diagnostic: ast::Diagnostic,
}

impl Diagnostic for ASTDiagnostic {
    fn related_information(&self) -> Vec<Box<dyn Diagnostic>> {
        let related = self.diagnostic.related_information();
        related
            .iter()
            .cloned()
            .map(|r| Box::new(ASTDiagnostic { diagnostic: r }) as Box<dyn Diagnostic>)
            .collect()
    }

    fn file(&self) -> Option<Box<dyn FileLike>> {
        self.diagnostic
            .file()
            .cloned()
            .map(|file| Box::new(file) as Box<dyn FileLike>)
    }

    fn message_chain(&self) -> Vec<Box<dyn Diagnostic>> {
        let chain = self.diagnostic.message_chain();
        chain
            .iter()
            .cloned()
            .map(|c| Box::new(ASTDiagnostic { diagnostic: c }) as Box<dyn Diagnostic>)
            .collect()
    }

    fn pos(&self) -> i32 {
        self.diagnostic.pos()
    }

    fn end(&self) -> i32 {
        self.diagnostic.end()
    }

    fn len(&self) -> i32 {
        self.diagnostic.len()
    }

    fn is_empty(&self) -> bool {
        self.diagnostic.is_empty()
    }

    fn code(&self) -> i32 {
        self.diagnostic.code()
    }

    fn category(&self) -> diagnostics::Category {
        self.diagnostic.category()
    }

    fn localize(&self, locale: locale::Locale) -> String {
        self.diagnostic.localize(locale)
    }
}

pub fn wrap_astdiagnostic(d: ast::Diagnostic) -> ASTDiagnostic {
    ASTDiagnostic { diagnostic: d }
}

pub fn wrap_ast_diagnostic(d: ast::Diagnostic) -> ASTDiagnostic {
    wrap_astdiagnostic(d)
}

pub fn wrap_astdiagnostics(diags: Vec<ast::Diagnostic>) -> Vec<ASTDiagnostic> {
    diags.into_iter().map(wrap_astdiagnostic).collect()
}

pub fn wrap_ast_diagnostics(diags: Vec<ast::Diagnostic>) -> Vec<ASTDiagnostic> {
    wrap_astdiagnostics(diags)
}

pub fn from_astdiagnostics(diags: Vec<ast::Diagnostic>) -> Vec<Box<dyn Diagnostic>> {
    diags
        .into_iter()
        .map(|d| Box::new(wrap_astdiagnostic(d)) as Box<dyn Diagnostic>)
        .collect()
}

pub fn from_ast_diagnostics(diags: Vec<ast::Diagnostic>) -> Vec<Box<dyn Diagnostic>> {
    from_astdiagnostics(diags)
}

pub fn to_diagnostics<T: Diagnostic + 'static>(diags: Vec<T>) -> Vec<Box<dyn Diagnostic>> {
    diags
        .into_iter()
        .map(|d| Box::new(d) as Box<dyn Diagnostic>)
        .collect()
}

pub fn compare_astdiagnostics(a: &ASTDiagnostic, b: &ASTDiagnostic) -> i32 {
    ast::compare_diagnostics(&a.diagnostic, &b.diagnostic)
}

pub fn compare_ast_diagnostics(a: &ASTDiagnostic, b: &ASTDiagnostic) -> i32 {
    compare_astdiagnostics(a, b)
}

pub struct FormattingOptions {
    pub locale: locale::Locale,
    pub compare_paths_options: tspath::ComparePathsOptions,
    pub new_line: String,
}

const FOREGROUND_COLOR_ESCAPE_GREY: &str = "\u{001b}[90m";
const FOREGROUND_COLOR_ESCAPE_RED: &str = "\u{001b}[91m";
const FOREGROUND_COLOR_ESCAPE_YELLOW: &str = "\u{001b}[93m";
const FOREGROUND_COLOR_ESCAPE_BLUE: &str = "\u{001b}[94m";
const FOREGROUND_COLOR_ESCAPE_CYAN: &str = "\u{001b}[96m";

const GUTTER_STYLE_SEQUENCE: &str = "\u{001b}[7m";
const GUTTER_SEPARATOR: &str = " ";
const RESET_ESCAPE_SEQUENCE: &str = "\u{001b}[0m";
const ELLIPSIS: &str = "...";
const HALF_INDENT: &str = "  ";
const INDENT: &str = "    ";

pub fn format_diagnostics_with_color_and_context(
    output: &mut dyn Write,
    diags: &[Box<dyn Diagnostic>],
    format_opts: &FormattingOptions,
) {
    if diags.is_empty() {
        return;
    }
    let mut text = Vec::new();
    for (i, diagnostic) in diags.iter().enumerate() {
        if i > 0 {
            write!(text, "{}", format_opts.new_line).ok();
        }
        format_diagnostic_with_color_and_context(&mut text, diagnostic.as_ref(), format_opts);
    }
    if text.ends_with(format_opts.new_line.as_bytes()) {
        text.truncate(text.len() - format_opts.new_line.len());
    }
    output.write_all(&text).ok();
}

pub fn format_diagnostic_with_color_and_context(
    output: &mut dyn Write,
    diagnostic: &dyn Diagnostic,
    format_opts: &FormattingOptions,
) {
    if let Some(file) = diagnostic.file() {
        let pos = diagnostic.pos();
        write_location(
            output,
            file.as_ref(),
            pos,
            Some(format_opts),
            write_with_style_and_reset,
        );
        write!(output, " - ").ok();
    }

    write_with_style_and_reset(
        output,
        diagnostic.category().name(),
        get_category_format(diagnostic.category()),
    );
    write!(
        output,
        "{} TS{}: {}",
        FOREGROUND_COLOR_ESCAPE_GREY,
        diagnostic.code(),
        RESET_ESCAPE_SEQUENCE
    )
    .ok();
    write_flattened_diagnostic_message(
        output,
        diagnostic,
        &format_opts.new_line,
        format_opts.locale.clone(),
    );

    if let Some(file) = diagnostic.file()
        && diagnostic.code() != diagnostics::File_appears_to_be_binary.code()
    {
        write!(output, "{}", format_opts.new_line).ok();
        write_code_snippet(
            output,
            file.as_ref(),
            diagnostic.pos(),
            diagnostic.len(),
            get_category_format(diagnostic.category()),
            "",
            format_opts,
        );
        write!(output, "{}", format_opts.new_line).ok();
    }

    let related = diagnostic.related_information();
    if !related.is_empty() {
        for related_information in related {
            if let Some(file) = related_information.file() {
                write!(output, "{}", format_opts.new_line).ok();
                write!(output, "{HALF_INDENT}").ok();
                let pos = related_information.pos();
                write_location(
                    output,
                    file.as_ref(),
                    pos,
                    Some(format_opts),
                    write_with_style_and_reset,
                );
                write!(output, " - ").ok();
                write_flattened_diagnostic_message(
                    output,
                    related_information.as_ref(),
                    &format_opts.new_line,
                    format_opts.locale.clone(),
                );
                write_code_snippet(
                    output,
                    file.as_ref(),
                    pos,
                    related_information.len(),
                    FOREGROUND_COLOR_ESCAPE_CYAN,
                    INDENT,
                    format_opts,
                );
            }
            write!(output, "{}", format_opts.new_line).ok();
        }
    }
}

fn write_code_snippet(
    writer: &mut dyn Write,
    source_file: &dyn FileLike,
    start: i32,
    length: i32,
    squiggle_color: &str,
    indent: &str,
    format_opts: &FormattingOptions,
) {
    let (first_line, first_line_char) =
        get_ecma_line_and_utf16_character_of_position(source_file, start);
    let (last_line, mut last_line_char) =
        get_ecma_line_and_utf16_character_of_position(source_file, start + length);
    if length == 0 {
        last_line_char += 1; // When length is zero, squiggle the character right after the start position.
    }

    let last_line_of_file = get_ecma_line_of_position(source_file, source_file.text().len() as i32);

    let has_more_than_five_lines = last_line - first_line >= 4;
    let mut gutter_width = (last_line + 1).to_string().len();
    if has_more_than_five_lines {
        gutter_width = gutter_width.max(ELLIPSIS.len());
    }

    let mut i = first_line;
    while i <= last_line {
        write!(writer, "{}", format_opts.new_line).ok();

        // If the error spans over 5 lines, we'll only show the first 2 and last 2 lines,
        // so we'll skip ahead to the second-to-last line.
        if has_more_than_five_lines && first_line + 1 < i && i < last_line - 1 {
            write!(
                writer,
                "{indent}{GUTTER_STYLE_SEQUENCE}{:>width$}{RESET_ESCAPE_SEQUENCE}{GUTTER_SEPARATOR}{}",
                ELLIPSIS,
                format_opts.new_line,
                width = gutter_width
            )
            .ok();
            i = last_line - 1;
        }

        let line_start = get_ecma_position_of_line_and_byte_offset(source_file, i, 0);
        let line_end = if i < last_line_of_file {
            get_ecma_position_of_line_and_byte_offset(source_file, i + 1, 0)
        } else {
            source_file.text().len() as i32
        };

        let mut line_content = source_file.text()[line_start as usize..line_end as usize]
            .trim_end_matches(char::is_whitespace)
            .to_string(); // trim from end
        line_content = line_content.replace('\t', " "); // convert tabs to single spaces

        // Output the gutter and the actual contents of the line.
        write!(
            writer,
            "{indent}{GUTTER_STYLE_SEQUENCE}{:>width$}{RESET_ESCAPE_SEQUENCE}{GUTTER_SEPARATOR}{line_content}{}",
            i + 1,
            format_opts.new_line,
            width = gutter_width
        )
        .ok();

        // Output the gutter and the error span for the line using tildes.
        write!(
            writer,
            "{indent}{GUTTER_STYLE_SEQUENCE}{:>width$}{RESET_ESCAPE_SEQUENCE}{GUTTER_SEPARATOR}{squiggle_color}",
            "",
            width = gutter_width
        )
        .ok();
        match i {
            line if line == first_line => {
                // If we're on the last line, then limit it to the last character of the last line.
                // Otherwise, we'll just squiggle the rest of the line, giving 'slice' no end position.
                let last_char_for_line = if i == last_line {
                    last_line_char as usize
                } else {
                    core::utf16_len(&line_content) as usize
                };

                // Fill with spaces until the first character,
                // then squiggle the remainder of the line.
                write!(writer, "{}", " ".repeat(first_line_char as usize)).ok();
                write!(
                    writer,
                    "{}",
                    "~".repeat(last_char_for_line - first_line_char as usize)
                )
                .ok();
            }
            line if line == last_line => {
                // Squiggle until the final character.
                write!(writer, "{}", "~".repeat(last_line_char as usize)).ok();
            }
            _ => {
                // Squiggle the entire line.
                write!(
                    writer,
                    "{}",
                    "~".repeat(core::utf16_len(&line_content) as usize)
                )
                .ok();
            }
        }

        write!(writer, "{RESET_ESCAPE_SEQUENCE}").ok();
        i += 1;
    }
}

fn get_ecma_line_of_position(source_file: &dyn FileLike, pos: i32) -> i32 {
    scanner::compute_line_of_position(&source_file.ecma_line_map(), pos.max(0) as usize) as i32
}

fn get_ecma_line_and_utf16_character_of_position(
    source_file: &dyn FileLike,
    pos: i32,
) -> (i32, core::UTF16Offset) {
    let text = source_file.text();
    let line_map = source_file.ecma_line_map();
    let pos = pos.max(0) as usize;
    let (line, byte_offset) = core::position_to_line_and_byte_offset(pos, &line_map);
    let line_start = line_map.get(line).copied().unwrap_or_default().max(0) as usize;
    let line_text = &text[line_start..line_start + byte_offset];
    (line as i32, core::utf16_len(line_text))
}

fn get_ecma_position_of_line_and_byte_offset(
    source_file: &dyn FileLike,
    line: i32,
    byte_offset: i32,
) -> i32 {
    scanner::compute_position_of_line_and_byte_offset(
        &source_file.ecma_line_map(),
        line.max(0) as usize,
        byte_offset.max(0) as usize,
    ) as i32
}

pub fn flatten_diagnostic_message(
    d: &dyn Diagnostic,
    new_line: &str,
    locale: locale::Locale,
) -> String {
    let mut output = Vec::new();
    write_flattened_diagnostic_message(&mut output, d, new_line, locale);
    String::from_utf8(output).unwrap_or_default()
}

pub fn write_flattened_astdiagnostic_message(
    writer: &mut dyn Write,
    diagnostic: ast::Diagnostic,
    newline: &str,
    locale: locale::Locale,
) {
    write_flattened_diagnostic_message(writer, &wrap_astdiagnostic(diagnostic), newline, locale)
}

pub fn write_flattened_ast_diagnostic_message(
    writer: &mut dyn Write,
    diagnostic: ast::Diagnostic,
    newline: &str,
    locale: locale::Locale,
) {
    write_flattened_astdiagnostic_message(writer, diagnostic, newline, locale)
}

pub fn write_flattened_diagnostic_message(
    writer: &mut dyn Write,
    diagnostic: &dyn Diagnostic,
    newline: &str,
    locale: locale::Locale,
) {
    write!(writer, "{}", diagnostic.localize(locale.clone())).ok();

    for chain in diagnostic.message_chain() {
        flatten_diagnostic_message_chain(
            writer,
            chain.as_ref(),
            newline,
            locale.clone(),
            1, /*level*/
        );
    }
}

fn flatten_diagnostic_message_chain(
    writer: &mut dyn Write,
    chain: &dyn Diagnostic,
    new_line: &str,
    locale: locale::Locale,
    level: usize,
) {
    write!(writer, "{new_line}").ok();
    for _ in 0..level {
        write!(writer, "  ").ok();
    }

    write!(writer, "{}", chain.localize(locale.clone())).ok();
    for child in chain.message_chain() {
        flatten_diagnostic_message_chain(
            writer,
            child.as_ref(),
            new_line,
            locale.clone(),
            level + 1,
        );
    }
}

fn get_category_format(category: diagnostics::Category) -> &'static str {
    match category {
        diagnostics::Category::Error => FOREGROUND_COLOR_ESCAPE_RED,
        diagnostics::Category::Warning => FOREGROUND_COLOR_ESCAPE_YELLOW,
        diagnostics::Category::Suggestion => FOREGROUND_COLOR_ESCAPE_GREY,
        diagnostics::Category::Message => FOREGROUND_COLOR_ESCAPE_BLUE,
        _ => "",
    }
}

type FormattedWriter = fn(&mut dyn Write, &str, &str);

fn write_with_style_and_reset(output: &mut dyn Write, text: &str, format_style: &str) {
    write!(output, "{format_style}{text}{RESET_ESCAPE_SEQUENCE}").ok();
}

pub fn write_location(
    output: &mut dyn Write,
    file: &dyn FileLike,
    pos: i32,
    format_opts: Option<&FormattingOptions>,
    write_with_style_and_reset: FormattedWriter,
) {
    let (first_line, first_char) = get_ecma_line_and_utf16_character_of_position(file, pos);
    let relative_file_name = if let Some(format_opts) = format_opts {
        tspath::convert_to_relative_path(&file.file_name(), &format_opts.compare_paths_options)
    } else {
        file.file_name()
    };

    write_with_style_and_reset(output, &relative_file_name, FOREGROUND_COLOR_ESCAPE_CYAN);
    write!(output, ":").ok();
    write_with_style_and_reset(
        output,
        &(first_line + 1).to_string(),
        FOREGROUND_COLOR_ESCAPE_YELLOW,
    );
    write!(output, ":").ok();
    write_with_style_and_reset(
        output,
        &(first_char + 1).to_string(),
        FOREGROUND_COLOR_ESCAPE_YELLOW,
    );
}

// Some of these lived in watch.ts, but they're not specific to the watch API.

pub struct ErrorSummary {
    pub total_error_count: usize,
    pub global_errors: Vec<Box<dyn Diagnostic>>,
    pub files_by_name: HashMap<String, Box<dyn FileLike>>,
    pub errors_by_file: HashMap<String, Vec<Box<dyn Diagnostic>>>,
    pub sorted_files: Vec<String>,
}

pub fn write_error_summary_text(
    output: &mut dyn Write,
    all_diagnostics: Vec<Box<dyn Diagnostic>>,
    format_opts: &FormattingOptions,
) {
    // Roughly corresponds to 'getErrorSummaryText' from watch.ts

    let error_summary = get_error_summary(all_diagnostics);
    let total_error_count = error_summary.total_error_count;
    if total_error_count == 0 {
        return;
    }

    let first_file_name = error_summary
        .sorted_files
        .first()
        .and_then(|file_name| {
            let file = error_summary.files_by_name.get(file_name)?;
            let file_errors = error_summary.errors_by_file.get(file_name)?;
            Some(pretty_path_for_file_error(
                Some(file.as_ref()),
                file_errors,
                format_opts,
            ))
        })
        .unwrap_or_default();
    let num_erroring_files = error_summary.errors_by_file.len();

    let message = if total_error_count == 1 {
        // Special-case a single error.
        if !error_summary.global_errors.is_empty() || first_file_name.is_empty() {
            diagnostics::Found_1_error.localize(format_opts.locale.clone(), Vec::new())
        } else {
            diagnostics::Found_1_error_in_0
                .localize(format_opts.locale.clone(), vec![Box::new(first_file_name)])
        }
    } else {
        match num_erroring_files {
            0 => {
                // No file-specific errors.
                diagnostics::Found_0_errors.localize(
                    format_opts.locale.clone(),
                    vec![Box::new(total_error_count)],
                )
            }
            1 => {
                // One file with errors.
                diagnostics::Found_0_errors_in_the_same_file_starting_at_Colon_1.localize(
                    format_opts.locale.clone(),
                    vec![Box::new(total_error_count), Box::new(first_file_name)],
                )
            }
            _ => {
                // Multiple files with errors.
                diagnostics::Found_0_errors_in_1_files.localize(
                    format_opts.locale.clone(),
                    vec![Box::new(total_error_count), Box::new(num_erroring_files)],
                )
            }
        }
    };
    write!(
        output,
        "{}{}{}{}",
        format_opts.new_line, message, format_opts.new_line, format_opts.new_line
    )
    .ok();
    if num_erroring_files > 1 {
        write_tabular_errors_display(output, &error_summary, format_opts);
        write!(output, "{}", format_opts.new_line).ok();
    }
}

fn get_error_summary(diags: Vec<Box<dyn Diagnostic>>) -> ErrorSummary {
    let mut total_error_count = 0;
    let mut global_errors = Vec::new();
    let mut errors_by_file: HashMap<String, Vec<Box<dyn Diagnostic>>> = HashMap::new();
    let mut files_by_name: HashMap<String, Box<dyn FileLike>> = HashMap::new();

    for diagnostic in diags {
        if diagnostic.category() != diagnostics::Category::Error {
            continue;
        }

        total_error_count += 1;
        if let Some(file) = diagnostic.file() {
            let file_name = file.file_name();
            files_by_name.entry(file_name.clone()).or_insert(file);
            errors_by_file
                .entry(file_name)
                .or_default()
                .push(diagnostic);
        } else {
            global_errors.push(diagnostic);
        }
    }

    // !!!
    // Need an ordered map here, but sorting for consistency.
    let mut sorted_files: Vec<_> = errors_by_file.keys().cloned().collect();
    sorted_files.sort();

    ErrorSummary {
        total_error_count,
        global_errors,
        files_by_name,
        errors_by_file,
        sorted_files,
    }
}

fn write_tabular_errors_display(
    output: &mut dyn Write,
    error_summary: &ErrorSummary,
    format_opts: &FormattingOptions,
) {
    let sorted_files = &error_summary.sorted_files;

    let mut max_errors = 0;
    for errors_for_file in error_summary.errors_by_file.values() {
        max_errors = max_errors.max(errors_for_file.len());
    }

    // !!!
    // TODO (drosen): This was never localized.
    // Should make this better.
    let header_row = diagnostics::Errors_Files.localize(format_opts.locale.clone(), Vec::new());
    let left_column_heading_length = header_row.split(' ').next().unwrap_or("").len();
    let length_of_biggest_error_count = max_errors.to_string().len();
    let left_padding_goal = left_column_heading_length.max(length_of_biggest_error_count);
    let header_padding = length_of_biggest_error_count.saturating_sub(left_column_heading_length);

    write!(
        output,
        "{}{}{}",
        " ".repeat(header_padding),
        header_row,
        format_opts.new_line
    )
    .ok();

    for file_name in sorted_files {
        let Some(file) = error_summary.files_by_name.get(file_name) else {
            continue;
        };
        let Some(file_errors) = error_summary.errors_by_file.get(file_name) else {
            continue;
        };
        let error_count = file_errors.len();

        write!(
            output,
            "{:>width$}  {}{}",
            error_count,
            pretty_path_for_file_error(Some(file.as_ref()), file_errors, format_opts),
            format_opts.new_line,
            width = left_padding_goal
        )
        .ok();
    }
}

fn pretty_path_for_file_error(
    file: Option<&dyn FileLike>,
    file_errors: &[Box<dyn Diagnostic>],
    format_opts: &FormattingOptions,
) -> String {
    let Some(file) = file else {
        return String::new();
    };
    if file_errors.is_empty() {
        return String::new();
    }
    let line = get_ecma_line_of_position(file, file_errors[0].pos());
    let mut file_name = file.file_name();
    if tspath::path_is_absolute(&file_name)
        && tspath::path_is_absolute(&format_opts.compare_paths_options.current_directory)
    {
        file_name =
            tspath::convert_to_relative_path(&file.file_name(), &format_opts.compare_paths_options);
    }
    format!(
        "{file_name}{FOREGROUND_COLOR_ESCAPE_GREY}:{}{RESET_ESCAPE_SEQUENCE}",
        line + 1
    )
}

pub fn write_format_diagnostics(
    output: &mut dyn Write,
    diagnostics: &[Box<dyn Diagnostic>],
    format_opts: &FormattingOptions,
) {
    for diagnostic in diagnostics {
        write_format_diagnostic(output, diagnostic.as_ref(), format_opts);
    }
}

pub fn write_format_diagnostic(
    output: &mut dyn Write,
    diagnostic: &dyn Diagnostic,
    format_opts: &FormattingOptions,
) {
    if let Some(file) = diagnostic.file() {
        let (line, character) =
            get_ecma_line_and_utf16_character_of_position(file.as_ref(), diagnostic.pos());
        let file_name = file.file_name();
        let relative_file_name =
            tspath::convert_to_relative_path(&file_name, &format_opts.compare_paths_options);
        write!(
            output,
            "{}({},{}): ",
            relative_file_name,
            line + 1,
            character + 1
        )
        .ok();
    }

    write!(
        output,
        "{} TS{}: ",
        diagnostic.category().name(),
        diagnostic.code()
    )
    .ok();
    write_flattened_diagnostic_message(
        output,
        diagnostic,
        &format_opts.new_line,
        format_opts.locale.clone(),
    );
    write!(output, "{}", format_opts.new_line).ok();
}

pub fn format_diagnostics_status_with_color_and_time(
    output: &mut dyn Write,
    time: &str,
    diag: &dyn Diagnostic,
    format_opts: &FormattingOptions,
) {
    write!(output, "[").ok();
    write_with_style_and_reset(output, time, FOREGROUND_COLOR_ESCAPE_GREY);
    write!(output, "] ").ok();
    write_flattened_diagnostic_message(
        output,
        diag,
        &format_opts.new_line,
        format_opts.locale.clone(),
    );
}

pub fn format_diagnostics_status_and_time(
    output: &mut dyn Write,
    time: &str,
    diag: &dyn Diagnostic,
    format_opts: &FormattingOptions,
) {
    write!(output, "{time} - ").ok();
    write_flattened_diagnostic_message(
        output,
        diag,
        &format_opts.new_line,
        format_opts.locale.clone(),
    );
}

pub fn screen_starting_codes() -> Vec<i32> {
    vec![
        diagnostics::Starting_compilation_in_watch_mode.code(),
        diagnostics::File_change_detected_Starting_incremental_compilation.code(),
    ]
}

pub fn try_clear_screen(
    output: &mut dyn Write,
    diag: &dyn Diagnostic,
    options: &core::CompilerOptions,
) -> bool {
    if !options.preserve_watch_output.is_true()
        && !options.extended_diagnostics.is_true()
        && !options.diagnostics.is_true()
        && screen_starting_codes().contains(&diag.code())
    {
        write!(output, "\x1B[2J\x1B[3J\x1B[H").ok(); // Clear screen and move cursor to home position
        return true;
    }
    false
}
