use std::cell::RefCell;
use std::rc::Rc;

use ts_ast as ast;
use ts_core as core;

// Externally opaque interface for printing text
pub trait EmitTextWriter {
    fn write(&mut self, s: &str);
    fn write_trailing_semicolon(&mut self, text: &str);
    fn write_comment(&mut self, text: &str);
    fn write_keyword(&mut self, text: &str);
    fn write_operator(&mut self, text: &str);
    fn write_punctuation(&mut self, text: &str);
    fn write_space(&mut self, text: &str);
    fn write_string_literal(&mut self, text: &str);
    fn write_parameter(&mut self, text: &str);
    fn write_property(&mut self, text: &str);
    fn write_symbol(&mut self, text: &str, symbol: Option<ast::SymbolHandle>);
    fn write_line(&mut self);
    fn write_line_force(&mut self, force: bool);
    fn increase_indent(&mut self);
    fn decrease_indent(&mut self);
    fn clear(&mut self);
    fn string(&self) -> String;
    fn raw_write(&mut self, s: &str);
    fn write_literal(&mut self, s: &str);
    fn get_text_pos(&self) -> i32;
    fn get_line(&self) -> i32;
    fn get_column(&self) -> core::UTF16Offset;
    fn get_indent(&self) -> i32;
    fn is_at_start_of_line(&self) -> bool;
    fn has_trailing_comment(&self) -> bool;
    fn has_trailing_whitespace(&self) -> bool;
}

pub type SharedEmitTextWriter = Rc<RefCell<Box<dyn EmitTextWriter>>>;

pub fn share_text_writer(writer: Box<dyn EmitTextWriter>) -> SharedEmitTextWriter {
    Rc::new(RefCell::new(writer))
}

pub struct SharedEmitTextWriterHandle {
    inner: SharedEmitTextWriter,
}

impl SharedEmitTextWriterHandle {
    pub fn new(inner: SharedEmitTextWriter) -> Self {
        Self { inner }
    }
}

impl EmitTextWriter for SharedEmitTextWriterHandle {
    fn write(&mut self, s: &str) {
        self.inner.borrow_mut().write(s)
    }

    fn write_trailing_semicolon(&mut self, text: &str) {
        self.inner.borrow_mut().write_trailing_semicolon(text)
    }

    fn write_comment(&mut self, text: &str) {
        self.inner.borrow_mut().write_comment(text)
    }

    fn write_keyword(&mut self, text: &str) {
        self.inner.borrow_mut().write_keyword(text)
    }

    fn write_operator(&mut self, text: &str) {
        self.inner.borrow_mut().write_operator(text)
    }

    fn write_punctuation(&mut self, text: &str) {
        self.inner.borrow_mut().write_punctuation(text)
    }

    fn write_space(&mut self, text: &str) {
        self.inner.borrow_mut().write_space(text)
    }

    fn write_string_literal(&mut self, text: &str) {
        self.inner.borrow_mut().write_string_literal(text)
    }

    fn write_parameter(&mut self, text: &str) {
        self.inner.borrow_mut().write_parameter(text)
    }

    fn write_property(&mut self, text: &str) {
        self.inner.borrow_mut().write_property(text)
    }

    fn write_symbol(&mut self, text: &str, symbol: Option<ast::SymbolHandle>) {
        self.inner.borrow_mut().write_symbol(text, symbol)
    }

    fn write_line(&mut self) {
        self.inner.borrow_mut().write_line()
    }

    fn write_line_force(&mut self, force: bool) {
        self.inner.borrow_mut().write_line_force(force)
    }

    fn increase_indent(&mut self) {
        self.inner.borrow_mut().increase_indent()
    }

    fn decrease_indent(&mut self) {
        self.inner.borrow_mut().decrease_indent()
    }

    fn clear(&mut self) {
        self.inner.borrow_mut().clear()
    }

    fn string(&self) -> String {
        self.inner.borrow().string()
    }

    fn raw_write(&mut self, s: &str) {
        self.inner.borrow_mut().raw_write(s)
    }

    fn write_literal(&mut self, s: &str) {
        self.inner.borrow_mut().write_literal(s)
    }

    fn get_text_pos(&self) -> i32 {
        self.inner.borrow().get_text_pos()
    }

    fn get_line(&self) -> i32 {
        self.inner.borrow().get_line()
    }

    fn get_column(&self) -> core::UTF16Offset {
        self.inner.borrow().get_column()
    }

    fn get_indent(&self) -> i32 {
        self.inner.borrow().get_indent()
    }

    fn is_at_start_of_line(&self) -> bool {
        self.inner.borrow().is_at_start_of_line()
    }

    fn has_trailing_comment(&self) -> bool {
        self.inner.borrow().has_trailing_comment()
    }

    fn has_trailing_whitespace(&self) -> bool {
        self.inner.borrow().has_trailing_whitespace()
    }
}

impl<T: EmitTextWriter + ?Sized> EmitTextWriter for Box<T> {
    fn write(&mut self, s: &str) {
        self.as_mut().write(s)
    }

    fn write_trailing_semicolon(&mut self, text: &str) {
        self.as_mut().write_trailing_semicolon(text)
    }

    fn write_comment(&mut self, text: &str) {
        self.as_mut().write_comment(text)
    }

    fn write_keyword(&mut self, text: &str) {
        self.as_mut().write_keyword(text)
    }

    fn write_operator(&mut self, text: &str) {
        self.as_mut().write_operator(text)
    }

    fn write_punctuation(&mut self, text: &str) {
        self.as_mut().write_punctuation(text)
    }

    fn write_space(&mut self, text: &str) {
        self.as_mut().write_space(text)
    }

    fn write_string_literal(&mut self, text: &str) {
        self.as_mut().write_string_literal(text)
    }

    fn write_parameter(&mut self, text: &str) {
        self.as_mut().write_parameter(text)
    }

    fn write_property(&mut self, text: &str) {
        self.as_mut().write_property(text)
    }

    fn write_symbol(&mut self, text: &str, symbol: Option<ast::SymbolHandle>) {
        self.as_mut().write_symbol(text, symbol)
    }

    fn write_line(&mut self) {
        self.as_mut().write_line()
    }

    fn write_line_force(&mut self, force: bool) {
        self.as_mut().write_line_force(force)
    }

    fn increase_indent(&mut self) {
        self.as_mut().increase_indent()
    }

    fn decrease_indent(&mut self) {
        self.as_mut().decrease_indent()
    }

    fn clear(&mut self) {
        self.as_mut().clear()
    }

    fn string(&self) -> String {
        self.as_ref().string()
    }

    fn raw_write(&mut self, s: &str) {
        self.as_mut().raw_write(s)
    }

    fn write_literal(&mut self, s: &str) {
        self.as_mut().write_literal(s)
    }

    fn get_text_pos(&self) -> i32 {
        self.as_ref().get_text_pos()
    }

    fn get_line(&self) -> i32 {
        self.as_ref().get_line()
    }

    fn get_column(&self) -> core::UTF16Offset {
        self.as_ref().get_column()
    }

    fn get_indent(&self) -> i32 {
        self.as_ref().get_indent()
    }

    fn is_at_start_of_line(&self) -> bool {
        self.as_ref().is_at_start_of_line()
    }

    fn has_trailing_comment(&self) -> bool {
        self.as_ref().has_trailing_comment()
    }

    fn has_trailing_whitespace(&self) -> bool {
        self.as_ref().has_trailing_whitespace()
    }
}
