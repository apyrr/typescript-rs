use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{DidYouMeanOptionsDiagnostics, ParseCommandLineWorkerDiagnostics};
use crate::namemap::NameMap;
use crate::parsedcommandline::ParsedCommandLine;
use crate::{CommandLineOption, CommandLineOptionKind, ExtraValidation};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandLineParser {
    pub diagnostics: ParseCommandLineWorkerDiagnostics,
    pub name_map: NameMap,
}

impl CommandLineParser {
    pub fn unknown_option_diagnostic(&self) -> &str {
        &self.diagnostics.did_you_mean.unknown_option_diagnostic
    }

    pub fn alternate_mode(&self) -> Option<&crate::diagnostics::AlternateModeDiagnostics> {
        self.diagnostics.did_you_mean.alternate_mode.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseOptionValue {
    Bool(bool),
    String(String),
    List(Vec<String>),
}

pub fn parse_command_line_worker(
    diagnostics: DidYouMeanOptionsDiagnostics,
    args: &[String],
) -> ParsedCommandLine {
    let mut parsed = ParsedCommandLine::default();
    let options_map = crate::namemap::get_name_map_from_list(&diagnostics.option_declarations);
    let watch_options = crate::options_for_watch();
    let watch_options_map = crate::namemap::watch_name_map(&watch_options);
    let watch_diagnostics = crate::watch_options_did_you_mean_diagnostics(&watch_options);
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        index += 1;
        if arg.is_empty() {
            continue;
        }
        if let Some(response_file) = arg.strip_prefix('@') {
            parsed
                .errors
                .push(format!("Cannot_read_file_0: {}", response_file));
            continue;
        }
        if arg.starts_with('-') {
            let input_name = get_input_option_name(arg);
            let Some(option) = options_map.get_option_declaration_from_name(&input_name, true)
            else {
                if let Some(watch_option) =
                    watch_options_map.get_option_declaration_from_name(&input_name, true)
                {
                    parse_known_option(
                        &mut parsed.watch_options,
                        &mut parsed.explicit_null_watch_options,
                        &mut parsed.errors,
                        args,
                        &mut index,
                        watch_option,
                        &watch_diagnostics.did_you_mean,
                    );
                    continue;
                }
                if let Some(alternate_mode) = &diagnostics.alternate_mode
                    && let Some(options_name_map) = &alternate_mode.options_name_map
                    && let Some(other_option) =
                        options_name_map.get_option_declaration_from_name(&input_name, true)
                {
                    let diagnostic = if other_option.name == "build" {
                        "Option_build_must_be_the_first_command_line_argument"
                    } else {
                        &alternate_mode.diagnostic
                    };
                    parsed.errors.push(format!("{diagnostic}: {input_name}"));
                    continue;
                }
                parsed.errors.push(format!(
                    "{}: {}",
                    diagnostics.unknown_option_diagnostic, arg
                ));
                continue;
            };

            parse_known_option(
                &mut parsed.options,
                &mut parsed.explicit_null_options,
                &mut parsed.errors,
                args,
                &mut index,
                option,
                &diagnostics,
            );
        } else {
            parsed.file_names.push(arg.clone());
        }
    }
    if diagnostics.option_declarations.is_empty() {
        parsed
            .errors
            .push("no option declarations supplied to parser".to_owned());
    }
    parsed
}

fn parse_known_option(
    parsed_options: &mut BTreeMap<String, String>,
    explicit_null_options: &mut BTreeSet<String>,
    errors: &mut Vec<String>,
    args: &[String],
    index: &mut usize,
    option: &CommandLineOption,
    diagnostics: &DidYouMeanOptionsDiagnostics,
) {
    if option.is_tsconfig_only {
        let option_value = if let Some(value) = args.get(*index) {
            value.as_str()
        } else {
            ""
        };
        if option_value == "null" {
            set_parsed_option(parsed_options, explicit_null_options, &option.name, "null");
            explicit_null_options.insert(option.name.clone());
            *index += 1;
        } else if option.kind == Some(CommandLineOptionKind::Boolean) {
            if option_value == "false" {
                set_parsed_option(parsed_options, explicit_null_options, &option.name, "false");
                *index += 1;
            } else {
                if option_value == "true" {
                    *index += 1;
                }
                errors.push(format!(
                    "Option_0_can_only_be_specified_in_tsconfig_json_file_or_set_to_false_or_null_on_command_line: {}",
                    option.name
                ));
            }
        } else {
            errors.push(format!(
                "Option_0_can_only_be_specified_in_tsconfig_json_file_or_set_to_null_on_command_line: {}",
                option.name
            ));
            if !option_value.is_empty() && !option_value.starts_with('-') {
                *index += 1;
            }
        }
        return;
    }

    if args.get(*index).is_some_and(|value| value == "null") {
        set_parsed_option(parsed_options, explicit_null_options, &option.name, "null");
        explicit_null_options.insert(option.name.clone());
        *index += 1;
        return;
    }

    if option.kind == Some(CommandLineOptionKind::Boolean) {
        // boolean flag has optional value true, false, others
        let option_value = args.get(*index).map(String::as_str).unwrap_or("");

        // check next argument as boolean flag value
        if option_value == "false" {
            set_parsed_option(parsed_options, explicit_null_options, &option.name, "false");
        } else {
            set_parsed_option(parsed_options, explicit_null_options, &option.name, "true");
        }
        // try to consume next argument as value for boolean flag; do not consume argument if it is not "true" or "false"
        if option_value == "false" || option_value == "true" {
            *index += 1;
        }
        return;
    }

    // Check to see if no argument was provided (e.g. "--locale" is the last command-line argument).
    if *index >= args.len() {
        errors.push(format!(
            "{}: {}",
            diagnostics.unknown_did_you_mean_diagnostic, option.name
        ));
        if option.kind == Some(CommandLineOptionKind::List) {
            set_parsed_option(parsed_options, explicit_null_options, &option.name, "");
        }
        return;
    }

    if option.kind == Some(CommandLineOptionKind::List) && args[*index].starts_with('-') {
        set_parsed_option(parsed_options, explicit_null_options, &option.name, "");
        return;
    }

    if option.kind == Some(CommandLineOptionKind::String) {
        if let Some(error) = validate_extra_option_value(option, &args[*index]) {
            errors.push(error);
        } else {
            set_parsed_option(
                parsed_options,
                explicit_null_options,
                &option.name,
                &args[*index],
            );
        }
        *index += 1;
        return;
    }
    if option.kind == Some(CommandLineOptionKind::List) {
        validate_list_option_value(option, &args[*index], errors);
    } else if option.kind == Some(CommandLineOptionKind::Enum) {
        validate_enum_option_value(option, &args[*index], errors);
    }
    set_parsed_option(
        parsed_options,
        explicit_null_options,
        &option.name,
        &args[*index],
    );
    *index += 1;
}

fn set_parsed_option(
    parsed_options: &mut BTreeMap<String, String>,
    explicit_null_options: &mut BTreeSet<String>,
    name: &str,
    value: &str,
) {
    explicit_null_options.remove(name);
    parsed_options.insert(name.to_owned(), value.to_owned());
}

fn validate_extra_option_value(option: &CommandLineOption, value: &str) -> Option<String> {
    match option.extra_validation {
        ExtraValidation::Locale => {
            if ts_locale::parse(value).1 {
                None
            } else {
                Some("Locale_must_be_an_IETF_BCP_47_language_tag_Examples_Colon_0_1".to_owned())
            }
        }
        ExtraValidation::None | ExtraValidation::Spec => None,
    }
}

pub fn parse_command_line(
    args: &[String],
    diagnostics: DidYouMeanOptionsDiagnostics,
) -> ParsedCommandLine {
    parse_command_line_worker(diagnostics, args)
}

pub fn get_input_option_name(input: &str) -> String {
    // removes at most two leading '-' from the input string
    let input = if let Some(stripped) = input.strip_prefix('-') {
        stripped
    } else {
        input
    };
    if let Some(stripped) = input.strip_prefix('-') {
        stripped.to_owned()
    } else {
        input.to_owned()
    }
}

pub fn parse_custom_type_option(
    option: &CommandLineOption,
    value: &str,
) -> Option<ParseOptionValue> {
    match option.kind {
        Some(CommandLineOptionKind::Boolean) => Some(ParseOptionValue::Bool(value == "true")),
        Some(CommandLineOptionKind::List) | Some(CommandLineOptionKind::ListOrElement) => Some(
            ParseOptionValue::List(value.split(',').map(str::to_owned).collect()),
        ),
        Some(_) => Some(ParseOptionValue::String(value.to_owned())),
        None => None,
    }
}

pub fn convert_enable_option_value(value: Option<&str>) -> bool {
    value.map(|value| value != "false").unwrap_or(true)
}

pub fn parse_option_value(args: &BTreeMap<String, String>, name: &str) -> Option<String> {
    args.get(name).cloned()
}

fn validate_list_option_value(option: &CommandLineOption, value: &str, errors: &mut Vec<String>) {
    let Some(element) = option.elements() else {
        return;
    };
    if element.kind != Some(CommandLineOptionKind::Enum) {
        return;
    }
    let Some(enum_map) = element.enum_map() else {
        return;
    };
    if value
        .split(',')
        .map(|entry| entry.trim().to_ascii_lowercase())
        .any(|entry| !entry.is_empty() && !enum_map.contains_key(&entry))
    {
        errors.push(invalid_enum_error(option));
    }
}

fn validate_enum_option_value(option: &CommandLineOption, value: &str, errors: &mut Vec<String>) {
    let Some(enum_map) = option.enum_map() else {
        return;
    };
    if !enum_map.contains_key(&value.trim().to_ascii_lowercase()) {
        errors.push(invalid_enum_error(option));
    }
}

pub(crate) fn invalid_enum_error(option: &CommandLineOption) -> String {
    let keys = if option.name == "lib" {
        crate::enummaps::lib_names()
    } else {
        crate::enummaps::enum_keys(&option.name).expect("enum option keys")
    };
    format!(
        "Argument_for_0_option_must_be_Colon_1: --{}\u{1f}{}",
        option.name,
        crate::errors::format_enum_type_keys(option, keys)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declsbuild::build_opts;
    use crate::declscompiler::options_declarations;
    use crate::diagnostics::get_parse_command_line_worker_diagnostics;
    use crate::namemap::build_name_map;

    #[test]
    fn allows_tsconfig_only_option_to_be_set_to_null() {
        let diagnostics = get_parse_command_line_worker_diagnostics(
            options_declarations(),
            build_name_map(&build_opts()),
        );
        let args = ["--composite", "null", "-tsBuildInfoFile", "null", "0.ts"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let parsed = parse_command_line_worker(diagnostics.did_you_mean, &args);

        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert_eq!(
            parsed.options.get("composite").map(String::as_str),
            Some("null")
        );
        assert_eq!(
            parsed.options.get("tsBuildInfoFile").map(String::as_str),
            Some("null")
        );
        assert_eq!(parsed.file_names, vec!["0.ts"]);
        assert!(parsed.compiler_options().composite.is_unknown());
    }

    #[test]
    fn public_parse_preserves_composite_false_with_build_info_null() {
        let host = crate::tsoptionstest::VfsParseConfigHost::new(
            std::collections::BTreeMap::new(),
            "/project",
            true,
        );
        let args = ["--composite", "false", "--tsBuildInfoFile", "null"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let parsed = crate::parse_command_line(&args, host);

        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert_eq!(
            parsed.options.get("composite").map(String::as_str),
            Some("false")
        );
        assert!(parsed.compiler_options().composite.is_false());
    }

    #[test]
    fn public_parse_converts_lib_names_to_lib_file_names() {
        let host = crate::tsoptionstest::VfsParseConfigHost::new(
            std::collections::BTreeMap::new(),
            "/project",
            true,
        );
        let args = ["--lib", "es6 ", "first.ts"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let parsed = crate::parse_command_line(&args, host);

        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert_eq!(parsed.file_names, vec!["first.ts"]);
        assert_eq!(parsed.compiler_options().lib, vec!["lib.es2015.d.ts"]);
    }

    #[test]
    fn public_parse_build_accepts_help() {
        let host = crate::tsoptionstest::VfsParseConfigHost::new(
            std::collections::BTreeMap::new(),
            "/project",
            true,
        );
        let args = ["--build", "--help"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let parsed = crate::parse_build_command_line(&args, host);

        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert!(parsed.compiler_options.help.is_true());
        assert_eq!(parsed.projects, vec!["."]);
    }

    #[test]
    fn public_parse_accepts_watch_options() {
        let host = crate::tsoptionstest::VfsParseConfigHost::new(
            std::collections::BTreeMap::new(),
            "/project",
            true,
        );
        let args = ["-w", "--watchInterval", "1000"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let parsed = crate::parse_command_line(&args, host);

        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert_eq!(
            parsed
                .watch_options
                .get("watchInterval")
                .map(String::as_str),
            Some("1000")
        );
    }
}
