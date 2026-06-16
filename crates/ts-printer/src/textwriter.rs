use std::cell::RefCell;
use std::rc::Rc;

use crate::{EmitTextWriter, SharedEmitTextWriter, UTF16Offset};
use ts_stringutil::{is_line_break, is_white_space_like};

pub struct TextWriter {
    new_line: String,
    indent_size: i32,
    builder: String,
    last_written: String,
    indent: i32,
    line_start: bool,
    line_count: i32,
    line_pos: usize,
    has_trailing_comment_state: bool,
}

impl TextWriter {
    pub(crate) fn new_with_indent_size(new_line: String, indent_size: i32) -> Self {
        let mut writer = Self {
            new_line,
            indent_size,
            builder: String::new(),
            last_written: String::new(),
            indent: 0,
            line_start: true,
            line_count: 0,
            line_pos: 0,
            has_trailing_comment_state: false,
        };
        writer.clear();
        writer
    }

    pub fn clear(&mut self) {
        let new_line = self.new_line.clone();
        let indent_size = self.indent_size;
        *self = Self {
            new_line,
            indent_size,
            builder: String::new(),
            last_written: String::new(),
            indent: 0,
            line_start: true,
            line_count: 0,
            line_pos: 0,
            has_trailing_comment_state: false,
        };
    }

    pub fn grow(&mut self, n: usize) {
        self.builder.reserve(n);
    }

    fn update_line_count_and_pos_for(&mut self, s: &str) {
        let mut count = 0;
        let mut last_line_start = 0usize;

        for line_start in compute_ecma_line_starts_seq(s) {
            count += 1;
            last_line_start = line_start;
        }

        if count > 1 {
            self.line_count += count - 1;
            let cur_len = self.builder.len();
            self.line_pos = cur_len - s.len() + last_line_start;
            self.line_start = self.line_pos == cur_len;
            return;
        }
        self.line_start = false;
    }

    fn write_text(&mut self, s: &str) {
        if !s.is_empty() {
            if self.line_start {
                self.builder
                    .push_str(&get_indent_string(self.indent, self.indent_size));
                self.line_start = false;
            }
            self.builder.push_str(s);
            self.last_written.clear();
            self.last_written.push_str(s);
            self.update_line_count_and_pos_for(s);
        }
    }

    fn write_line_raw(&mut self) {
        self.builder.push_str(&self.new_line);
        self.last_written.clear();
        self.last_written.push_str(&self.new_line);
        self.line_count += 1;
        self.line_pos = self.builder.len();
        self.line_start = true;
        self.has_trailing_comment_state = false;
    }
}

impl EmitTextWriter for TextWriter {
    fn write(&mut self, s: &str) {
        if !s.is_empty() {
            self.has_trailing_comment_state = false;
        }
        self.write_text(s);
    }

    fn write_trailing_semicolon(&mut self, text: &str) {
        self.write(text);
    }

    fn write_comment(&mut self, text: &str) {
        if !text.is_empty() {
            self.has_trailing_comment_state = true;
        }
        self.write_text(text);
    }

    fn write_keyword(&mut self, text: &str) {
        self.write(text);
    }

    fn write_operator(&mut self, text: &str) {
        self.write(text);
    }

    fn write_punctuation(&mut self, text: &str) {
        self.write(text);
    }

    fn write_space(&mut self, text: &str) {
        self.write(text);
    }

    fn write_string_literal(&mut self, text: &str) {
        self.write(text);
    }

    fn write_parameter(&mut self, text: &str) {
        self.write(text);
    }

    fn write_property(&mut self, text: &str) {
        self.write(text);
    }

    fn write_symbol(&mut self, text: &str, _symbol: Option<ts_ast::SymbolHandle>) {
        self.write(text);
    }

    fn write_line(&mut self) {
        if !self.line_start {
            self.write_line_raw();
        }
    }

    fn write_line_force(&mut self, force: bool) {
        if !self.line_start || force {
            self.write_line_raw();
        }
    }

    fn increase_indent(&mut self) {
        self.indent += 1;
    }

    fn decrease_indent(&mut self) {
        self.indent -= 1;
    }

    fn clear(&mut self) {
        TextWriter::clear(self);
    }

    fn string(&self) -> String {
        self.builder.clone()
    }

    fn raw_write(&mut self, s: &str) {
        if !s.is_empty() {
            self.builder.push_str(s);
            self.last_written.clear();
            self.last_written.push_str(s);
            self.has_trailing_comment_state = false;
        }
        self.update_line_count_and_pos_for(s);
    }

    fn write_literal(&mut self, s: &str) {
        self.write(s);
    }

    fn get_text_pos(&self) -> i32 {
        self.builder.len() as i32
    }

    fn get_line(&self) -> i32 {
        self.line_count
    }

    fn get_column(&self) -> UTF16Offset {
        if self.line_start {
            return (self.indent * self.indent_size) as UTF16Offset;
        }
        utf16_len(&self.builder[self.line_pos..])
    }

    fn get_indent(&self) -> i32 {
        self.indent
    }

    fn is_at_start_of_line(&self) -> bool {
        self.line_start
    }

    fn has_trailing_comment(&self) -> bool {
        self.has_trailing_comment_state
    }

    fn has_trailing_whitespace(&self) -> bool {
        if self.builder.is_empty() {
            return false;
        }
        self.last_written
            .chars()
            .next_back()
            .is_some_and(is_white_space_like)
    }
}

pub const DEFAULT_INDENT_SIZE: i32 = 4;

// GetDefaultIndentSize returns the default indent size (4 spaces) used when no specific indent size is configured.
pub const fn get_default_indent_size() -> i32 {
    DEFAULT_INDENT_SIZE
}

pub fn get_indent_string(indent: i32, indent_size: i32) -> String {
    if indent == 0 {
        return String::new();
    }
    // TODO: This is cached in tsc - should it be cached here?
    " ".repeat((indent * indent_size) as usize)
}

pub fn new_text_writer(new_line: String, indent_size: i32) -> Box<dyn EmitTextWriter> {
    let indent_size = if indent_size <= 0 {
        DEFAULT_INDENT_SIZE
    } else {
        indent_size
    };
    let mut w = TextWriter {
        new_line,
        indent_size,
        builder: String::new(),
        last_written: String::new(),
        indent: 0,
        line_start: false,
        line_count: 0,
        line_pos: 0,
        has_trailing_comment_state: false,
    };
    w.clear();
    Box::new(w)
}

pub fn new_shared_text_writer(new_line: String, indent_size: i32) -> SharedEmitTextWriter {
    Rc::new(RefCell::new(new_text_writer(new_line, indent_size)))
}

fn compute_ecma_line_starts_seq(text: &str) -> Vec<usize> {
    let mut line_starts = Vec::with_capacity(text.bytes().filter(|b| *b == b'\n').count() + 1);
    let bytes = text.as_bytes();
    let text_len = bytes.len();
    let mut pos = 0usize;
    let mut line_start = 0usize;
    while pos < text_len {
        let b = bytes[pos];
        if b < 0x80 {
            pos += 1;
            match b {
                b'\r' => {
                    if pos < text_len && bytes[pos] == b'\n' {
                        pos += 1;
                    }
                    line_starts.push(line_start);
                    line_start = pos;
                }
                b'\n' => {
                    line_starts.push(line_start);
                    line_start = pos;
                }
                _ => {}
            }
        } else {
            let mut chars = text[pos..].chars();
            let Some(ch) = chars.next() else {
                break;
            };
            pos += ch.len_utf8();
            if is_line_break(ch) {
                line_starts.push(line_start);
                line_start = pos;
            }
        }
    }
    line_starts.push(line_start);
    line_starts
}

fn utf16_len(s: &str) -> UTF16Offset {
    for (i, b) in s.bytes().enumerate() {
        if b >= 0x80 {
            let mut n = i as UTF16Offset;
            for ch in s[i..].chars() {
                n += ch.len_utf16() as UTF16Offset;
            }
            return n;
        }
    }
    s.len() as UTF16Offset
}
