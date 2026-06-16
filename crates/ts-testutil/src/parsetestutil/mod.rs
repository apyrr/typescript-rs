mod implementation;

pub use implementation::{
    ScriptKind, SourceFile, SyntheticRecursive, TextRange, check_diagnostics,
    check_diagnostics_message, format_diagnostics, mark_synthetic_recursive, parse_type_script,
};
