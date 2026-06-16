use std::cell::RefCell;

use crate::{EmitTextWriter, UTF16Offset};
use ts_stringutil::is_white_space_like;

#[derive(Default)]
pub struct SingleLineStringWriter {
    builder: String,
    last_written: String,
}

impl SingleLineStringWriter {
    pub fn clear(&mut self) {
        self.last_written.clear();
        self.builder.clear();
    }

    fn write_raw_text(&mut self, text: &str) {
        self.last_written.clear();
        self.last_written.push_str(text);
        self.builder.push_str(text);
    }
}

impl EmitTextWriter for SingleLineStringWriter {
    fn write(&mut self, s: &str) {
        self.write_raw_text(s);
    }

    fn write_trailing_semicolon(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_comment(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_keyword(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_operator(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_punctuation(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_space(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_string_literal(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_parameter(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_property(&mut self, text: &str) {
        self.write_raw_text(text);
    }

    fn write_symbol(&mut self, text: &str, _symbol: Option<ts_ast::SymbolHandle>) {
        self.write_raw_text(text);
    }

    fn write_line(&mut self) {
        self.write_raw_text(" ");
    }

    fn write_line_force(&mut self, _force: bool) {
        self.write_raw_text(" ");
    }

    fn increase_indent(&mut self) {
        // Do Nothing
    }

    fn decrease_indent(&mut self) {
        // Do Nothing
    }

    fn clear(&mut self) {
        SingleLineStringWriter::clear(self);
    }

    fn string(&self) -> String {
        self.builder.clone()
    }

    fn raw_write(&mut self, s: &str) {
        self.write_raw_text(s);
    }

    fn write_literal(&mut self, s: &str) {
        self.write_raw_text(s);
    }

    fn get_text_pos(&self) -> i32 {
        self.builder.len() as i32
    }

    fn get_line(&self) -> i32 {
        0
    }

    fn get_column(&self) -> UTF16Offset {
        0
    }

    fn get_indent(&self) -> i32 {
        0
    }

    fn is_at_start_of_line(&self) -> bool {
        false
    }

    fn has_trailing_comment(&self) -> bool {
        false
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

thread_local! {
    static SINGLE_LINE_STRING_WRITER_POOL: RefCell<Vec<SingleLineStringWriter>> =
        const { RefCell::new(Vec::new()) };
}

struct PooledSingleLineStringWriter {
    writer: Option<SingleLineStringWriter>,
}

impl PooledSingleLineStringWriter {
    fn writer(&self) -> &SingleLineStringWriter {
        self.writer
            .as_ref()
            .expect("single-line writer already returned to pool")
    }

    fn writer_mut(&mut self) -> &mut SingleLineStringWriter {
        self.writer
            .as_mut()
            .expect("single-line writer already returned to pool")
    }
}

impl Drop for PooledSingleLineStringWriter {
    fn drop(&mut self) {
        if let Some(mut writer) = self.writer.take() {
            writer.clear();
            SINGLE_LINE_STRING_WRITER_POOL.with(|pool| pool.borrow_mut().push(writer));
        }
    }
}

impl EmitTextWriter for PooledSingleLineStringWriter {
    fn write(&mut self, s: &str) {
        self.writer_mut().write(s);
    }

    fn write_trailing_semicolon(&mut self, text: &str) {
        self.writer_mut().write_trailing_semicolon(text);
    }

    fn write_comment(&mut self, text: &str) {
        self.writer_mut().write_comment(text);
    }

    fn write_keyword(&mut self, text: &str) {
        self.writer_mut().write_keyword(text);
    }

    fn write_operator(&mut self, text: &str) {
        self.writer_mut().write_operator(text);
    }

    fn write_punctuation(&mut self, text: &str) {
        self.writer_mut().write_punctuation(text);
    }

    fn write_space(&mut self, text: &str) {
        self.writer_mut().write_space(text);
    }

    fn write_string_literal(&mut self, text: &str) {
        self.writer_mut().write_string_literal(text);
    }

    fn write_parameter(&mut self, text: &str) {
        self.writer_mut().write_parameter(text);
    }

    fn write_property(&mut self, text: &str) {
        self.writer_mut().write_property(text);
    }

    fn write_symbol(&mut self, text: &str, symbol: Option<ts_ast::SymbolHandle>) {
        self.writer_mut().write_symbol(text, symbol);
    }

    fn write_line(&mut self) {
        self.writer_mut().write_line();
    }

    fn write_line_force(&mut self, force: bool) {
        self.writer_mut().write_line_force(force);
    }

    fn increase_indent(&mut self) {
        self.writer_mut().increase_indent();
    }

    fn decrease_indent(&mut self) {
        self.writer_mut().decrease_indent();
    }

    fn clear(&mut self) {
        self.writer_mut().clear();
    }

    fn string(&self) -> String {
        self.writer().string()
    }

    fn raw_write(&mut self, s: &str) {
        self.writer_mut().raw_write(s);
    }

    fn write_literal(&mut self, s: &str) {
        self.writer_mut().write_literal(s);
    }

    fn get_text_pos(&self) -> i32 {
        self.writer().get_text_pos()
    }

    fn get_line(&self) -> i32 {
        self.writer().get_line()
    }

    fn get_column(&self) -> UTF16Offset {
        self.writer().get_column()
    }

    fn get_indent(&self) -> i32 {
        self.writer().get_indent()
    }

    fn is_at_start_of_line(&self) -> bool {
        self.writer().is_at_start_of_line()
    }

    fn has_trailing_comment(&self) -> bool {
        self.writer().has_trailing_comment()
    }

    fn has_trailing_whitespace(&self) -> bool {
        self.writer().has_trailing_whitespace()
    }
}

pub fn get_single_line_string_writer() -> Box<dyn EmitTextWriter> {
    let mut writer = SINGLE_LINE_STRING_WRITER_POOL
        .with(|pool| pool.borrow_mut().pop())
        .unwrap_or_default();
    writer.clear();
    Box::new(PooledSingleLineStringWriter {
        writer: Some(writer),
    })
}
