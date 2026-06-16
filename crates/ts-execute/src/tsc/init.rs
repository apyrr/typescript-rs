use ts_ast as ast;
use ts_collections::OrderedMap;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_json as json;
use ts_json::Value;
use ts_locale as locale;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use super::{DiagnosticReporter, System, get_header};

const TAB: &str = "  ";

fn emit_header(result: &mut Vec<String>, locale: &locale::Locale, header: &diagnostics::Message) {
    result.push(format!(
        "{TAB}{TAB}// {}",
        header.localize(locale.clone(), vec![])
    ));
}

pub fn write_config_file(
    mut sys: System,
    locale: locale::Locale,
    report_diagnostic: DiagnosticReporter,
    options: OrderedMap<String, Value>,
) {
    let get_current_directory = sys.get_current_directory();
    let file = tspath::normalize_path(&tspath::combine_paths(
        &get_current_directory,
        &["tsconfig.json"],
    ));
    if sys.fs().file_exists(&file) {
        let args: Vec<diagnostics::Argument> = vec![Box::new(file.clone())];
        report_diagnostic(ast::new_compiler_diagnostic(
            &diagnostics::A_tsconfig_json_file_is_already_defined_at_Colon_0,
            &args,
        ));
    } else {
        let _ = sys
            .fs()
            .write_file(&file, &generate_ts_config(options, locale));
        let mut output = vec!["\n".to_owned()];
        output.extend(get_header(
            sys.clone(),
            "Created a new tsconfig.json".to_owned(),
        ));
        output.push("You can learn more at https://aka.ms/tsconfig".to_owned());
        output.push("\n".to_owned());
        let _ = write!(sys.writer(), "{}", output.join(""));
    }
}

pub fn generate_ts_config(options: OrderedMap<String, Value>, locale: locale::Locale) -> String {
    let mut result = Vec::new();

    let mut all_set_options = Vec::new();
    for k in options.keys() {
        if k != "init" && k != "help" && k != "watch" {
            all_set_options.push(k.clone());
        }
    }

    let newline = |result: &mut Vec<String>| {
        result.push(String::new());
    };
    let push = |result: &mut Vec<String>, args: Vec<String>| {
        result.extend(args);
    };

    let compiler_options_value_matches =
        |value: &Value, compiler_options_value: &tsoptions::CompilerOptionsValue| -> bool {
            match (value, compiler_options_value) {
                (Value::Bool(value), tsoptions::CompilerOptionsValue::Bool(option_value)) => {
                    value == option_value
                }
                (Value::String(value), tsoptions::CompilerOptionsValue::String(option_value)) => {
                    value == option_value
                }
                (Value::Number(value), tsoptions::CompilerOptionsValue::Number(option_value)) => {
                    value.as_i64() == Some(i64::from(*option_value))
                }
                _ => false,
            }
        };

    let format_panic_value = |value: &Value| -> String {
        if let Value::String(value) = value {
            value.clone()
        } else {
            value.to_string()
        }
    };

    let format_single_value = |mut value: Value, enum_map: Option<&tsoptions::EnumMap>| -> String {
        if let Some(enum_map) = enum_map {
            let mut found = value
                .as_str()
                .is_some_and(|text| enum_map.contains_key(text));
            if !found {
                for (k, v) in enum_map.iter() {
                    if compiler_options_value_matches(&value, v) {
                        value = Value::String(k.clone());
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                panic!("No matching value of {}", format_panic_value(&value));
            }
        }

        String::from_utf8(
            json::marshal_indent(&value, "", "")
                .unwrap_or_else(|err| panic!("should not happen: {err}")),
        )
        .unwrap_or_else(|err| panic!("should not happen: {err}"))
    };

    let format_value_or_array = |setting_name: &str, value: Value| -> String {
        let mut option = None;
        for decl in tsoptions::options_declarations() {
            if decl.name == setting_name {
                option = Some(decl);
                break;
            }
        }
        let Some(option) = option else {
            panic!("No option named {setting_name}");
        };

        if let Some(arr) = value.as_array() {
            let enum_map = option.elements().and_then(|element| element.enum_map());

            let mut elems = Vec::new();
            for elem in arr {
                elems.push(format_single_value(elem.clone(), enum_map));
            }
            format!("[{}]", elems.join(", "))
        } else {
            let enum_map = option.enum_map();
            format_single_value(value, enum_map)
        }
    };

    // commentedNever': Never comment this out
    // commentedAlways': Always comment this out, even if it's on commandline
    // commentedOptional': Comment out unless it's on commandline
    #[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    enum Commented {
        Never,
        Always,
        Optional,
    }
    let emit_option = |result: &mut Vec<String>,
                       all_set_options: &mut Vec<String>,
                       setting: &str,
                       default_value: Value,
                       commented: Commented| {
        let existing_option_index = all_set_options.iter().position(|option| option == setting);
        if let Some(existing_option_index) = existing_option_index {
            all_set_options.remove(existing_option_index);
        }

        let comment = match commented {
            Commented::Always => true,
            Commented::Never => false,
            Commented::Optional => !options.has(&setting.to_owned()),
        };

        let value = options
            .get(&setting.to_owned())
            .cloned()
            .unwrap_or(default_value);

        if comment {
            result.push(format!(
                "{TAB}{TAB}// \"{setting}\": {},",
                format_value_or_array(setting, value)
            ));
        } else {
            result.push(format!(
                "{TAB}{TAB}\"{setting}\": {},",
                format_value_or_array(setting, value)
            ));
        }
    };

    push(&mut result, vec!["{".to_owned()]);
    push(
        &mut result,
        vec![format!(
            "{TAB}// {}",
            diagnostics::Visit_https_Colon_Slash_Slashaka_ms_Slashtsconfig_to_read_more_about_this_file.localize(locale.clone(), vec![])
        )],
    );
    push(&mut result, vec![format!("{TAB}\"compilerOptions\": {{")]);

    emit_header(&mut result, &locale, &diagnostics::File_Layout);
    emit_option(
        &mut result,
        &mut all_set_options,
        "rootDir",
        "./src".into(),
        Commented::Optional,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "outDir",
        "./dist".into(),
        Commented::Optional,
    );

    newline(&mut result);

    emit_header(&mut result, &locale, &diagnostics::Environment_Settings);
    emit_header(
        &mut result,
        &locale,
        &diagnostics::See_also_https_Colon_Slash_Slashaka_ms_Slashtsconfig_Slashmodule,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "module",
        Value::String(core::MODULE_KIND_NODE_NEXT.to_string()),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "target",
        Value::String(core::SCRIPT_TARGET_ES_NEXT.to_string()),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "types",
        Value::Array(Vec::new()),
        Commented::Never,
    );
    if let Some(lib) = options.get(&"lib".to_owned()) {
        emit_option(
            &mut result,
            &mut all_set_options,
            "lib",
            lib.clone(),
            Commented::Never,
        );
    }
    emit_header(&mut result, &locale, &diagnostics::For_nodejs_Colon);
    push(
        &mut result,
        vec![format!("{TAB}{TAB}// \"lib\": [\"esnext\"],")],
    );
    push(
        &mut result,
        vec![format!("{TAB}{TAB}// \"types\": [\"node\"],")],
    );
    emit_header(
        &mut result,
        &locale,
        &diagnostics::X_and_npm_install_D_types_Slashnode,
    );

    newline(&mut result);

    emit_header(&mut result, &locale, &diagnostics::Other_Outputs);
    emit_option(
        &mut result,
        &mut all_set_options,
        "sourceMap",
        true.into(),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "declaration",
        true.into(),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "declarationMap",
        true.into(),
        Commented::Never,
    );

    newline(&mut result);

    emit_header(
        &mut result,
        &locale,
        &diagnostics::Stricter_Typechecking_Options,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "noUncheckedIndexedAccess",
        true.into(),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "exactOptionalPropertyTypes",
        true.into(),
        Commented::Never,
    );

    newline(&mut result);

    emit_header(&mut result, &locale, &diagnostics::Style_Options);
    emit_option(
        &mut result,
        &mut all_set_options,
        "noImplicitReturns",
        true.into(),
        Commented::Optional,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "noImplicitOverride",
        true.into(),
        Commented::Optional,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "noUnusedLocals",
        true.into(),
        Commented::Optional,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "noUnusedParameters",
        true.into(),
        Commented::Optional,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "noFallthroughCasesInSwitch",
        true.into(),
        Commented::Optional,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "noPropertyAccessFromIndexSignature",
        true.into(),
        Commented::Optional,
    );

    newline(&mut result);

    emit_header(&mut result, &locale, &diagnostics::Recommended_Options);
    emit_option(
        &mut result,
        &mut all_set_options,
        "strict",
        true.into(),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "jsx",
        Value::String("ReactJSX".to_owned()),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "verbatimModuleSyntax",
        true.into(),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "isolatedModules",
        true.into(),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "noUncheckedSideEffectImports",
        true.into(),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "moduleDetection",
        Value::String("Force".to_owned()),
        Commented::Never,
    );
    emit_option(
        &mut result,
        &mut all_set_options,
        "skipLibCheck",
        true.into(),
        Commented::Never,
    );

    // Write any user-provided options we haven't already
    if !all_set_options.is_empty() {
        newline(&mut result);
        while !all_set_options.is_empty() {
            let setting = all_set_options[0].clone();
            let value = options.get(&setting).cloned().unwrap_or(Value::Null);
            emit_option(
                &mut result,
                &mut all_set_options,
                &setting,
                value,
                Commented::Never,
            );
        }
    }

    push(&mut result, vec![format!("{TAB}}}")]);
    push(&mut result, vec!["}".to_owned()]);
    push(&mut result, vec![String::new()]);

    result.join("\n")
}
