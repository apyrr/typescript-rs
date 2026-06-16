use crate::parsetestutil::{check_diagnostics_message, parse_type_script};

pub trait EmitContextLike {
    fn emit_source_file(&self, file: &impl SourceFileLike) -> String;
}

pub trait SourceFileLike {
    fn language_variant_is_jsx(&self) -> bool;
}

pub fn check_emit(
    emit_context: &impl EmitContextLike,
    file: &impl SourceFileLike,
    expected: &str,
) -> Result<(), String> {
    let text = emit_context.emit_source_file(file);
    let actual = text.strip_suffix('\n').unwrap_or(&text);
    if actual != expected {
        return Err(format!(
            "emit mismatch\nexpected:\n{expected}\nactual:\n{actual}"
        ));
    }
    let reparsed = parse_type_script(&text, file.language_variant_is_jsx());
    check_diagnostics_message(&reparsed, "error on reparse: ")
}
