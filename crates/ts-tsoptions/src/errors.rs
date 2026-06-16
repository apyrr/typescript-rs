use crate::CommandLineOption;
use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_scanner as scanner;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub message: String,
    pub args: Vec<String>,
}

pub fn create_diagnostic_for_invalid_enum_type(opt: &CommandLineOption) -> Diagnostic {
    let names = crate::enummaps::enum_keys(&opt.name).expect("enum option keys");
    Diagnostic {
        message: "Argument_for_0_option_must_be_Colon_1".to_owned(),
        args: vec![format!("--{}", opt.name), format_enum_type_keys(opt, names)],
    }
}

pub fn format_enum_type_keys(opt: &CommandLineOption, mut keys: Vec<String>) -> String {
    if let Some(deprecated) = opt.deprecated_keys() {
        keys.retain(|key| !deprecated.contains(key));
    }
    format!("'{}'", keys.join("', '"))
}

pub fn get_compiler_option_value_type_string(option: &CommandLineOption) -> String {
    match option.kind {
        Some(crate::CommandLineOptionKind::ListOrElement) => option
            .elements()
            .map(|element| {
                format!(
                    "{} or Array",
                    get_compiler_option_value_type_string(&element)
                )
            })
            .unwrap_or_else(|| "Array".to_owned()),
        Some(crate::CommandLineOptionKind::List) => "Array".to_owned(),
        Some(kind) => kind.as_str().to_owned(),
        None => String::new(),
    }
}

pub fn create_unknown_option_error(
    unknown_option: &str,
    unknown_option_diagnostic: &str,
    unknown_option_error_text: Option<&str>,
) -> Diagnostic {
    Diagnostic {
        message: unknown_option_diagnostic.to_owned(),
        args: vec![
            unknown_option_error_text
                .unwrap_or(unknown_option)
                .to_owned(),
        ],
    }
}

pub fn create_diagnostic_for_node_in_source_file_or_compiler_diagnostic(
    message: &str,
    args: &[String],
) -> Diagnostic {
    Diagnostic {
        message: message.to_owned(),
        args: args.to_vec(),
    }
}

pub fn create_diagnostic_for_node_in_source_file(
    source_file: &ast::SourceFile,
    node: ast::Node,
    message: &diagnostics::Message,
    args: &[diagnostics::Argument],
) -> ast::Diagnostic {
    let store = source_file.store();
    let loc = store.loc(node);
    ast::new_diagnostic(
        Some(source_file),
        core::new_text_range(
            scanner::skip_trivia(source_file.text(), loc.pos() as usize) as i32,
            loc.end(),
        ),
        message,
        args,
    )
}

pub fn create_diagnostic_for_ast_node_in_source_file_or_compiler_diagnostic(
    source_file: Option<&ast::SourceFile>,
    node: Option<ast::Node>,
    message: &diagnostics::Message,
    args: &[diagnostics::Argument],
) -> ast::Diagnostic {
    if let (Some(source_file), Some(node)) = (source_file, node) {
        return create_diagnostic_for_node_in_source_file(source_file, node, message, args);
    }
    ast::new_compiler_diagnostic(message, args)
}

pub fn extra_key_diagnostics(s: &str) -> Option<&'static str> {
    match s {
        "compilerOptions" => Some("Unknown_compiler_option_0"),
        "watchOptions" => Some("Unknown_watch_option_0"),
        "typeAcquisition" => Some("Unknown_type_acquisition_option_0"),
        "buildOptions" => Some("Unknown_build_option_0"),
        _ => None,
    }
}

pub fn extra_key_did_you_mean_diagnostics(s: &str) -> Option<&'static str> {
    match s {
        "compilerOptions" => Some("Unknown_compiler_option_0_Did_you_mean_1"),
        "watchOptions" => Some("Unknown_watch_option_0_Did_you_mean_1"),
        "typeAcquisition" => Some("Unknown_type_acquisition_option_0_Did_you_mean_1"),
        "buildOptions" => Some("Unknown_build_option_0_Did_you_mean_1"),
        _ => None,
    }
}
