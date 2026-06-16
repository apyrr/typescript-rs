use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt,
    sync::{LazyLock, Mutex},
};

use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;

use crate::ast::{DiagnosticFile, SourceFile};

// RepopulateDiagnosticKind indicates the kind of repopulation for a diagnostic chain entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RepopulateDiagnosticKind {
    ModeMismatch = 1,
    ModuleNotFound = 2,
}

impl serde::Serialize for RepopulateDiagnosticKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(*self as i32)
    }
}

impl<'de> serde::Deserialize<'de> for RepopulateDiagnosticKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RepopulateDiagnosticKindVisitor;

        impl serde::de::Visitor<'_> for RepopulateDiagnosticKindVisitor {
            type Value = RepopulateDiagnosticKind;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a repopulate diagnostic kind number")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    1 => Ok(RepopulateDiagnosticKind::ModeMismatch),
                    2 => Ok(RepopulateDiagnosticKind::ModuleNotFound),
                    _ => Err(E::custom(format!(
                        "unknown repopulate diagnostic kind: {value}"
                    ))),
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_i64(value as i64)
            }
        }

        deserializer.deserialize_i64(RepopulateDiagnosticKindVisitor)
    }
}

#[allow(non_snake_case, non_upper_case_globals)]
pub mod RepopulateKind {
    use super::RepopulateDiagnosticKind;

    pub const ModeMismatch: RepopulateDiagnosticKind = RepopulateDiagnosticKind::ModeMismatch;
    pub const ModuleNotFound: RepopulateDiagnosticKind = RepopulateDiagnosticKind::ModuleNotFound;
}

impl Default for RepopulateDiagnosticKind {
    fn default() -> Self {
        Self::ModeMismatch
    }
}

// RepopulateDiagnosticInfo stores information needed to recompute a diagnostic chain entry
// during incremental builds when the program state may have changed.
#[derive(Debug, Clone)]
pub struct RepopulateDiagnosticInfo {
    pub kind: RepopulateDiagnosticKind,
    pub module_reference: String,
    pub mode: core::ResolutionMode,
    pub package_name: String,
}

// Diagnostic

#[derive(Clone)]
pub struct Diagnostic {
    file: Option<DiagnosticFile>,
    loc: core::TextRange,
    code: i32,
    category: diagnostics::Category,
    // Original message; may be nil.
    message: Option<diagnostics::Message>,
    message_key: diagnostics::Key,
    message_args: Vec<String>,
    message_chain: Vec<Diagnostic>,
    related_information: Vec<Diagnostic>,
    reports_unnecessary: bool,
    reports_deprecated: bool,
    skipped_on_no_emit: bool,
    repopulate_info: Option<RepopulateDiagnosticInfo>,
    canonical_code: Option<i32>,
    canonical_message_args: Option<Vec<String>>,
}

impl Diagnostic {
    pub fn file(&self) -> Option<&DiagnosticFile> {
        self.file.as_ref()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.file.as_ref().map(|file| file.file_name())
    }

    pub fn pos(&self) -> i32 {
        self.loc.pos()
    }

    pub fn end(&self) -> i32 {
        self.loc.end()
    }

    pub fn len(&self) -> i32 {
        self.loc.len()
    }

    pub fn is_empty(&self) -> bool {
        self.loc.is_empty()
    }

    pub fn loc(&self) -> core::TextRange {
        self.loc
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn category(&self) -> diagnostics::Category {
        self.category
    }

    pub fn message_key(&self) -> diagnostics::Key {
        self.message_key.clone()
    }

    pub fn message_args(&self) -> &[String] {
        &self.message_args
    }

    pub fn canonical_code(&self) -> i32 {
        self.canonical_code.unwrap_or(self.code)
    }

    pub fn canonical_message_args(&self) -> &[String] {
        self.canonical_message_args
            .as_deref()
            .unwrap_or(&self.message_args)
    }

    pub fn message_chain(&self) -> &[Diagnostic] {
        &self.message_chain
    }

    pub fn related_information(&self) -> &[Diagnostic] {
        &self.related_information
    }

    pub fn reports_unnecessary(&self) -> bool {
        self.reports_unnecessary
    }

    pub fn reports_deprecated(&self) -> bool {
        self.reports_deprecated
    }

    pub fn skipped_on_no_emit(&self) -> bool {
        self.skipped_on_no_emit
    }

    pub fn repopulate_info(&self) -> Option<&RepopulateDiagnosticInfo> {
        self.repopulate_info.as_ref()
    }

    pub fn set_file(&mut self, file: Option<&SourceFile>) {
        self.file = file.map(DiagnosticFile::from_source_file);
    }

    pub(crate) fn set_diagnostic_file(&mut self, file: Option<DiagnosticFile>) {
        self.file = file;
    }

    pub(crate) fn set_diagnostic_file_recursively(&mut self, file: Option<DiagnosticFile>) {
        self.file = file.clone();
        for related in &mut self.related_information {
            related.set_diagnostic_file_recursively(file.clone());
        }
        for chain in &mut self.message_chain {
            chain.set_diagnostic_file_recursively(file.clone());
        }
    }

    pub fn set_location(&mut self, loc: core::TextRange) {
        self.loc = loc;
    }

    pub fn set_source_from_diagnostic(&mut self, diagnostic: &Diagnostic) {
        self.file = diagnostic.file.clone();
        self.loc = diagnostic.loc;
    }

    pub fn set_category(&mut self, category: diagnostics::Category) {
        self.category = category;
    }

    pub fn set_skipped_on_no_emit(&mut self) {
        self.skipped_on_no_emit = true;
    }

    pub fn set_repopulate_info(&mut self, info: Option<RepopulateDiagnosticInfo>) {
        self.repopulate_info = info;
    }

    pub fn set_canonical_head(
        &mut self,
        message: &diagnostics::Message,
        args: &[diagnostics::Argument],
    ) {
        self.canonical_code = Some(message.code());
        self.canonical_message_args = Some(args.iter().map(|arg| arg.to_string()).collect());
    }

    pub fn set_message_chain(&mut self, message_chain: Vec<Diagnostic>) -> &mut Diagnostic {
        self.message_chain = message_chain;
        self
    }

    pub fn add_message_chain(&mut self, message_chain: Option<Diagnostic>) -> &mut Diagnostic {
        if let Some(message_chain) = message_chain {
            self.message_chain.push(message_chain);
        }
        self
    }

    pub fn set_related_info(&mut self, related_information: Vec<Diagnostic>) -> &mut Diagnostic {
        self.related_information = related_information;
        self
    }

    pub fn add_related_info(
        &mut self,
        related_information: impl Into<Option<Diagnostic>>,
    ) -> &mut Diagnostic {
        if let Some(related_information) = related_information.into() {
            self.related_information.push(related_information);
        }
        self
    }

    pub fn clone_diagnostic(&self) -> Diagnostic {
        self.clone()
    }

    pub fn localize(&self, locale: locale::Locale) -> String {
        diagnostics::localize(
            locale,
            self.message.as_ref(),
            self.message_key.clone(),
            &self.message_args,
        )
    }
}

// For debugging only.
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            diagnostics::localize(
                locale::DEFAULT,
                self.message.as_ref(),
                self.message_key.clone(),
                &self.message_args,
            )
        )
    }
}

pub struct SerializedDiagnosticParams {
    pub file: Option<DiagnosticFile>,
    pub loc: core::TextRange,
    pub code: i32,
    pub category: diagnostics::Category,
    pub message_key: diagnostics::Key,
    pub message_args: Vec<String>,
    pub message_chain: Vec<Diagnostic>,
    pub related_information: Vec<Diagnostic>,
    pub reports_unnecessary: bool,
    pub reports_deprecated: bool,
    pub skipped_on_no_emit: bool,
}

pub fn new_diagnostic_from_serialized(params: SerializedDiagnosticParams) -> Diagnostic {
    let SerializedDiagnosticParams {
        file,
        loc,
        code,
        category,
        message_key,
        message_args,
        message_chain,
        related_information,
        reports_unnecessary,
        reports_deprecated,
        skipped_on_no_emit,
    } = params;
    Diagnostic {
        file,
        loc,
        code,
        category,
        message: None,
        message_key,
        message_args,
        message_chain,
        related_information,
        reports_unnecessary,
        reports_deprecated,
        skipped_on_no_emit,
        repopulate_info: None,
        canonical_code: None,
        canonical_message_args: None,
    }
}

pub fn new_diagnostic_with_file(
    file: Option<DiagnosticFile>,
    loc: core::TextRange,
    message: &diagnostics::Message,
    args: &[diagnostics::Argument],
) -> Diagnostic {
    Diagnostic {
        file,
        loc,
        code: message.code(),
        category: message.category(),
        message: Some(message.clone()),
        message_key: message.key().to_owned(),
        message_args: args.iter().map(|arg| arg.to_string()).collect(),
        message_chain: Vec::new(),
        related_information: Vec::new(),
        reports_unnecessary: message.reports_unnecessary(),
        reports_deprecated: message.reports_deprecated(),
        skipped_on_no_emit: false,
        repopulate_info: None,
        canonical_code: None,
        canonical_message_args: None,
    }
}

pub fn new_diagnostic(
    file: Option<&SourceFile>,
    loc: core::TextRange,
    message: &diagnostics::Message,
    args: &[diagnostics::Argument],
) -> Diagnostic {
    new_diagnostic_with_file(
        file.map(DiagnosticFile::from_source_file),
        loc,
        message,
        args,
    )
}

pub fn new_diagnostic_chain(
    chain: Option<Diagnostic>,
    message: &diagnostics::Message,
    args: &[diagnostics::Argument],
) -> Diagnostic {
    if let Some(chain) = chain {
        let mut diagnostic = new_diagnostic_with_file(chain.file.clone(), chain.loc, message, args);
        diagnostic
            .add_message_chain(Some(chain.clone()))
            .set_related_info(chain.related_information.clone());
        return diagnostic;
    }
    new_diagnostic(None, core::TextRange::default(), message, args)
}

pub trait DiagnosticMessageArg {
    fn as_message(&self) -> &diagnostics::Message;
}

impl DiagnosticMessageArg for diagnostics::Message {
    fn as_message(&self) -> &diagnostics::Message {
        self
    }
}

impl DiagnosticMessageArg for &diagnostics::Message {
    fn as_message(&self) -> &diagnostics::Message {
        self
    }
}

impl DiagnosticMessageArg for LazyLock<diagnostics::Message> {
    fn as_message(&self) -> &diagnostics::Message {
        self
    }
}

impl DiagnosticMessageArg for &LazyLock<diagnostics::Message> {
    fn as_message(&self) -> &diagnostics::Message {
        self
    }
}

pub fn new_compiler_diagnostic<M, A>(message: M, args: A) -> Diagnostic
where
    M: DiagnosticMessageArg,
    A: AsRef<[diagnostics::Argument]>,
{
    new_diagnostic(
        None,
        core::undefined_text_range(),
        message.as_message(),
        args.as_ref(),
    )
}

#[derive(Default)]
pub struct DiagnosticsCollection {
    inner: Mutex<DiagnosticsCollectionInner>,
}

#[derive(Default)]
struct DiagnosticsCollectionInner {
    count: usize,
    file_diagnostics: HashMap<String, Vec<Diagnostic>>,
    file_diagnostics_sorted: collections::Set<String>,
    non_file_diagnostics: Vec<Diagnostic>,
    non_file_diagnostics_sorted: bool,
}

impl DiagnosticsCollection {
    pub fn add(&self, diagnostic: Diagnostic) {
        let mut inner = self.inner.lock().unwrap_or_else(|err| err.into_inner());

        inner.count += 1;

        if let Some(file_name) = diagnostic.file_name() {
            let file_name = file_name.to_owned();
            inner
                .file_diagnostics
                .entry(file_name.clone())
                .or_default()
                .push(diagnostic);
            inner.file_diagnostics_sorted.delete(&file_name);
        } else {
            inner.non_file_diagnostics.push(diagnostic);
            inner.non_file_diagnostics_sorted = false;
        }
    }

    pub fn lookup(&self, diagnostic: &Diagnostic) -> Option<Diagnostic> {
        let mut inner = self.inner.lock().unwrap_or_else(|err| err.into_inner());

        let diagnostics = if let Some(file_name) = diagnostic.file_name() {
            Self::get_diagnostics_for_file_locked(&mut inner, file_name)
        } else {
            Self::get_global_diagnostics_locked(&mut inner)
        };
        diagnostics
            .binary_search_by(|candidate| compare_diagnostics(candidate, diagnostic).cmp(&0))
            .ok()
            .map(|i| diagnostics[i].clone())
    }

    pub fn add_or_update_by_key<R>(
        &self,
        diagnostic: Diagnostic,
        update: impl FnOnce(&mut Diagnostic) -> R,
    ) -> R {
        let mut inner = self.inner.lock().unwrap_or_else(|err| err.into_inner());

        if let Some(file_name) = diagnostic.file_name().map(str::to_owned) {
            let (result, inserted) = {
                let diagnostics = inner.file_diagnostics.entry(file_name.clone()).or_default();
                let index = diagnostics.iter().position(|candidate| {
                    equal_diagnostics_no_related_info(candidate, &diagnostic)
                });
                let inserted = index.is_none();
                let index = if let Some(index) = index {
                    index
                } else {
                    diagnostics.push(diagnostic);
                    diagnostics.len() - 1
                };
                (update(&mut diagnostics[index]), inserted)
            };
            if inserted {
                inner.count += 1;
            }
            inner.file_diagnostics_sorted.delete(&file_name);
            result
        } else {
            let index = inner
                .non_file_diagnostics
                .iter()
                .position(|candidate| equal_diagnostics_no_related_info(candidate, &diagnostic));
            let index = if let Some(index) = index {
                index
            } else {
                inner.count += 1;
                inner.non_file_diagnostics.push(diagnostic);
                inner.non_file_diagnostics.len() - 1
            };
            let result = update(&mut inner.non_file_diagnostics[index]);
            inner.non_file_diagnostics_sorted = false;
            result
        }
    }

    pub fn add_or_update<R>(
        &self,
        diagnostic: Diagnostic,
        update: impl FnOnce(&mut Diagnostic) -> R,
    ) -> R {
        let mut inner = self.inner.lock().unwrap_or_else(|err| err.into_inner());

        if let Some(file_name) = diagnostic.file_name().map(str::to_owned) {
            let (result, inserted) = {
                let diagnostics = inner.file_diagnostics.entry(file_name.clone()).or_default();
                let index = diagnostics
                    .iter()
                    .position(|candidate| compare_diagnostics(candidate, &diagnostic) == 0);
                let inserted = index.is_none();
                let index = if let Some(index) = index {
                    index
                } else {
                    diagnostics.push(diagnostic);
                    diagnostics.len() - 1
                };
                (update(&mut diagnostics[index]), inserted)
            };
            if inserted {
                inner.count += 1;
            }
            inner.file_diagnostics_sorted.delete(&file_name);
            result
        } else {
            let index = inner
                .non_file_diagnostics
                .iter()
                .position(|candidate| compare_diagnostics(candidate, &diagnostic) == 0);
            let index = if let Some(index) = index {
                index
            } else {
                inner.count += 1;
                inner.non_file_diagnostics.push(diagnostic);
                inner.non_file_diagnostics.len() - 1
            };
            let result = update(&mut inner.non_file_diagnostics[index]);
            inner.non_file_diagnostics_sorted = false;
            result
        }
    }

    pub fn get_global_diagnostics(&self) -> Vec<Diagnostic> {
        let mut inner = self.inner.lock().unwrap_or_else(|err| err.into_inner());

        Self::get_global_diagnostics_locked(&mut inner)
    }

    fn get_global_diagnostics_locked(inner: &mut DiagnosticsCollectionInner) -> Vec<Diagnostic> {
        if !inner.non_file_diagnostics_sorted {
            inner
                .non_file_diagnostics
                .sort_by(compare_diagnostics_ordering);
            inner.non_file_diagnostics_sorted = true;
        }
        inner.non_file_diagnostics.clone()
    }

    pub fn get_diagnostics_for_file(&self, file_name: &str) -> Vec<Diagnostic> {
        let mut inner = self.inner.lock().unwrap_or_else(|err| err.into_inner());

        Self::get_diagnostics_for_file_locked(&mut inner, file_name)
    }

    fn get_diagnostics_for_file_locked(
        inner: &mut DiagnosticsCollectionInner,
        file_name: &str,
    ) -> Vec<Diagnostic> {
        if !inner.file_diagnostics_sorted.has(&file_name.to_owned()) {
            if let Some(diagnostics) = inner.file_diagnostics.get_mut(file_name) {
                diagnostics.sort_by(compare_diagnostics_ordering);
            }
            inner.file_diagnostics_sorted.add(file_name.to_owned());
        }
        inner
            .file_diagnostics
            .get(file_name)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_diagnostics(&self) -> Vec<Diagnostic> {
        let inner = self.inner.lock().unwrap_or_else(|err| err.into_inner());

        let mut diagnostics = Vec::with_capacity(inner.count);
        diagnostics.extend(inner.non_file_diagnostics.clone());
        for diags in inner.file_diagnostics.values() {
            diagnostics.extend(diags.clone());
        }
        diagnostics.sort_by(compare_diagnostics_ordering);
        diagnostics
    }
}

fn get_diagnostic_path(d: &Diagnostic) -> String {
    if let Some(file) = d.file_name() {
        return file.to_owned();
    }
    String::new()
}

pub fn equal_diagnostics(d1: &Diagnostic, d2: &Diagnostic) -> bool {
    if std::ptr::eq(d1, d2) {
        return true;
    }
    equal_diagnostics_no_related_info(d1, d2)
        && d1
            .related_information()
            .iter()
            .zip(d2.related_information())
            .all(|(d1, d2)| equal_diagnostics(d1, d2))
        && d1.related_information().len() == d2.related_information().len()
}

pub fn equal_diagnostics_no_related_info(d1: &Diagnostic, d2: &Diagnostic) -> bool {
    if std::ptr::eq(d1, d2) {
        return true;
    }
    get_diagnostic_path(d1) == get_diagnostic_path(d2)
        && d1.loc() == d2.loc()
        && d1.code() == d2.code()
        && d1.message_args() == d2.message_args()
        && d1
            .message_chain()
            .iter()
            .zip(d2.message_chain())
            .all(|(c1, c2)| equal_message_chain(c1, c2))
        && d1.message_chain().len() == d2.message_chain().len()
}

fn equal_message_chain(c1: &Diagnostic, c2: &Diagnostic) -> bool {
    if std::ptr::eq(c1, c2) {
        return true;
    }
    c1.code() == c2.code()
        && c1.message_args() == c2.message_args()
        && c1
            .message_chain()
            .iter()
            .zip(c2.message_chain())
            .all(|(c1, c2)| equal_message_chain(c1, c2))
        && c1.message_chain().len() == c2.message_chain().len()
}

fn compare_message_chain_size(c1: &[Diagnostic], c2: &[Diagnostic]) -> i32 {
    let mut c = c2.len() as i32 - c1.len() as i32;
    if c != 0 {
        return c;
    }
    for i in 0..c1.len() {
        c = compare_message_chain_size(c1[i].message_chain(), c2[i].message_chain());
        if c != 0 {
            return c;
        }
    }
    0
}

fn compare_message_chain_content(c1: &[Diagnostic], c2: &[Diagnostic]) -> i32 {
    for i in 0..c1.len() {
        let c = compare_string_slices(c1[i].message_args(), c2[i].message_args());
        if c != 0 {
            return c;
        }
        if !c1[i].message_chain().is_empty() {
            let c = compare_message_chain_content(c1[i].message_chain(), c2[i].message_chain());
            if c != 0 {
                return c;
            }
        }
    }
    0
}

fn compare_related_info(r1: &[Diagnostic], r2: &[Diagnostic]) -> i32 {
    let mut c = r2.len() as i32 - r1.len() as i32;
    if c != 0 {
        return c;
    }
    for i in 0..r1.len() {
        c = compare_diagnostics(&r1[i], &r2[i]);
        if c != 0 {
            return c;
        }
    }
    0
}

pub fn compare_diagnostics(d1: &Diagnostic, d2: &Diagnostic) -> i32 {
    if std::ptr::eq(d1, d2) {
        return 0;
    }
    let mut c = compare_strings(&get_diagnostic_path(d1), &get_diagnostic_path(d2));
    if c != 0 {
        return c;
    }
    c = d1.loc().pos() - d2.loc().pos();
    if c != 0 {
        return c;
    }
    c = d1.loc().end() - d2.loc().end();
    if c != 0 {
        return c;
    }
    c = d1.canonical_code() - d2.canonical_code();
    if c != 0 {
        return c;
    }
    c = compare_string_slices(d1.canonical_message_args(), d2.canonical_message_args());
    if c != 0 {
        return c;
    }
    c = compare_message_chain_size(d1.message_chain(), d2.message_chain());
    if c != 0 {
        return c;
    }
    c = compare_message_chain_content(d1.message_chain(), d2.message_chain());
    if c != 0 {
        return c;
    }
    compare_related_info(d1.related_information(), d2.related_information())
}

fn compare_diagnostics_ordering(d1: &Diagnostic, d2: &Diagnostic) -> Ordering {
    compare_diagnostics(d1, d2).cmp(&0)
}

fn compare_strings(a: &str, b: &str) -> i32 {
    match a.cmp(b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

fn compare_string_slices(a: &[String], b: &[String]) -> i32 {
    for (a, b) in a.iter().zip(b) {
        let c = compare_strings(a, b);
        if c != 0 {
            return c;
        }
    }
    a.len() as i32 - b.len() as i32
}
