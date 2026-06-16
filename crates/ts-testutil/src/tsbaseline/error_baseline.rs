use std::cmp::Ordering;
use std::sync::{Arc, LazyLock};

use crate::baseline;
use crate::harnessutil::TestFile;
use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_diagnosticwriter as diagnosticwriter;
use ts_locale as locale;
use ts_scanner as scanner;
use ts_tspath as tspath;

pub const HARNESS_NEW_LINE: &str = "\r\n";

static DIAGNOSTICS_LOCATION_PREFIX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?im)^(lib.*\.d\.ts)\(\d+,\d+\)").unwrap());

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Diagnostic {
    pub file_name: Option<String>,
    pub sort_pos: i32,
    pub sort_end: i32,
    pub pos: usize,
    pub len: usize,
    pub line: usize,
    pub character: usize,
    pub code: i32,
    pub category: String,
    pub message: String,
    pub related: Vec<Diagnostic>,
}

struct DiagnosticSource<'a> {
    text: &'a str,
}

impl ast::SourceFileLike for DiagnosticSource<'_> {
    fn text(&self) -> String {
        self.text.to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        Arc::from(core::compute_ecma_line_starts(self.text))
    }
}

#[derive(Clone, Debug)]
struct BaselineFile {
    file_name: String,
    text: String,
}

impl diagnosticwriter::FileLike for BaselineFile {
    fn file_name(&self) -> String {
        self.file_name.clone()
    }

    fn text(&self) -> String {
        self.text.clone()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        Arc::from(core::compute_ecma_line_starts(&self.text))
    }
}

#[derive(Clone, Debug)]
struct BaselineDiagnostic {
    diagnostic: Diagnostic,
    input_files: Arc<Vec<BaselineFile>>,
}

impl diagnosticwriter::Diagnostic for BaselineDiagnostic {
    fn file(&self) -> Option<Box<dyn diagnosticwriter::FileLike>> {
        let file_name = self.diagnostic.file_name.as_ref()?;
        self.input_files
            .iter()
            .find(|input_file| same_test_file_path(file_name, &input_file.file_name))
            .cloned()
            .map(|file| Box::new(file) as Box<dyn diagnosticwriter::FileLike>)
    }

    fn pos(&self) -> i32 {
        self.diagnostic.pos as i32
    }

    fn end(&self) -> i32 {
        self.diagnostic.pos.saturating_add(self.diagnostic.len) as i32
    }

    fn len(&self) -> i32 {
        self.diagnostic.len as i32
    }

    fn is_empty(&self) -> bool {
        self.diagnostic.len == 0
    }

    fn code(&self) -> i32 {
        self.diagnostic.code
    }

    fn category(&self) -> diagnostics::Category {
        match self.diagnostic.category.as_str() {
            "warning" => diagnostics::Category::Warning,
            "suggestion" => diagnostics::Category::Suggestion,
            "message" => diagnostics::Category::Message,
            _ => diagnostics::Category::Error,
        }
    }

    fn localize(&self, _locale: locale::Locale) -> String {
        remove_test_path_prefixes(&self.diagnostic.message)
    }

    fn message_chain(&self) -> Vec<Box<dyn diagnosticwriter::Diagnostic>> {
        Vec::new()
    }

    fn related_information(&self) -> Vec<Box<dyn diagnosticwriter::Diagnostic>> {
        self.diagnostic
            .related
            .iter()
            .map(|diagnostic| {
                Box::new(BaselineDiagnostic {
                    diagnostic: diagnostic.clone(),
                    input_files: self.input_files.clone(),
                }) as Box<dyn diagnosticwriter::Diagnostic>
            })
            .collect()
    }
}

pub fn diagnostic_from_ast(diagnostic: &ast::Diagnostic) -> Diagnostic {
    diagnostic_from_ast_with_files(diagnostic, &[])
}

pub fn diagnostic_from_ast_with_files(
    diagnostic: &ast::Diagnostic,
    input_files: &[TestFile],
) -> Diagnostic {
    let sort_pos = diagnostic.pos();
    let sort_end = diagnostic.end();
    let pos = sort_pos.max(0) as usize;
    let (file_name, line, character) = if let Some(file) = diagnostic.file() {
        let text = file.text();
        let (line, character) = scanner::get_ecma_line_and_utf16_character_of_position(
            DiagnosticSource { text: &text },
            pos,
        );
        (Some(file.file_name().to_string()), line, character as usize)
    } else {
        (None, 0, 0)
    };
    Diagnostic {
        file_name,
        sort_pos,
        sort_end,
        pos,
        len: diagnostic.len().max(0) as usize,
        line,
        character,
        code: diagnostic.code(),
        category: diagnostic.category().name().to_owned(),
        message: flatten_diagnostic_message(diagnostic),
        related: diagnostic
            .related_information()
            .iter()
            .map(|diagnostic| diagnostic_from_ast_with_files(diagnostic, input_files))
            .collect(),
    }
}

fn flatten_diagnostic_message(diagnostic: &ast::Diagnostic) -> String {
    let mut output = diagnostic.to_string();
    for chain in diagnostic.message_chain() {
        flatten_diagnostic_message_chain(&mut output, chain, 1);
    }
    output
}

fn flatten_diagnostic_message_chain(
    output: &mut String,
    diagnostic: &ast::Diagnostic,
    level: usize,
) {
    output.push_str(HARNESS_NEW_LINE);
    output.push_str(&"  ".repeat(level));
    output.push_str(&diagnostic.to_string());
    for child in diagnostic.message_chain() {
        flatten_diagnostic_message_chain(output, child, level + 1);
    }
}

pub fn do_error_baseline(
    baseline_path: &str,
    input_files: &[TestFile],
    errors: &[Diagnostic],
    pretty: bool,
    opts: baseline::Options,
) -> Result<(), String> {
    let baseline_path = replace_ts_extension(baseline_path, ".errors.txt");
    let error_baseline = if errors.is_empty() {
        baseline::NO_CONTENT.to_string()
    } else {
        get_error_baseline(input_files, errors, pretty)
    };
    baseline::run(&baseline_path, &error_baseline, opts)
}

pub fn minimal_diagnostics_to_string(
    input_files: &[TestFile],
    diagnostics: &[Diagnostic],
    pretty: bool,
) -> String {
    if pretty {
        return pretty_minimal_diagnostics_to_string(input_files, diagnostics);
    }
    diagnostics
        .iter()
        .flat_map(format_minimal_diagnostic_lines)
        .collect::<Vec<_>>()
        .join(HARNESS_NEW_LINE)
}

pub fn get_error_baseline(
    input_files: &[TestFile],
    diagnostics: &[Diagnostic],
    pretty: bool,
) -> String {
    let mut output_lines = iterate_error_baseline(input_files, diagnostics, pretty);
    if pretty {
        output_lines.push(pretty_error_summary_to_string(input_files, diagnostics));
    }
    output_lines.join("")
}

pub fn iterate_error_baseline(
    input_files: &[TestFile],
    input_diagnostics: &[Diagnostic],
    pretty: bool,
) -> Vec<String> {
    let mut diagnostics = input_diagnostics.to_vec();
    diagnostics.sort_by(compare_diagnostics);

    let mut result = Vec::new();
    let top_diagnostics = remove_test_path_prefixes(&minimal_diagnostics_to_string(
        input_files,
        &diagnostics,
        pretty,
    ));
    let top_diagnostics = DIAGNOSTICS_LOCATION_PREFIX.replace_all(&top_diagnostics, "$1(--,--)");
    result.push(format!(
        "{}{}{}",
        top_diagnostics, HARNESS_NEW_LINE, HARNESS_NEW_LINE
    ));

    for diagnostic in diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.file_name.is_none())
    {
        result.push(format_diagnostic_line(diagnostic));
    }

    for input_file in input_files {
        let file_errors = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .file_name
                    .as_ref()
                    .is_some_and(|file_name| same_test_file_path(file_name, &input_file.unit_name))
            })
            .collect::<Vec<_>>();
        result.push(format!(
            "{}==== {} ({} errors) ====",
            HARNESS_NEW_LINE,
            remove_test_path_prefixes(&input_file.unit_name),
            file_errors.len()
        ));
        let lines = line_ranges(&input_file.content);
        for (line_index, line) in lines.iter().enumerate() {
            let is_last_line = line_index + 1 == lines.len();
            let line_text = line.text.trim_end_matches('\r');
            result.push(format!("{}    {}", HARNESS_NEW_LINE, line_text));
            for diagnostic in &file_errors {
                if !diagnostic_overlaps_line(diagnostic, line.start, line.end, is_last_line) {
                    continue;
                }

                let squiggle_start = diagnostic.pos.saturating_sub(line.start);
                let diagnostic_end = diagnostic.pos.saturating_add(diagnostic.len);
                let squiggle_end = diagnostic_end.min(line.end).saturating_sub(line.start);
                let squiggle_start = squiggle_start.min(line_text.len());
                let squiggle_end = squiggle_end.min(line_text.len());
                let squiggle_prefix = replace_non_whitespace_with_space(
                    line_text.get(..squiggle_start).unwrap_or_default(),
                );
                let squiggle_len = line_text
                    .get(squiggle_start..squiggle_end)
                    .map(str::chars)
                    .map(Iterator::count)
                    .unwrap_or_else(|| squiggle_end.saturating_sub(squiggle_start));
                result.push(format!(
                    "{}    {}{}",
                    HARNESS_NEW_LINE,
                    squiggle_prefix,
                    "~".repeat(squiggle_len)
                ));

                if is_last_line || line.end > diagnostic_end {
                    result.push(format_diagnostic_line(diagnostic));
                }
            }
        }
    }

    result
}

fn compare_diagnostics(a: &Diagnostic, b: &Diagnostic) -> Ordering {
    a.file_name
        .cmp(&b.file_name)
        .then(a.sort_pos.cmp(&b.sort_pos))
        .then(a.sort_end.cmp(&b.sort_end))
        .then(a.code.cmp(&b.code))
        .then(a.message.cmp(&b.message))
        .then_with(|| compare_related_info(&a.related, &b.related))
}

fn compare_related_info(a: &[Diagnostic], b: &[Diagnostic]) -> Ordering {
    b.len().cmp(&a.len()).then_with(|| {
        a.iter()
            .zip(b)
            .map(|(a, b)| compare_diagnostics(a, b))
            .find(|order| *order != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    })
}

fn pretty_minimal_diagnostics_to_string(
    input_files: &[TestFile],
    diagnostics: &[Diagnostic],
) -> String {
    let diagnostics = baseline_diagnostic_boxes(input_files, diagnostics);
    let format_opts = pretty_formatting_options();
    let mut output = Vec::new();
    diagnosticwriter::format_diagnostics_with_color_and_context(
        &mut output,
        &diagnostics,
        &format_opts,
    );
    String::from_utf8(output).unwrap_or_default()
}

fn pretty_error_summary_to_string(input_files: &[TestFile], diagnostics: &[Diagnostic]) -> String {
    let diagnostics = baseline_diagnostic_boxes(input_files, diagnostics);
    let format_opts = pretty_formatting_options();
    let mut output = Vec::new();
    diagnosticwriter::write_error_summary_text(&mut output, diagnostics, &format_opts);
    String::from_utf8(output).unwrap_or_default()
}

fn baseline_diagnostic_boxes(
    input_files: &[TestFile],
    diagnostics: &[Diagnostic],
) -> Vec<Box<dyn diagnosticwriter::Diagnostic>> {
    let input_files = Arc::new(
        input_files
            .iter()
            .map(|input_file| BaselineFile {
                file_name: remove_test_path_prefixes(&input_file.unit_name),
                text: input_file.content.clone(),
            })
            .collect::<Vec<_>>(),
    );
    diagnostics
        .iter()
        .map(|diagnostic| {
            Box::new(BaselineDiagnostic {
                diagnostic: diagnostic.clone(),
                input_files: input_files.clone(),
            }) as Box<dyn diagnosticwriter::Diagnostic>
        })
        .collect()
}

fn pretty_formatting_options() -> diagnosticwriter::FormattingOptions {
    diagnosticwriter::FormattingOptions {
        locale: locale::Locale::default(),
        compare_paths_options: tspath::ComparePathsOptions {
            current_directory: String::new(),
            use_case_sensitive_file_names: true,
        },
        new_line: HARNESS_NEW_LINE.to_owned(),
    }
}

pub fn format_location(file_name: &str, line: usize, character: usize) -> String {
    format!("{file_name}:{}:{}", line + 1, character + 1)
}

fn format_diagnostic_line(diagnostic: &Diagnostic) -> String {
    format_diagnostic_lines(diagnostic).join("")
}

fn format_diagnostic_lines(diagnostic: &Diagnostic) -> Vec<String> {
    let mut lines = Vec::new();
    for line in diagnostic.message.lines() {
        if !line.trim().is_empty() {
            lines.push(format!(
                "{}!!! {} TS{}: {}",
                HARNESS_NEW_LINE,
                diagnostic.category,
                diagnostic.code,
                remove_test_path_prefixes(line.trim_end_matches('\r'))
            ));
        }
    }

    for related in &diagnostic.related {
        let location = related
            .file_name
            .as_ref()
            .map(|file_name| {
                if is_default_library_file(file_name) {
                    format!(
                        " {}:--:--",
                        remove_test_path_prefixes(&default_library_file_name(file_name))
                    )
                } else {
                    format!(
                        " {}",
                        remove_test_path_prefixes(&format_location(
                            file_name,
                            related.line,
                            related.character,
                        ))
                    )
                }
            })
            .unwrap_or_default();
        lines.push(format!(
            "{}!!! related TS{}{}: {}",
            HARNESS_NEW_LINE,
            related.code,
            location,
            remove_test_path_prefixes(&related.message)
        ));
    }

    lines
}

fn format_minimal_diagnostic_lines(diagnostic: &Diagnostic) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, line) in diagnostic.message.lines().enumerate() {
        if !line.trim().is_empty() {
            if index != 0 {
                lines.push(remove_test_path_prefixes(line.trim_end_matches('\r')));
                continue;
            }
            let location = diagnostic
                .file_name
                .as_ref()
                .map(|file_name| {
                    format!(
                        "{}({},{}): ",
                        remove_test_path_prefixes(file_name),
                        diagnostic.line + 1,
                        diagnostic.character + 1
                    )
                })
                .unwrap_or_default();
            lines.push(format!(
                "{}{} TS{}: {}",
                location,
                diagnostic.category,
                diagnostic.code,
                remove_test_path_prefixes(line.trim_end_matches('\r'))
            ));
        }
    }
    lines
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LineRange<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

fn line_ranges(text: &str) -> Vec<LineRange<'_>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if ch != '\n' {
            continue;
        }
        ranges.push(LineRange {
            start,
            end: index + ch.len_utf8(),
            text: &text[start..index],
        });
        start = index + ch.len_utf8();
    }
    ranges.push(LineRange {
        start,
        end: text.len(),
        text: &text[start..],
    });
    ranges
}

fn diagnostic_overlaps_line(
    diagnostic: &Diagnostic,
    line_start: usize,
    line_end: usize,
    is_last_line: bool,
) -> bool {
    let diagnostic_end = diagnostic.pos.saturating_add(diagnostic.len);
    diagnostic_end >= line_start && (diagnostic.pos < line_end || is_last_line)
}

fn replace_non_whitespace_with_space(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_whitespace() { ch } else { ' ' })
        .collect()
}

fn replace_ts_extension(path: &str, replacement: &str) -> String {
    for ext in [".tsx", ".ts"] {
        if let Some(prefix) = path.strip_suffix(ext) {
            return format!("{prefix}{replacement}");
        }
    }
    format!("{path}{replacement}")
}

fn remove_test_path_prefixes(path: &str) -> String {
    path.replace("/.src/", "")
        .replace("/.lib/", "")
        .replace("/.ts/", "")
        .replace("bundled:///libs/", "")
        .replace("file:///./src/", "file:///")
        .replace("file:///./lib/", "file:///")
        .replace("file:///./ts/", "file:///")
}

fn same_test_file_path(left: &str, right: &str) -> bool {
    tspath::normalize_path(&remove_test_path_prefixes(left))
        == tspath::normalize_path(&remove_test_path_prefixes(right))
}

fn is_default_library_file(file_path: &str) -> bool {
    let file_name = default_library_file_name(file_path);
    file_name.starts_with("lib.") && file_name.ends_with(".d.ts")
}

fn default_library_file_name(file_path: &str) -> String {
    file_path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(file_path)
        .to_owned()
}
