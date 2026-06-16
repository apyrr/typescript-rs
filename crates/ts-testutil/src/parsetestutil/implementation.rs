#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceFile {
    pub file_name: String,
    pub path: String,
    pub script_kind: ScriptKind,
    pub text: String,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScriptKind {
    #[default]
    TypeScript,
    Tsx,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextRange {
    pub pos: isize,
    pub end: isize,
}

impl TextRange {
    pub const fn undefined() -> Self {
        Self { pos: -1, end: -1 }
    }
}

pub trait SyntheticRecursive {
    fn mark_synthetic_recursive(&mut self);
}

pub fn parse_type_script(text: &str, jsx: bool) -> SourceFile {
    let file_name = if jsx { "/main.tsx" } else { "/main.ts" };
    SourceFile {
        file_name: file_name.to_string(),
        path: file_name.to_string(),
        script_kind: if jsx {
            ScriptKind::Tsx
        } else {
            ScriptKind::TypeScript
        },
        text: text.to_string(),
        diagnostics: Vec::new(),
    }
}

pub fn check_diagnostics(file: &SourceFile) -> Result<(), String> {
    if file.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(format_diagnostics(&file.diagnostics))
    }
}

pub fn check_diagnostics_message(file: &SourceFile, message: &str) -> Result<(), String> {
    if file.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{message}{}",
            format_diagnostics(&file.diagnostics)
        ))
    }
}

pub fn format_diagnostics(diagnostics: &[String]) -> String {
    diagnostics.join("\n")
}

pub fn mark_synthetic_recursive<T>(node: &mut T)
where
    T: SyntheticRecursive,
{
    node.mark_synthetic_recursive();
}
