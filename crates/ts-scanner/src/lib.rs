#![forbid(unsafe_code)]

use std::sync::Arc;

use ts_collections::{FastHashMap as HashMap, FastHashMapExt};

pub mod regexp;
pub mod scanner;
pub mod unicodeproperties;
pub mod utilities;

pub use scanner::*;
pub use utilities::*;

use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostics;

pub fn get_ecmaline_of_position(source_file: impl ast::SourceFileLike, pos: usize) -> usize {
    get_ecma_line_of_position(source_file, pos)
}

pub fn get_ecmaline_starts(source_file: impl ast::SourceFileLike) -> Arc<[core::TextPos]> {
    get_ecma_line_starts(source_file)
}

#[derive(Clone)]
pub struct ScannerDiagnostic {
    pub message: diagnostics::Message,
    pub start: usize,
    pub length: usize,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) enum ScannerTokenValue {
    #[default]
    None,
    SourceRange {
        start: usize,
        end: usize,
    },
    Owned(String),
}

#[derive(Clone)]
pub struct Scanner {
    pub text: Arc<str>,
    pub end: usize,
    pub language_variant: core::LanguageVariant,
    pub script_target: core::ScriptTarget,
    pub on_error: Option<scanner::ErrorCallback>,
    pub skip_trivia: bool,
    pub pos: usize,
    pub full_start_pos: usize,
    pub token_start: usize,
    pub token: ast::Kind,
    pub(crate) token_value: ScannerTokenValue,
    pub token_flags: ast::TokenFlags,
    pub comment_directives: Vec<ast::CommentDirective>,
    pub contains_non_ascii: bool,
    pub number_cache: HashMap<String, String>,
    pub hex_number_cache: HashMap<String, String>,
    pub hex_digit_cache: HashMap<String, String>,
    pub diagnostics: Vec<ScannerDiagnostic>,
}

impl Scanner {
    pub fn new(text: impl Into<Arc<str>>, language_version: core::ScriptTarget) -> Self {
        let text = text.into();
        Self {
            end: text.len(),
            text,
            language_variant: core::LanguageVariant::Standard,
            script_target: language_version,
            on_error: None,
            skip_trivia: true,
            pos: 0,
            full_start_pos: 0,
            token_start: 0,
            token: ast::Kind::Unknown,
            token_value: ScannerTokenValue::None,
            token_flags: ast::TokenFlags::NONE,
            comment_directives: Vec::new(),
            contains_non_ascii: false,
            number_cache: HashMap::new(),
            hex_number_cache: HashMap::new(),
            hex_digit_cache: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn char(&self) -> char {
        if self.pos < self.end {
            return self.text.as_bytes()[self.pos] as char;
        }
        '\0'
    }

    pub fn char_at(&self, offset: usize) -> char {
        if let Some(pos) = self.pos.checked_add(offset) {
            if pos < self.end {
                return self.text.as_bytes()[pos] as char;
            }
        }
        '\0'
    }

    pub fn language_version(&self) -> core::ScriptTarget {
        if self.script_target == core::ScriptTarget::None {
            core::SCRIPT_TARGET_LATEST
        } else {
            self.script_target
        }
    }

    pub fn error_at(
        &mut self,
        message: &diagnostics::Message,
        start: usize,
        length: usize,
        args: Vec<String>,
    ) {
        self.diagnostics.push(ScannerDiagnostic {
            message: message.clone(),
            start,
            length,
            args,
        });
        if let Some(on_error) = self.on_error.as_ref() {
            let diagnostic = self.diagnostics.last().unwrap();
            let mut on_error = on_error.lock().unwrap_or_else(|err| err.into_inner());
            (on_error)(message, start, length, &diagnostic.args);
        }
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new(Arc::<str>::from(""), core::SCRIPT_TARGET_LATEST_STANDARD)
    }
}
