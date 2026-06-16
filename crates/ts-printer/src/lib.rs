#![forbid(unsafe_code)]
#![expect(
    non_camel_case_types,
    reason = "ported printer names mirror upstream conventions"
)]

#[expect(
    dead_code,
    reason = "ported change-tracker writer helpers are ahead of current callers"
)]
mod changetrackerwriter;
mod emitcontext;
mod emitflags;
mod emithost;
mod emitresolver;
mod emittextwriter;
mod factory;
mod generatedidentifierflags;
mod helpers;
mod namegenerator;
#[expect(
    dead_code,
    unused_assignments,
    reason = "ported printer helpers are ahead of current callers"
)]
mod printer;
mod singlelinestringwriter;
mod sourcefilemetadataprovider;
mod textwriter;
mod utilities;

pub use ts_core::UTF16Offset;

pub use changetrackerwriter::{ChangeTrackerWriter, new_change_tracker_writer};
pub use emitcontext::{
    AutoGenerateId, AutoGenerateInfo, AutoGenerateOptions, EmitContext, EnvironmentFlags, VarScope,
    get_emit_context, new_emit_context,
};
pub use emitflags::*;
pub use emithost::{EmitBindingFacts, EmitHost, with_emit_resolver};
pub use emitresolver::{
    EmitResolver, SymbolAccessibility, SymbolAccessibilityResult, TypeReferenceSerializationKind,
};
pub use emittextwriter::{
    EmitTextWriter, SharedEmitTextWriter, SharedEmitTextWriterHandle, share_text_writer,
};
pub use factory::{AssignedNameOptions, NameOptions, NodeFactory, PrivateIdentifierKind};
pub use generatedidentifierflags::GeneratedIdentifierFlags;
pub use helpers::{
    ADVANCED_ASYNC_SUPER_HELPER, ASYNC_SUPER_HELPER, EmitHelper, METADATA_HELPER, Priority,
    compare_emit_helpers, helper_from_key,
};
pub use namegenerator::{
    LocalNameBindingFacts, NameGenerator, TEMP_FLAGS_AUTO, TEMP_FLAGS_COUNT_MASK, TEMP_FLAGS_I,
    TempFlags, is_unique_local_name,
};
pub use printer::{
    PrintHandlers, Printer, PrinterOptions, TokenEmitFlags, WriteKind, get_lines_between_positions,
    positions_are_on_same_line, range_is_on_single_line, range_start_positions_are_on_same_line,
};
pub use singlelinestringwriter::{SingleLineStringWriter, get_single_line_string_writer};
pub use sourcefilemetadataprovider::{SourceFileMetaData, SourceFileMetaDataProvider};
pub use textwriter::{
    DEFAULT_INDENT_SIZE, TextWriter, get_default_indent_size, get_indent_string,
    new_shared_text_writer, new_text_writer,
};
pub use utilities::{
    GetLiteralTextFlags, LineCharacterCache, QuoteChar, ensure_leading_hash, escape_string,
    format_generated_name, is_file_level_unique_name, is_pinned_comment,
    is_recognized_triple_slash_comment, make_identifier_from_module_name, remove_leading_hash,
};

#[cfg(test)]
mod namegenerator_test;
#[cfg(test)]
mod printer_test;
#[cfg(test)]
mod utilities_test;

pub const SYMBOL_ACCESSIBILITY_ACCESSIBLE: SymbolAccessibility = SymbolAccessibility::Accessible;
pub const GENERATED_IDENTIFIER_FLAGS_OPTIMISTIC: GeneratedIdentifierFlags =
    GeneratedIdentifierFlags::OPTIMISTIC;

pub fn new_node_factory(context: &mut EmitContext) -> NodeFactory {
    factory::new_node_factory(context)
}

pub fn new_printer(
    options: PrinterOptions,
    handlers: PrintHandlers,
    emit_context: Option<EmitContext>,
) -> Printer {
    printer::new_printer(options, handlers, emit_context)
}

impl Printer {
    pub fn new(
        options: PrinterOptions,
        handlers: PrintHandlers,
        emit_context: Option<EmitContext>,
    ) -> Self {
        new_printer(options, handlers, emit_context)
    }
}
