//! Port of `internal/tsoptions/commandlineoption.go`.
//!
//! The concrete command-line option model currently lives in `lib.rs` so older
//! call sites keep importing the same root symbols. This file records the Go
//! file boundary and mirrors the exported API surface for the port checklist.

pub use crate::{
    COMMAND_LINE_OPTION_TYPE_BOOLEAN, COMMAND_LINE_OPTION_TYPE_ENUM, COMMAND_LINE_OPTION_TYPE_LIST,
    COMMAND_LINE_OPTION_TYPE_LIST_OR_ELEMENT, COMMAND_LINE_OPTION_TYPE_NUMBER,
    COMMAND_LINE_OPTION_TYPE_OBJECT, COMMAND_LINE_OPTION_TYPE_STRING, CommandLineOption,
    CommandLineOptionKind, CommandLineOptionNameMap, CompilerOptionsValue, DefaultValueDescription,
    ExtraValidation, Tristate, command_line_option_deprecated, command_line_option_elements,
    command_line_option_enum_map,
};
