use super::TestFile;
use ts_core as core;
use ts_scanner as scanner;
use ts_sourcemap as sourcemap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WriterAggregator {
    pub text: String,
}

impl WriterAggregator {
    pub fn write_stringf(&mut self, s: impl AsRef<str>) {
        self.text.push_str(s.as_ref());
    }

    pub fn write_line(&mut self, s: &str) {
        self.text.push_str(s);
        self.text.push_str("\r\n");
    }

    pub fn write_linef(&mut self, s: impl AsRef<str>) {
        self.write_line(s.as_ref());
    }
}

impl std::fmt::Display for WriterAggregator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Mapping {
    pub generated_line: i32,
    pub generated_character: i32,
    pub source_index: i32,
    pub source_line: i32,
    pub source_character: i32,
    pub name_index: i32,
}

impl Mapping {
    pub fn is_source_mapping(&self) -> bool {
        self.source_index >= 0 && self.source_line >= 0 && self.source_character >= 0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RawSourceMap {
    pub file: String,
    pub source_root: String,
    pub sources: Vec<String>,
    pub sources_content: Vec<Option<String>>,
    pub names: Vec<String>,
    pub mappings: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceMapSpanWithDecodeErrors {
    pub source_map_span: Mapping,
    pub decode_errors: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DecodedMapping {
    pub source_map_span: Mapping,
    pub error: Option<String>,
}

pub struct SourceMapDecoder {
    pub source_map_mappings: String,
    pub mappings: sourcemap::MappingsDecoder,
}

pub fn new_source_map_decoder(source_map: &RawSourceMap) -> SourceMapDecoder {
    SourceMapDecoder {
        source_map_mappings: source_map.mappings.clone(),
        mappings: sourcemap::decode_mappings(source_map.mappings.clone()),
    }
}

impl SourceMapDecoder {
    pub fn decode_next_encoded_source_map_span(&mut self) -> DecodedMapping {
        if let Some(value) = self.mappings.next() {
            return DecodedMapping {
                source_map_span: mapping_from_sourcemap(&value),
                error: None,
            };
        }

        DecodedMapping {
            source_map_span: mapping_from_sourcemap(&self.mappings.state()),
            error: Some(
                self.mappings
                    .error()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "No encoded entry found".to_string()),
            ),
        }
    }

    pub fn has_completed_decoding(&self) -> bool {
        self.mappings.pos() == self.source_map_mappings.len()
    }

    pub fn get_remaining_decode_string(&self) -> &str {
        &self.source_map_mappings[self.mappings.pos()..]
    }
}

fn mapping_from_sourcemap(mapping: &sourcemap::Mapping) -> Mapping {
    Mapping {
        generated_line: mapping.generated_line,
        generated_character: mapping.generated_character,
        source_index: mapping.source_index,
        source_line: mapping.source_line,
        source_character: mapping.source_character,
        name_index: mapping.name_index,
    }
}

pub struct SourceMapSpanWriter {
    pub source_map_recorder: WriterAggregator,
    pub source_map_sources: Vec<String>,
    pub source_map_names: Vec<String>,
    pub js_file: TestFile,
    pub js_line_map: Vec<core::TextPos>,
    pub ts_code: String,
    pub ts_line_map: Vec<core::TextPos>,
    pub spans_on_single_line: Vec<SourceMapSpanWithDecodeErrors>,
    pub prev_written_source_pos: usize,
    pub next_js_line_to_write: usize,
    pub span_marker_continues: bool,
    pub source_map_decoder: SourceMapDecoder,
}

pub fn new_source_map_span_writer(
    mut source_map_recorder: WriterAggregator,
    source_map: &RawSourceMap,
    js_file: TestFile,
) -> SourceMapSpanWriter {
    source_map_recorder
        .write_line("===================================================================");
    source_map_recorder.write_linef(format!("JsFile: {}", source_map.file));
    source_map_recorder.write_linef(format!(
        "mapUrl: {}",
        try_get_source_mapping_url(&js_file.content)
    ));
    source_map_recorder.write_linef(format!("sourceRoot: {}", source_map.source_root));
    source_map_recorder.write_linef(format!("sources: {}", source_map.sources.join(",")));
    if !source_map.sources_content.is_empty() {
        let content = serde_json::to_string(&source_map.sources_content)
            .unwrap_or_else(|err| panic!("{err}"));
        source_map_recorder.write_linef(format!("sourcesContent: {content}"));
    }
    source_map_recorder
        .write_line("===================================================================");
    SourceMapSpanWriter {
        source_map_recorder,
        source_map_sources: source_map.sources.clone(),
        source_map_names: source_map.names.clone(),
        js_line_map: compute_line_starts(&js_file.content),
        js_file,
        ts_code: String::new(),
        ts_line_map: Vec::new(),
        spans_on_single_line: Vec::new(),
        prev_written_source_pos: 0,
        next_js_line_to_write: 0,
        span_marker_continues: false,
        source_map_decoder: new_source_map_decoder(source_map),
    }
}

impl SourceMapSpanWriter {
    pub fn get_source_map_span_string(
        &self,
        map_entry: &Mapping,
        get_absent_name_index: bool,
    ) -> String {
        let mut map_string = format!(
            "Emitted({}, {})",
            map_entry.generated_line + 1,
            map_entry.generated_character + 1
        );
        if map_entry.is_source_mapping() {
            map_string.push_str(&format!(
                " Source({}, {}) + SourceIndex({})",
                map_entry.source_line + 1,
                map_entry.source_character + 1,
                map_entry.source_index
            ));
            if map_entry.name_index >= 0
                && (map_entry.name_index as usize) < self.source_map_names.len()
            {
                map_string.push_str(&format!(
                    " name ({})",
                    self.source_map_names[map_entry.name_index as usize]
                ));
            } else if map_entry.name_index != -1 || get_absent_name_index {
                map_string.push_str(&format!(" nameIndex ({})", map_entry.name_index));
            }
        }
        map_string
    }

    pub fn record_source_map_span(&mut self, source_map_span: Mapping) {
        let decode_result = self
            .source_map_decoder
            .decode_next_encoded_source_map_span();
        let mut decode_errors = Vec::new();
        if decode_result.error.is_some() || decode_result.source_map_span != source_map_span {
            if let Some(error) = decode_result.error {
                decode_errors.push(format!(
                    "!!^^ !!^^ There was decoding error in the sourcemap at this location: {error}"
                ));
            } else {
                decode_errors.push(
                    "!!^^ !!^^ The decoded span from sourcemap's mapping entry does not match what was encoded for this span:".to_string(),
                );
            }
            decode_errors.push(format!(
                "!!^^ !!^^ Decoded span from sourcemap's mappings entry: {} Span encoded by the emitter:{}",
                self.get_source_map_span_string(&decode_result.source_map_span, true),
                self.get_source_map_span_string(&source_map_span, true),
            ));
        }
        if !self.spans_on_single_line.is_empty()
            && self.spans_on_single_line[0].source_map_span.generated_line
                != source_map_span.generated_line
        {
            self.write_recorded_spans();
            self.spans_on_single_line.clear();
        }
        self.spans_on_single_line
            .push(SourceMapSpanWithDecodeErrors {
                source_map_span,
                decode_errors,
            });
    }

    pub fn record_new_source_file_span(
        &mut self,
        source_map_span: Mapping,
        new_source_file_code: String,
    ) {
        let mut continues_line = false;
        if !self.spans_on_single_line.is_empty()
            && self.spans_on_single_line[0]
                .source_map_span
                .generated_character
                == source_map_span.generated_line
        {
            self.write_recorded_spans();
            self.spans_on_single_line.clear();
            self.next_js_line_to_write -= 1; // walk back one line to reprint the line
            continues_line = true;
        }

        self.record_source_map_span(source_map_span.clone());
        if self.spans_on_single_line.len() != 1 {
            panic!("expected a single span");
        }

        self.source_map_recorder
            .write_line("-------------------------------------------------------------------");
        if continues_line {
            self.source_map_recorder.write_linef(format!(
                "emittedFile:{} ({}, {})",
                self.js_file.unit_name,
                source_map_span.generated_line + 1,
                source_map_span.generated_character + 1
            ));
        } else {
            self.source_map_recorder
                .write_linef(format!("emittedFile:{}", self.js_file.unit_name));
        }
        if let Some(source) = self
            .source_map_sources
            .get(source_map_span.source_index as usize)
        {
            self.source_map_recorder
                .write_linef(format!("sourceFile:{source}"));
        }
        self.source_map_recorder
            .write_line("-------------------------------------------------------------------");
        self.ts_line_map = compute_line_starts(&new_source_file_code);
        self.ts_code = new_source_file_code;
        self.prev_written_source_pos = 0;
    }

    pub fn close(&mut self) {
        self.write_recorded_spans();
        if !self.source_map_decoder.has_completed_decoding() {
            self.source_map_recorder.write_line(
                "!!!! **** There are more source map entries in the sourceMap's mapping than what was encoded",
            );
            self.source_map_recorder.write_linef(format!(
                "!!!! **** Remaining decoded string: {}",
                self.source_map_decoder.get_remaining_decode_string()
            ));
        }
        self.write_js_file_lines(self.js_line_map.len());
    }

    pub fn get_text_of_line(&self, line: usize, line_map: &[core::TextPos], code: &str) -> String {
        let start_pos = line_map[line].max(0) as usize;
        let end_pos = if line + 1 < line_map.len() {
            line_map[line + 1].max(0) as usize
        } else {
            code.len()
        };
        let text = &code[start_pos..end_pos];
        if line == 0 {
            text.trim_start_matches('\u{feff}').to_string()
        } else {
            text.to_string()
        }
    }

    pub fn write_js_file_lines(&mut self, end_js_line: usize) {
        while self.next_js_line_to_write < end_js_line {
            let text = self.get_text_of_line(
                self.next_js_line_to_write,
                &self.js_line_map,
                &self.js_file.content,
            );
            self.source_map_recorder.write_stringf(format!(">>>{text}"));
            self.next_js_line_to_write += 1;
        }
    }

    pub fn write_recorded_spans(&mut self) {
        let mut writer = RecordedSpanWriter::new(self);
        writer.write_recorded_spans();
    }
}

pub struct RecordedSpanWriter<'a> {
    pub marker_ids: Vec<String>,
    pub prev_emitted_col: i32,
    pub writer: &'a mut SourceMapSpanWriter,
}

impl<'a> RecordedSpanWriter<'a> {
    pub fn new(writer: &'a mut SourceMapSpanWriter) -> Self {
        Self {
            marker_ids: Vec::new(),
            prev_emitted_col: 0,
            writer,
        }
    }

    pub fn get_marker_id(&self, marker_index: usize) -> String {
        if self.writer.span_marker_continues {
            assert_eq!(marker_index, 0);
            "1->".to_string()
        } else {
            let marker_id = (marker_index + 1).to_string();
            if marker_id.len() < 2 {
                format!("{marker_id} >")
            } else {
                format!("{marker_id}>")
            }
        }
    }

    pub fn write_source_map_indent(&mut self, indent_length: i32, indent_prefix: &str) {
        self.writer.source_map_recorder.write_stringf(indent_prefix);
        for _ in 0..indent_length {
            self.writer.source_map_recorder.write_stringf(" ");
        }
    }

    pub fn iterate_spans<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self, usize),
    {
        self.prev_emitted_col = 0;
        for index in 0..self.writer.spans_on_single_line.len() {
            f(self, index);
            self.prev_emitted_col = self.writer.spans_on_single_line[index]
                .source_map_span
                .generated_character;
        }
    }

    pub fn write_source_map_marker(&mut self, index: usize) {
        let end_column = self.writer.spans_on_single_line[index]
            .source_map_span
            .generated_character;
        self.write_source_map_marker_ex(index, end_column, false);
    }

    pub fn write_source_map_marker_ex(
        &mut self,
        marker_index: usize,
        end_column: i32,
        end_continues: bool,
    ) {
        let marker_id = self.get_marker_id(marker_index);
        self.marker_ids.push(marker_id.clone());
        self.write_source_map_indent(self.prev_emitted_col, &marker_id);
        for _ in self.prev_emitted_col..end_column {
            self.writer.source_map_recorder.write_stringf("^");
        }
        if end_continues {
            self.writer.source_map_recorder.write_stringf("->");
        }
        self.writer.source_map_recorder.write_line("");
        self.writer.span_marker_continues = end_continues;
    }

    pub fn write_recorded_spans(&mut self) {
        if self.writer.spans_on_single_line.is_empty() {
            return;
        }
        let current_js_line = self.writer.spans_on_single_line[0]
            .source_map_span
            .generated_line as usize;
        self.writer.write_js_file_lines(current_js_line + 1);

        self.iterate_spans(|writer, index| writer.write_source_map_marker(index));

        let js_file_text = if current_js_line + 1 < self.writer.js_line_map.len() {
            self.writer.get_text_of_line(
                current_js_line + 1,
                &self.writer.js_line_map,
                &self.writer.js_file.content,
            )
        } else {
            String::new()
        };
        if self.prev_emitted_col < js_file_text.len() as i32 - 1 {
            self.write_source_map_marker_ex(
                self.writer.spans_on_single_line.len(),
                js_file_text.len() as i32 - 1,
                true,
            );
        }

        self.iterate_spans(|writer, index| writer.write_source_map_source_text(index));

        self.iterate_spans(|writer, index| writer.write_span_details(index));

        self.writer.source_map_recorder.write_line("---");
    }

    fn write_span_details(&mut self, index: usize) {
        let marker = self.marker_ids.get(index).cloned().unwrap_or_default();
        let span = &self.writer.spans_on_single_line[index];
        self.writer.source_map_recorder.write_linef(format!(
            "{}{}",
            marker,
            self.writer
                .get_source_map_span_string(&span.source_map_span, false)
        ));
    }

    fn write_source_map_source_text(&mut self, index: usize) {
        let span = self.writer.spans_on_single_line[index]
            .source_map_span
            .clone();
        let source_pos = scanner::compute_position_of_line_and_utf16_character(
            &self.writer.ts_line_map,
            span.source_line.max(0) as usize,
            span.source_character,
            &self.writer.ts_code,
            true,
        );
        let source_text = if self.writer.prev_written_source_pos < source_pos {
            self.writer.ts_code[self.writer.prev_written_source_pos..source_pos].to_string()
        } else {
            String::new()
        };

        let marker = self.marker_ids.get(index).cloned().unwrap_or_default();
        let decode_errors = self.writer.spans_on_single_line[index]
            .decode_errors
            .clone();
        for decode_error in decode_errors {
            self.write_source_map_indent(self.prev_emitted_col, &marker);
            self.writer.source_map_recorder.write_linef(decode_error);
        }

        let source_line_map = compute_line_starts(&source_text);
        for line_index in 0..source_line_map.len() {
            if line_index == 0 {
                self.write_source_map_indent(self.prev_emitted_col, &marker);
            } else {
                self.write_source_map_indent(self.prev_emitted_col, "  >");
            }
            let text = self
                .writer
                .get_text_of_line(line_index, &source_line_map, &source_text);
            self.writer.source_map_recorder.write_stringf(text);
            if line_index == source_line_map.len() - 1 {
                self.writer.source_map_recorder.write_line("");
            }
        }
        self.writer.prev_written_source_pos = source_pos;
    }
}

fn compute_line_starts(code: &str) -> Vec<core::TextPos> {
    core::compute_ecma_line_starts(code)
}

fn try_get_source_mapping_url(text: &str) -> String {
    text.lines()
        .rev()
        .find_map(|line| {
            line.trim_start()
                .strip_prefix("//# sourceMappingURL=")
                .map(|url| url.trim().to_string())
        })
        .unwrap_or_default()
}
