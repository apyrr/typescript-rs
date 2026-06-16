use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ts_lsproto::{self as lsproto, DocumentUriExt};

use crate::{FourslashTest, get_base_file_name_from_test};

pub const AUTO_IMPORTS_CMD: BaselineCommand = BaselineCommand("Auto Imports");
pub const CALL_HIERARCHY_CMD: BaselineCommand = BaselineCommand("Call Hierarchy");
pub const CLOSING_TAG_CMD: BaselineCommand = BaselineCommand("Closing Tag");
pub const COMPLETIONS_CMD: BaselineCommand = BaselineCommand("Completions");
pub const DOCUMENT_HIGHLIGHTS_CMD: BaselineCommand = BaselineCommand("documentHighlights");
pub const FIND_ALL_REFERENCES_CMD: BaselineCommand = BaselineCommand("findAllReferences");
pub const GO_TO_DEFINITION_CMD: BaselineCommand = BaselineCommand("goToDefinition");
pub const GO_TO_IMPLEMENTATION_CMD: BaselineCommand = BaselineCommand("goToImplementation");
pub const GO_TO_SOURCE_DEFINITION_CMD: BaselineCommand = BaselineCommand("goToSourceDefinition");
pub const GO_TO_TYPE_DEFINITION_CMD: BaselineCommand = BaselineCommand("goToType");
pub const INLAY_HINTS_CMD: BaselineCommand = BaselineCommand("Inlay Hints");
pub const NON_SUGGESTION_DIAGNOSTICS_CMD: BaselineCommand =
    BaselineCommand("Syntax and Semantic Diagnostics");
pub const QUICK_INFO_CMD: BaselineCommand = BaselineCommand("QuickInfo");
pub const LINKED_EDITING_CMD: BaselineCommand = BaselineCommand("linkedEditing");
pub const RENAME_CMD: BaselineCommand = BaselineCommand("findRenameLocations");
pub const SIGNATURE_HELP_CMD: BaselineCommand = BaselineCommand("SignatureHelp");
pub const SMART_SELECTION_CMD: BaselineCommand = BaselineCommand("Smart Selection");
pub const CODE_LENSES_CMD: BaselineCommand = BaselineCommand("Code Lenses");
pub const DOCUMENT_SYMBOLS_CMD: BaselineCommand = BaselineCommand("Document Symbols");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BaselineCommand(pub &'static str);

impl FourslashTest {
    pub fn add_result_to_baseline(
        &mut self,
        _t: &mut TestingT,
        command: BaselineCommand,
        actual: String,
    ) {
        let b = if self.test_data.is_state_baselining_enabled() {
            // Single baseline for all commands
            &mut self
                .state_baseline
                .as_mut()
                .expect("state baselining enabled without state baseline")
                .baseline
        } else {
            self.baselines.entry(command).or_default()
        };
        if !b.is_empty() {
            b.push_str("\n\n\n\n");
        }
        b.push_str("// === ");
        b.push_str(command.0);
        b.push_str(" ===\n");
        b.push_str(&actual);
    }

    pub fn write_to_baseline(&mut self, command: BaselineCommand, content: String) {
        self.baselines
            .entry(command)
            .or_default()
            .push_str(&content);
    }

    pub fn get_baseline_options(
        &self,
        command: BaselineCommand,
        test_path: &str,
    ) -> BaselineOptions {
        let subfolder = format!("fourslash/{}", normalize_command_name(command.0));
        if !is_submodule_test(test_path) {
            return BaselineOptions {
                subfolder,
                ..BaselineOptions::default()
            };
        }
        match command {
            SMART_SELECTION_CMD => BaselineOptions {
                subfolder,
                is_submodule: true,
                ..BaselineOptions::default()
            },
            CALL_HIERARCHY_CMD => BaselineOptions {
                subfolder,
                is_submodule: true,
                diff_fixup_old: Some(Box::new(|mut s| {
                    // TypeScript baselines have "/tests/cases/fourslash/" prefix in file paths
                    // Handle /server/ subdirectory - need to remove both prefixes
                    s = s.replace("/tests/cases/fourslash/server/", "/");
                    s = s.replace("/tests/cases/fourslash/", "/");
                    // SymbolKind enum differences between Strada and tsgo
                    s = s.replace("kind: getter", "kind: property");
                    s = s.replace("kind: script", "kind: file");
                    s
                })),
                ..BaselineOptions::default()
            },
            RENAME_CMD => BaselineOptions {
                subfolder,
                is_submodule: true,
                diff_fixup_old: Some(Box::new(|s| {
                    let mut command_lines = Vec::new();
                    let test_file_prefix = "/tests/cases/fourslash";
                    let server_test_file_prefix = "/server";
                    let context_span_opening = "<|";
                    let context_span_closing = "|>";
                    let old_preference = "providePrefixAndSuffixTextForRename";
                    let new_preference = "useAliasesForRename";
                    let mut is_in_command = false;
                    for line in s.lines() {
                        if line.starts_with("// @findInStrings: ")
                            || line.starts_with("// @findInComments: ")
                        {
                            continue;
                        }
                        if let Some(command_name) = command_header(line) {
                            is_in_command = command_name == RENAME_CMD.0;
                        }
                        if is_in_command {
                            let fixed_line = line
                                .replace(context_span_opening, "")
                                .replace(context_span_closing, "")
                                .replace(test_file_prefix, "")
                                .replace(server_test_file_prefix, "")
                                .replace(old_preference, new_preference);
                            command_lines.push(fixed_line);
                        }
                    }
                    drop_trailing_empty_lines(command_lines).join("\n")
                })),
                ..BaselineOptions::default()
            },
            INLAY_HINTS_CMD => BaselineOptions {
                subfolder,
                is_submodule: true,
                diff_fixup_old: Some(Box::new(fixup_old_inlay_hints)),
                diff_fixup_new: Some(Box::new(fixup_new_inlay_hints)),
            },
            GO_TO_DEFINITION_CMD
            | GO_TO_TYPE_DEFINITION_CMD
            | GO_TO_IMPLEMENTATION_CMD
            | GO_TO_SOURCE_DEFINITION_CMD => BaselineOptions {
                subfolder,
                is_submodule: true,
                diff_fixup_old: Some(Box::new(move |s| fixup_old_go_to(command, s))),
                diff_fixup_new: Some(Box::new(|s| s.replace("bundled:///libs/", ""))),
            },
            FIND_ALL_REFERENCES_CMD => BaselineOptions {
                subfolder,
                is_submodule: true,
                diff_fixup_old: Some(Box::new(fixup_old_find_all_references)),
                ..BaselineOptions::default()
            },
            LINKED_EDITING_CMD => BaselineOptions {
                subfolder,
                is_submodule: true,
                diff_fixup_old: Some(Box::new(delete_linked_editing_info)),
                diff_fixup_new: Some(Box::new(delete_linked_editing_info)),
            },
            _ => BaselineOptions {
                subfolder,
                ..BaselineOptions::default()
            },
        }
    }

    pub fn get_baseline_for_locations_with_file_contents(
        &self,
        locations: Vec<lsproto::Location>,
        options: BaselineFourslashLocationsOptions,
    ) -> String {
        self.get_baseline_for_spans_with_file_contents(
            locations.into_iter().map(location_to_span).collect(),
            options,
        )
    }

    pub fn get_baseline_for_spans_with_file_contents(
        &self,
        spans: Vec<DocumentSpan>,
        mut options: BaselineFourslashLocationsOptions,
    ) -> String {
        let mut spans_by_file: BTreeMap<lsproto::DocumentUri, Vec<DocumentSpan>> = BTreeMap::new();
        for span in spans.iter().cloned() {
            spans_by_file
                .entry(span.uri.clone())
                .or_default()
                .push(span);
        }
        if options.preserve_result_order {
            options.ordered_files = unique_files_in_span_order(&spans);
        }
        self.get_baseline_for_grouped_spans_with_file_contents(spans_by_file, options)
    }

    pub fn get_baseline_for_grouped_spans_with_file_contents(
        &self,
        grouped_ranges: BTreeMap<lsproto::DocumentUri, Vec<DocumentSpan>>,
        options: BaselineFourslashLocationsOptions,
    ) -> String {
        // We must always print the file containing the marker,
        // but don't want to print it twice at the end if it already
        // found in a file with ranges.
        let mut found_marker = false;
        let mut found_additional_location = false;
        let mut span_to_context_id: HashMap<DocumentSpan, usize> = HashMap::new();

        let mut baseline_entries = Vec::new();
        let mut add_file_entry = |path: String| {
            let file_name = file_name_to_document_uri(&path);
            let ranges = grouped_ranges.get(&file_name).cloned().unwrap_or_default();
            if ranges.is_empty() {
                return;
            }

            let Some(content) = self.text_of_file(&path).map(|(text, _)| text) else {
                return;
            };

            if options
                .marker
                .as_ref()
                .is_some_and(|marker| marker.file_name() == path)
            {
                found_marker = true;
            }

            if options
                .additional_span
                .as_ref()
                .is_some_and(|span| span.uri == file_name)
            {
                found_additional_location = true;
            }

            baseline_entries.push(self.get_baseline_content_for_file(
                path,
                content,
                ranges,
                &mut span_to_context_id,
                options.clone(),
            ));
        };

        if options.preserve_result_order {
            for uri in options.ordered_files.iter() {
                add_file_entry(uri.file_name());
            }
        } else {
            for path in self.vfs.files.keys().cloned().collect::<Vec<_>>() {
                add_file_entry(path);
            }
        }

        // In Strada, there is a bug where we only ever add additional spans to baselines if we haven't
        // already added the file to the baseline.
        if let Some(additional_span) = options.additional_span.as_ref() {
            if !found_additional_location {
                let file_name = additional_span.uri.file_name();
                if let Some((content, _)) = self.text_of_file(&file_name) {
                    baseline_entries.push(self.get_baseline_content_for_file(
                        file_name.clone(),
                        content,
                        vec![additional_span.clone()],
                        &mut span_to_context_id,
                        options.clone(),
                    ));
                    if options
                        .marker
                        .as_ref()
                        .is_some_and(|marker| marker.file_name() == file_name)
                    {
                        found_marker = true;
                    }
                }
            }
        }

        if !found_marker {
            if let Some(marker) = options.marker.as_ref() {
                // If we didn't find the marker in any file, we need to add it.
                let marker_file_name = marker.file_name();
                if let Some((content, _)) = self.text_of_file(&marker_file_name) {
                    baseline_entries.push(self.get_baseline_content_for_file(
                        marker_file_name,
                        content,
                        Vec::new(),
                        &mut span_to_context_id,
                        options,
                    ));
                }
            }
        }

        // !!! skipDocumentContainingOnlyMarker

        baseline_entries.join("\n\n")
    }

    pub fn text_of_file(&self, file_name: &str) -> Option<(String, bool)> {
        if self.open_files.contains(file_name) {
            return Some((self.get_script_info(file_name).content.clone(), true));
        }
        self.vfs.read_file(file_name).map(|text| (text, true))
    }

    pub fn get_baseline_content_for_file(
        &self,
        file_name: String,
        content: String,
        spans_in_file: Vec<DocumentSpan>,
        span_to_context_id: &mut HashMap<DocumentSpan, usize>,
        options: BaselineFourslashLocationsOptions,
    ) -> String {
        let mut details: Vec<BaselineDetail> = Vec::new();
        let mut detail_prefixes: HashMap<usize, String> = HashMap::new();
        let mut detail_suffixes: HashMap<usize, String> = HashMap::new();
        let mut can_determine_context_id_inline = true;
        let mut next_detail_id = 0usize;
        let mut next_span_id = 0usize;

        if options
            .marker
            .as_ref()
            .is_some_and(|marker| marker.file_name() == file_name)
        {
            let marker = options.marker.as_ref().unwrap();
            details.push(BaselineDetail {
                id: next_detail_id,
                pos: marker.ls_pos(),
                position_marker: options.marker_name.clone(),
                span: None,
                span_id: None,
                kind: DETAIL_KIND_MARKER,
            });
            next_detail_id += 1;
        }

        for span in spans_in_file.iter().cloned() {
            let span_id = next_span_id;
            next_span_id += 1;
            let context_span_index = details.len();

            // Add context span markers if present
            if let Some(context_span) = span.context_span.clone() {
                details.push(BaselineDetail {
                    id: next_detail_id,
                    pos: context_span.start,
                    position_marker: "<|".to_string(),
                    span: Some(span.clone()),
                    span_id: Some(span_id),
                    kind: DETAIL_KIND_CONTEXT_START,
                });
                next_detail_id += 1;

                // Check if context span starts after text span
                if compare_positions(context_span.start, span.text_span.start) > 0 {
                    can_determine_context_id_inline = false;
                }
            }

            let text_span_index = details.len();
            let mut start_marker = "[|".to_string();
            if let Some(get_location_data) = options.get_location_data.as_ref() {
                start_marker.push_str(&get_location_data(span.clone()));
            }
            details.push(BaselineDetail {
                id: next_detail_id,
                pos: span.text_span.start,
                position_marker: start_marker,
                span: Some(span.clone()),
                span_id: Some(span_id),
                kind: DETAIL_KIND_TEXT_START,
            });
            next_detail_id += 1;
            details.push(BaselineDetail {
                id: next_detail_id,
                pos: span.text_span.end,
                position_marker: if options.end_marker.is_empty() {
                    "|]".to_string()
                } else {
                    options.end_marker.clone()
                },
                span: Some(span.clone()),
                span_id: Some(span_id),
                kind: DETAIL_KIND_TEXT_END,
            });
            next_detail_id += 1;

            if let Some(context_span) = span.context_span.clone() {
                details.push(BaselineDetail {
                    id: next_detail_id,
                    pos: context_span.end,
                    position_marker: "|>".to_string(),
                    span: Some(span.clone()),
                    span_id: Some(span_id),
                    kind: DETAIL_KIND_CONTEXT_END,
                });
                next_detail_id += 1;
            }

            if let Some(start_marker_prefix) = options.start_marker_prefix.as_ref() {
                if let Some(start_prefix) = start_marker_prefix(span.clone()) {
                    // Special case: if this span starts at the same position as the provided marker,
                    // we want the span's prefix to appear before the marker name.
                    // i.e. We want `/*START PREFIX*/A: /*RENAME*/[|ARENAME|]`,
                    // not `/*RENAME*//*START PREFIX*/A: [|ARENAME|]`
                    if options.marker.as_ref().is_some_and(|marker| {
                        file_name == marker.file_name() && span.text_span.start == marker.ls_pos()
                    }) {
                        detail_prefixes.insert(details[0].id, start_prefix);
                    } else if span
                        .context_span
                        .as_ref()
                        .is_some_and(|context_span| context_span.start == span.text_span.start)
                    {
                        detail_prefixes.insert(details[context_span_index].id, start_prefix);
                    } else {
                        detail_prefixes.insert(details[text_span_index].id, start_prefix);
                    }
                }
            }

            if let Some(end_marker_suffix) = options.end_marker_suffix.as_ref() {
                if let Some(end_suffix) = end_marker_suffix(span.clone()) {
                    // Same as above for suffixes:
                    if options.marker.as_ref().is_some_and(|marker| {
                        file_name == marker.file_name() && span.text_span.end == marker.ls_pos()
                    }) {
                        detail_suffixes.insert(details[0].id, end_suffix);
                    } else if span
                        .context_span
                        .as_ref()
                        .is_some_and(|context_span| context_span.end == span.text_span.end)
                    {
                        detail_suffixes.insert(details[text_span_index + 2].id, end_suffix);
                    } else {
                        detail_suffixes.insert(details[text_span_index + 1].id, end_suffix);
                    }
                }
            }
        }

        // Our preferred way to write markers is
        // /*MARKER*/[| some text |]
        // [| some /*MARKER*/ text |]
        // [| some text |]/*MARKER*/
        details.sort_by(|d1, d2| compare_details(d1, d2));
        // !!! if canDetermineContextIdInline

        let mut text_with_context = new_text_with_context(file_name, content);
        for (index, detail) in details.iter().cloned().enumerate() {
            text_with_context.add(Some(detail.clone()));
            text_with_context.pos = detail.pos;
            // Prefix
            if let Some(prefix) = detail_prefixes.get(&detail.id) {
                text_with_context.new_content.push_str(prefix);
            }
            text_with_context
                .new_content
                .push_str(&detail.position_marker);
            if let Some(span) = detail.span.as_ref() {
                match detail.kind {
                    DETAIL_KIND_TEXT_START => {
                        let mut text = String::new();
                        if let Some(context_id) = span_to_context_id.get(span) {
                            let mut is_after_context_start = false;
                            for text_start_detail in details[..index].iter().rev() {
                                if text_start_detail.kind == DETAIL_KIND_CONTEXT_START
                                    && text_start_detail.span_id == detail.span_id
                                {
                                    is_after_context_start = true;
                                    break;
                                }
                                // Marker is ok to skip over
                                if text_start_detail.span.is_some() {
                                    break;
                                }
                            }
                            // Skip contextId on span thats surrounded by context span immediately
                            if !is_after_context_start {
                                text = format!("contextId: {context_id}");
                            }
                        }
                        if !text.is_empty() {
                            text_with_context.new_content.push_str("{ ");
                            text_with_context.new_content.push_str(&text);
                            text_with_context.new_content.push_str(" |}");
                        }
                    }
                    DETAIL_KIND_CONTEXT_START => {
                        if can_determine_context_id_inline {
                            span_to_context_id.insert(span.clone(), span_to_context_id.len());
                        }
                    }
                    _ => {}
                }
            }
            if let Some(suffix) = detail_suffixes.get(&detail.id) {
                text_with_context.new_content.push_str(suffix);
            }
        }
        text_with_context.add(None);
        if !text_with_context.new_content.is_empty() {
            text_with_context.readable_contents.push('\n');
            text_with_context.readable_jsonc_baseline(text_with_context.new_content.clone());
        }
        text_with_context.readable_contents
    }
}

pub fn get_baseline_file_name(t: &TestingT, command: BaselineCommand) -> String {
    format!(
        "{}.{}",
        get_base_file_name_from_test(t),
        get_baseline_extension(command)
    )
}

pub fn get_baseline_extension(command: BaselineCommand) -> &'static str {
    match command {
        QUICK_INFO_CMD
        | SIGNATURE_HELP_CMD
        | SMART_SELECTION_CMD
        | INLAY_HINTS_CMD
        | NON_SUGGESTION_DIAGNOSTICS_CMD
        | DOCUMENT_SYMBOLS_CMD
        | CLOSING_TAG_CMD => "baseline",
        CALL_HIERARCHY_CMD => "callHierarchy.txt",
        AUTO_IMPORTS_CMD => "baseline.md",
        LINKED_EDITING_CMD => "linkedEditing.txt",
        _ => "baseline.jsonc",
    }
}

pub fn drop_trailing_empty_lines(mut ss: Vec<String>) -> Vec<String> {
    while ss.last().is_some_and(|s| s.is_empty()) {
        ss.pop();
    }
    ss
}

pub fn is_submodule_test(test_path: &str) -> bool {
    test_path.contains("fourslash/tests/gen") || test_path.contains("fourslash/tests/manual")
}

pub fn normalize_command_name(command: &str) -> String {
    let mut command = command.split_whitespace().collect::<Vec<_>>().join("");
    if let Some(first) = command.get_mut(0..1) {
        first.make_ascii_lowercase();
    }
    command
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSpan {
    pub uri: lsproto::DocumentUri,
    pub text_span: lsproto::Range,
    pub context_span: Option<lsproto::Range>,
}

impl Hash for DocumentSpan {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uri.hash(state);
        self.text_span.start.line.hash(state);
        self.text_span.start.character.hash(state);
        self.text_span.end.line.hash(state);
        self.text_span.end.character.hash(state);
        if let Some(context_span) = self.context_span {
            true.hash(state);
            context_span.start.line.hash(state);
            context_span.start.character.hash(state);
            context_span.end.line.hash(state);
            context_span.end.character.hash(state);
        } else {
            false.hash(state);
        }
    }
}

#[derive(Clone)]
pub struct BaselineFourslashLocationsOptions {
    // markerInfo
    pub marker: Option<MarkerOrRange>, // location
    pub marker_name: String,           // name of the marker to be printed in baseline

    pub end_marker: String,

    pub start_marker_prefix: Option<Arc<dyn Fn(DocumentSpan) -> Option<String>>>,
    pub end_marker_suffix: Option<Arc<dyn Fn(DocumentSpan) -> Option<String>>>,
    pub get_location_data: Option<Arc<dyn Fn(DocumentSpan) -> String>>,

    pub additional_span: Option<DocumentSpan>,
    pub preserve_result_order: bool,
    pub ordered_files: Vec<lsproto::DocumentUri>,
}

pub fn location_to_span(loc: lsproto::Location) -> DocumentSpan {
    DocumentSpan {
        uri: loc.uri,
        text_span: loc.range,
        context_span: None,
    }
}

pub fn unique_files_in_span_order(spans: &[DocumentSpan]) -> Vec<lsproto::DocumentUri> {
    if spans.is_empty() {
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::with_capacity(spans.len());
    for span in spans {
        if seen.contains(&span.uri) {
            continue;
        }
        seen.insert(span.uri.clone());
        result.push(span.uri.clone());
    }
    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetailKind(pub i32);

pub const DETAIL_KIND_MARKER: DetailKind = DetailKind(0); // /*MARKER*/
pub const DETAIL_KIND_CONTEXT_START: DetailKind = DetailKind(1); // <|
pub const DETAIL_KIND_TEXT_START: DetailKind = DetailKind(2); // [|
pub const DETAIL_KIND_TEXT_END: DetailKind = DetailKind(3); // |]
pub const DETAIL_KIND_CONTEXT_END: DetailKind = DetailKind(4); // |>

impl DetailKind {
    pub fn is_end(self) -> bool {
        self == DETAIL_KIND_CONTEXT_END || self == DETAIL_KIND_TEXT_END
    }

    pub fn is_start(self) -> bool {
        self == DETAIL_KIND_CONTEXT_START || self == DETAIL_KIND_TEXT_START
    }
}

#[derive(Clone)]
pub struct BaselineDetail {
    pub id: usize,
    pub pos: lsproto::Position,
    pub position_marker: String,
    pub span: Option<DocumentSpan>,
    pub span_id: Option<usize>,
    pub kind: DetailKind,
}

impl BaselineDetail {
    pub fn get_range(&self) -> lsproto::Range {
        match self.kind {
            DETAIL_KIND_CONTEXT_START | DETAIL_KIND_CONTEXT_END => {
                self.span.as_ref().unwrap().context_span.unwrap()
            }
            DETAIL_KIND_TEXT_START | DETAIL_KIND_TEXT_END => self.span.as_ref().unwrap().text_span,
            DETAIL_KIND_MARKER => lsproto::Range {
                start: self.pos,
                end: self.pos,
            },
            _ => panic!("unknown detail kind"),
        }
    }
}

pub struct TextWithContext {
    pub n_lines_context: usize, // number of context lines to write to baseline

    pub readable_contents: String, // builds what will be returned to be written to baseline

    pub new_content: String, // helper; the part of the original file content to write between details
    pub pos: lsproto::Position,
    pub is_lib_file: bool,
    pub file_name: String,
    pub content: String, // content of the original file
    pub line_starts: Vec<usize>,

    // posLineInfo
    pub pos_info: Option<lsproto::Position>,
    pub line_info: usize,
}

// implements lsconv.Script
impl TextWithContext {
    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }

    // implements lsconv.Script
    pub fn text(&self) -> String {
        self.content.clone()
    }

    pub fn add(&mut self, detail: Option<BaselineDetail>) {
        if self.new_content.is_empty() && detail.is_none() {
            panic!("Unsupported");
        }
        if detail.as_ref().is_none_or(|detail| {
            detail.kind != DETAIL_KIND_TEXT_END && detail.kind != DETAIL_KIND_CONTEXT_END
        }) {
            // Calculate pos to location number of lines
            let mut pos_line_index = self.line_info;
            if self.pos_info != Some(self.pos) {
                pos_line_index =
                    self.compute_index_of_line_start(self.line_and_character_to_position(self.pos));
            }

            let mut location_line_index = self.line_starts.len() - 1;
            if let Some(detail) = detail.as_ref() {
                location_line_index = self
                    .compute_index_of_line_start(self.line_and_character_to_position(detail.pos));
                self.pos_info = Some(detail.pos);
                self.line_info = location_line_index;
            }

            let mut n_lines = 0;
            if !self.new_content.is_empty() {
                n_lines += self.n_lines_context + 1;
            }
            if detail.is_some() {
                n_lines += self.n_lines_context + 1;
            }
            // first nLinesContext and last nLinesContext
            if location_line_index.saturating_sub(pos_line_index) > n_lines {
                if !self.new_content.is_empty() {
                    let skipped_string = if self.is_lib_file {
                        "--- (line: --) skipped ---\n".to_string()
                    } else {
                        format!(
                            "--- (line: {}) skipped ---",
                            pos_line_index + self.n_lines_context + 1
                        )
                    };

                    self.readable_contents.push('\n');
                    self.readable_jsonc_baseline(format!(
                        "{}{}{}",
                        self.new_content,
                        self.slice_of_content(
                            self.get_index_position(self.pos),
                            Some(self.line_starts[pos_line_index + self.n_lines_context]),
                        ),
                        skipped_string
                    ));

                    if detail.is_some() {
                        self.readable_contents.push('\n');
                    }
                    self.new_content.clear();
                }
                if let Some(detail) = detail {
                    if self.is_lib_file {
                        self.new_content.push_str("--- (line: --) skipped ---\n");
                    } else {
                        writeln!(
                            self.new_content,
                            "--- (line: {}) skipped ---",
                            location_line_index - self.n_lines_context + 1
                        )
                        .ok();
                    }
                    self.new_content.push_str(&self.slice_of_content(
                        Some(self.line_starts[location_line_index - self.n_lines_context + 1]),
                        self.get_index_position(detail.pos),
                    ));
                }
                return;
            }
        }
        if let Some(detail) = detail {
            self.new_content.push_str(&self.slice_of_content(
                self.get_index_position(self.pos),
                self.get_index_position(detail.pos),
            ));
        } else {
            self.new_content
                .push_str(&self.slice_of_content(self.get_index_position(self.pos), None));
        }
    }

    pub fn readable_jsonc_baseline(&mut self, text: String) {
        for (i, line) in line_splitter(&text).into_iter().enumerate() {
            if i > 0 {
                self.readable_contents.push('\n');
            }
            self.readable_contents.push_str("// ");
            self.readable_contents.push_str(&line);
        }
    }

    pub fn slice_of_content(&self, start: Option<usize>, end: Option<usize>) -> String {
        let start = start.unwrap_or(0);
        let end = end.unwrap_or(self.content.len()).min(self.content.len());

        if start > end {
            return String::new();
        }

        self.content[start..end].to_string()
    }

    pub fn get_index(&self, i: IndexInput) -> Option<usize> {
        match i {
            IndexInput::Index(i) => Some(i),
            IndexInput::Position(position) => self.get_index_position(position),
            IndexInput::None => None,
        }
    }

    fn get_index_position(&self, position: lsproto::Position) -> Option<usize> {
        Some(self.line_and_character_to_position(position))
    }

    fn line_and_character_to_position(&self, position: lsproto::Position) -> usize {
        self.line_starts[position.line as usize] + position.character as usize
    }

    fn compute_index_of_line_start(&self, position: usize) -> usize {
        self.line_starts
            .partition_point(|line_start| *line_start <= position)
            .saturating_sub(1)
    }
}

pub fn new_text_with_context(file_name: String, content: String) -> TextWithContext {
    let mut t = TextWithContext {
        n_lines_context: 4,

        readable_contents: String::new(),

        is_lib_file: is_lib_file(&file_name),
        new_content: String::new(),
        pos: lsproto::Position {
            line: 0,
            character: 0,
        },
        file_name,
        content,
        line_starts: Vec::new(),
        pos_info: None,
        line_info: 0,
    };

    t.line_starts = compute_line_starts(&t.content);
    t.readable_contents.push_str("// === ");
    t.readable_contents.push_str(&t.file_name);
    t.readable_contents.push_str(" ===");
    t
}

pub struct MarkerAndItem<T> {
    pub marker: Marker,
    pub item: T,
}

pub fn annotate_content_with_tooltips<T: Clone + Default + PartialEq>(
    _t: &mut TestingT,
    f: &FourslashTest,
    markers_and_items: Vec<MarkerAndItem<T>>,
    op_name: &str,
    get_range: impl Fn(T) -> Option<lsproto::Range>,
    get_tooltip_lines: impl Fn(T, T) -> Vec<String>,
) -> String {
    let bar_with_gutter = format!("| {}", "-".repeat(70));

    // sort by file, then *backwards* by position in the file
    // so we can insert multiple times on a line without counting.
    let mut sorted = markers_and_items;
    sorted.sort_by(|a, b| {
        a.marker
            .file_name()
            .cmp(&b.marker.file_name())
            .then_with(|| b.marker.position.cmp(&a.marker.position))
    });

    let mut files_to_lines: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut previous = T::default();
    for item_and_marker in sorted {
        let marker = item_and_marker.marker;
        let item = item_and_marker.item;

        let text_range = get_range(item.clone()).unwrap_or_else(|| {
            let start = marker.ls_position;
            let mut end = start;
            end.character += 1;
            lsproto::Range { start, end }
        });

        if text_range.start.line != text_range.end.line {
            panic!("Expected text range to be on a single line, got range");
        }
        let underline = format!(
            "{}{}",
            " ".repeat(text_range.start.character as usize),
            "^".repeat((text_range.end.character - text_range.start.character) as usize)
        );

        let file_name = marker.file_name();
        let mut lines = files_to_lines
            .remove(&file_name)
            .unwrap_or_else(|| line_splitter(&f.get_script_info(&file_name).content));

        let mut tooltip_lines = if item != T::default() {
            get_tooltip_lines(item.clone(), previous)
        } else {
            Vec::new()
        };
        if tooltip_lines.is_empty() {
            tooltip_lines = vec![format!(
                "No {} at /*{}*/.",
                op_name,
                marker.name.unwrap_or_default()
            )];
        }
        tooltip_lines = tooltip_lines
            .into_iter()
            .map(|line| format!("| {line}"))
            .collect();

        let mut lines_to_insert = Vec::with_capacity(tooltip_lines.len() + 3);
        lines_to_insert.push(underline);
        lines_to_insert.push(bar_with_gutter.clone());
        lines_to_insert.extend(tooltip_lines);
        lines_to_insert.push(bar_with_gutter.clone());

        lines.splice(
            text_range.start.line as usize + 1..text_range.start.line as usize + 1,
            lines_to_insert,
        );
        files_to_lines.insert(file_name.to_string(), lines);

        previous = item;
    }

    let mut builder = String::new();
    let mut seen_first = false;
    for (file_name, lines) in files_to_lines {
        writeln!(builder, "=== {file_name} ===").ok();
        for line in lines {
            builder.push_str("// ");
            builder.push_str(&line);
            builder.push('\n');
        }

        if seen_first {
            builder.push_str("\n\n");
        } else {
            seen_first = true;
        }
    }

    builder
}

pub enum IndexInput {
    Index(usize),
    Position(lsproto::Position),
    None,
}

pub fn code_fence(lang: &str, code: &str) -> String {
    format!("```{lang}\n{code}\n```")
}

pub fn symbol_information_to_data(symbol: &lsproto::SymbolInformation) -> String {
    format!("{{| name: {}, kind: {:?} |}}", symbol.name, symbol.kind)
}

fn command_header(line: &str) -> Option<&str> {
    line.strip_prefix("// === ")?.strip_suffix(" ===")
}

fn fixup_old_inlay_hints(s: String) -> String {
    let mut command_lines = Vec::new();
    let lines: Vec<_> = s.lines().map(str::to_string).collect();
    let mut is_in_command = false;
    let mut hint_start = 0usize;
    let mut i = 0usize;
    while i < lines.len() {
        let mut line = lines[i].clone();
        if let Some(command_name) = command_header(&line) {
            is_in_command = command_name == INLAY_HINTS_CMD.0;
        }
        if is_in_command {
            if line == "{" {
                hint_start = command_lines.len();
            }
            if line == "}"
                && command_lines
                    .last()
                    .is_some_and(|line: &String| line.ends_with(','))
            {
                if let Some(last) = command_lines.last_mut() {
                    *last = last.trim_end_matches(',').to_string();
                }
            }
            let trimmed_line = line.trim().to_string();
            // Ignore position, already verified via caret.
            if trimmed_line.starts_with("\"position\": ") {
                i += 1;
                continue;
            }
            if trimmed_line.starts_with("\"text\": ") {
                if trimmed_line == "\"text\": \"\"," {
                    i += 1;
                    continue;
                }
                line = line.replacen("\"text\":", "\"label\":", 1);
            }
            if trimmed_line.starts_with("\"kind\": ") {
                match trimmed_line.as_str() {
                    "\"kind\": \"Parameter\"," => {
                        line = line.replacen("\"kind\": \"Parameter\",", "\"kind\": 2,", 1)
                    }
                    "\"kind\": \"Type\"," => {
                        line = line.replacen("\"kind\": \"Type\",", "\"kind\": 1,", 1)
                    }
                    _ => {
                        i += 1;
                        continue;
                    }
                }
            }
            // Compare only text/value of display parts.
            // Record the presence of a span but not its details.
            if trimmed_line.starts_with("\"displayParts\": ") {
                let mut display_part_lines = vec![line.replacen("displayParts", "label", 1)];
                let mut j = i + 1;
                while j < lines.len() {
                    let mut line = lines[j].clone();
                    let trimmed_line = line.trim().to_string();
                    if trimmed_line.starts_with("\"text\": ") {
                        line = line.replacen("\"text\":", "\"value\":", 1);
                    } else if trimmed_line.starts_with("\"span\": ") {
                        display_part_lines.push(format!(
                            "{}{},",
                            line.replacen("span", "location", 1),
                            "}"
                        ));
                        j += 3;
                        continue;
                    } else if trimmed_line.starts_with("\"file\": ") {
                        j += 1;
                        continue;
                    }
                    if trimmed_line == "]" || trimmed_line == "]," {
                        let mut fixed_line = line;
                        if trimmed_line == "]" {
                            fixed_line.push(',');
                        }
                        display_part_lines.push(fixed_line);
                        break;
                    }
                    display_part_lines.push(line);
                    j += 1;
                }
                command_lines.splice(hint_start + 1..hint_start + 1, display_part_lines);
                i = j + 1;
                continue;
            }

            let fixed_line = line
                .replace("\"whitespaceAfter\"", "\"paddingRight\"")
                .replace("\"whitespaceBefore\"", "\"paddingLeft\"");
            command_lines.push(fixed_line);
        }
        i += 1;
    }
    drop_trailing_empty_lines(command_lines).join("\n")
}

fn fixup_new_inlay_hints(s: String) -> String {
    let mut fixed_lines = Vec::new();
    let lines: Vec<_> = s.lines().map(str::to_string).collect();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i].clone();
        let trimmed_line = line.trim();
        if trimmed_line.starts_with("\"position\": ") {
            i += 4;
            continue;
        }
        if trimmed_line.starts_with("\"location\": ") {
            fixed_lines.push(format!("{line}}},"));
            i += 13;
            continue;
        }
        fixed_lines.push(line);
        i += 1;
    }
    fixed_lines.join("\n")
}

fn fixup_old_go_to(command: BaselineCommand, s: String) -> String {
    let mut command_lines = Vec::new();
    let test_file_prefix = "/tests/cases/fourslash";
    let server_test_file_prefix = "/server";
    let old_go_to_def_command = "getDefinitionAtPosition";
    let old_go_to_def_comment = "/*GOTO DEF POS*/";
    let details_str = "// === Details ===";
    let mut is_in_command = false;
    let mut is_in_details = false;
    for line in s.lines() {
        if let Some(command_name) = command_header(line) {
            is_in_details = false;
            is_in_command = command_name == command.0
                || command == GO_TO_DEFINITION_CMD && command_name == old_go_to_def_command;
        }
        if is_in_command {
            if line.contains(details_str) {
                // Drop blank line before details
                command_lines.pop();
                is_in_details = true;
            }
            // We don't diff the details section, since the structure of responses is different.
            if !is_in_details {
                let fixed_line = strip_object_range(
                    &line
                        .replace(test_file_prefix, "")
                        .replace(server_test_file_prefix, "")
                        .replace(old_go_to_def_command, GO_TO_DEFINITION_CMD.0)
                        .replace(old_go_to_def_comment, "/*GOTO DEF*/"),
                );
                command_lines.push(fixed_line);
            } else if line == "  ]" {
                is_in_details = false;
            }
        }
    }
    drop_trailing_empty_lines(command_lines).join("\n")
}

fn fixup_old_find_all_references(s: String) -> String {
    let mut command_lines = Vec::new();
    let test_file_prefix = "/tests/cases/fourslash";
    let server_test_file_prefix = "/server";
    let context_span_opening = "<|";
    let context_span_closing = "|>";
    let definitions_str = "// === Definitions ===";
    let details_str = "// === Details ===";
    let mut is_in_command = false;
    let mut is_in_details = false;
    let mut is_in_definitions = false;

    // Track file sections for sorting
    #[derive(Clone)]
    struct FileSection {
        file_name: String,
        lines: Vec<String>,
    }
    let mut file_sections: Vec<FileSection> = Vec::new();
    let mut current_file_name = String::new();
    let mut current_file_lines: Vec<String> = Vec::new();

    for line in s.lines() {
        if let Some(command_name) = command_header(line) {
            is_in_details = false;
            is_in_definitions = false;
            if command_name == FIND_ALL_REFERENCES_CMD.0 {
                is_in_command = true;
                // Starting a new findAllReferences command block
                if !current_file_name.is_empty() {
                    file_sections.push(FileSection {
                        file_name: current_file_name,
                        lines: current_file_lines,
                    });
                }
                current_file_name = String::new();
                current_file_lines = Vec::new();
                file_sections.sort_by(|a, b| a.file_name.cmp(&b.file_name));
                for section in file_sections.drain(..) {
                    command_lines.extend(drop_trailing_empty_lines(section.lines));
                    command_lines.push(String::new());
                }
                if !command_lines.is_empty() {
                    command_lines.push(String::new());
                    command_lines.push(String::new());
                }
                command_lines.push(
                    line.replace(test_file_prefix, "")
                        .replace(server_test_file_prefix, ""),
                );
                continue;
            } else {
                is_in_command = false;
            }
        }
        if is_in_command {
            if line.contains(definitions_str) || line.contains(details_str) {
                is_in_definitions = line.contains(definitions_str);
                is_in_details = line.contains(details_str);
                // Drop blank line before definitions/details
                if current_file_lines
                    .last()
                    .is_some_and(|line| line.is_empty())
                {
                    current_file_lines.pop();
                }
            }
            // We don't diff the definitions or details sections
            if !(is_in_definitions || is_in_details) {
                let fixed_line = strip_object_range(
                    &line
                        .replace(test_file_prefix, "")
                        .replace(server_test_file_prefix, "")
                        .replace(context_span_opening, "")
                        .replace(context_span_closing, ""),
                );

                if let Some(file_name) = file_header(&fixed_line) {
                    if !current_file_name.is_empty() {
                        file_sections.push(FileSection {
                            file_name: current_file_name,
                            lines: current_file_lines,
                        });
                    }
                    current_file_name = file_name.to_string();
                    current_file_lines = vec![fixed_line];
                } else {
                    current_file_lines.push(fixed_line);
                }
            } else if is_in_details && line == "  ]" {
                is_in_details = false;
            }
        }
    }

    // Save any remaining file section
    if !current_file_name.is_empty() {
        file_sections.push(FileSection {
            file_name: current_file_name,
            lines: current_file_lines,
        });
    }

    // Sort and add remaining file sections
    if !file_sections.is_empty() {
        file_sections.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        for section in file_sections {
            command_lines.extend(drop_trailing_empty_lines(section.lines));
            command_lines.push(String::new());
        }
    }

    drop_trailing_empty_lines(command_lines).join("\n")
}

fn delete_linked_editing_info(s: String) -> String {
    let command_lines = Vec::<String>::new();
    let mut lines: Vec<_> = s.lines().map(str::to_string).collect();
    let mut in_linked_editing_info = false;
    for line in lines.iter_mut() {
        if is_linked_editing_info_header(line) {
            in_linked_editing_info = true;
            continue;
        }
        if is_file_name_header(line) {
            in_linked_editing_info = false;
            continue;
        }
        // drop the info since it's different--linked editing positions should be verified by file content/markers
        if !in_linked_editing_info {
            *line = String::new();
        }
    }
    drop_trailing_empty_lines(command_lines).join("\n")
}

fn compare_details(d1: &BaselineDetail, d2: &BaselineDetail) -> std::cmp::Ordering {
    let c = compare_positions(d1.pos, d2.pos);
    if c != 0 || d1.kind == DETAIL_KIND_MARKER && d2.kind == DETAIL_KIND_MARKER {
        return c.cmp(&0);
    }

    // /*MARKER*/[| some text |]
    if d1.kind == DETAIL_KIND_MARKER && d2.kind.is_start() {
        return std::cmp::Ordering::Less;
    }
    if d2.kind == DETAIL_KIND_MARKER && d1.kind.is_start() {
        return std::cmp::Ordering::Greater;
    }

    // [| some text |]/*MARKER*/
    if d1.kind == DETAIL_KIND_MARKER && d2.kind.is_end() {
        return std::cmp::Ordering::Greater;
    }
    if d2.kind == DETAIL_KIND_MARKER && d1.kind.is_end() {
        return std::cmp::Ordering::Less;
    }

    // [||] or <||>
    if d1.span_id.is_some() && d1.span_id == d2.span_id {
        return d1.kind.0.cmp(&d2.kind.0);
    }

    // ...|><|...
    if d1.kind.is_start() && d2.kind.is_end() {
        return std::cmp::Ordering::Greater;
    }
    if d1.kind.is_end() && d2.kind.is_start() {
        return std::cmp::Ordering::Less;
    }

    // <| ... [| ... |]|>
    if d1.kind.is_end() && d2.kind.is_end() {
        let c = compare_positions(d2.get_range().start, d1.get_range().start);
        if c != 0 {
            return c.cmp(&0);
        }
        return d1.kind.0.cmp(&d2.kind.0);
    }

    // <|[| ... |] ... |>
    if d1.kind.is_start() && d2.kind.is_start() {
        let c = compare_positions(d2.get_range().end, d2.get_range().end);
        if c != 0 {
            return c.cmp(&0);
        }
        return d1.kind.0.cmp(&d2.kind.0);
    }

    std::cmp::Ordering::Equal
}

fn strip_object_range(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find("{| ") {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find(" |}") {
            rest = &rest[start + end + 3..];
        } else {
            rest = &rest[start..];
            break;
        }
    }
    out.push_str(rest);
    out
}

fn file_header(line: &str) -> Option<&str> {
    line.strip_prefix("// === ")?
        .strip_suffix(" ===")
        .filter(|s| !s.contains(' '))
}

fn is_linked_editing_info_header(line: &str) -> bool {
    line.starts_with("=== ")
        && line.ends_with(" ===")
        && line[4..line.len() - 4].chars().all(|c| c.is_ascii_digit())
}

fn is_file_name_header(line: &str) -> bool {
    line.starts_with("=== ") && line.ends_with(" ===") && line.contains('.')
}

fn line_splitter(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split('\n')
        .map(str::to_string)
        .collect()
}

fn compute_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn compare_positions(a: lsproto::Position, b: lsproto::Position) -> i32 {
    a.line
        .cmp(&b.line)
        .then_with(|| a.character.cmp(&b.character)) as i32
}

fn file_name_to_document_uri(path: &str) -> lsproto::DocumentUri {
    ts_ls::file_name_to_document_uri(path)
}

fn is_lib_file(file_name: &str) -> bool {
    file_name.contains("/lib.") || file_name.starts_with("bundled:///libs/")
}

pub struct TestingT;

impl TestingT {
    pub fn helper(&mut self) {}

    pub fn name(&self) -> &str {
        ""
    }

    pub fn skip(&mut self, reason: &str) {
        panic!("skipped: {reason}");
    }
}

pub struct BaselineOptions {
    pub subfolder: String,
    pub is_submodule: bool,
    pub diff_fixup_old: Option<Box<dyn Fn(String) -> String>>,
    pub diff_fixup_new: Option<Box<dyn Fn(String) -> String>>,
}

impl Default for BaselineOptions {
    fn default() -> Self {
        Self {
            subfolder: String::new(),
            is_submodule: false,
            diff_fixup_old: None,
            diff_fixup_new: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MarkerOrRange {
    pub file_name: String,
    pub ls_pos: lsproto::Position,
}

impl MarkerOrRange {
    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }

    pub fn ls_pos(&self) -> lsproto::Position {
        self.ls_pos
    }
}

#[derive(Clone, Debug)]
pub struct Marker {
    pub file_name: String,
    pub position: usize,
    pub ls_position: lsproto::Position,
    pub name: Option<String>,
    pub data: BTreeMap<String, String>,
}

impl Marker {
    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }
}
