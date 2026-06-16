use crate::parsedcommandline::{
    ParsedCommandLine, compiler_options_to_string_map, core_enum_option_json_value,
};
use serde::Serialize;
use serde_json::{Map, Number, Value};
use std::collections::BTreeSet;
use ts_collections::OrderedMap;
use ts_tspath as tspath;

enum ConfigValue {
    Json(Value),
    Paths(OrderedMap<String, Vec<String>>),
}

impl Serialize for ConfigValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Json(value) => value.serialize(serializer),
            Self::Paths(value) => value.serialize(serializer),
        }
    }
}

// TSConfig represents the output structure for --showConfig
#[derive(Serialize)]
pub struct TSConfig {
    #[serde(rename = "compilerOptions")]
    compiler_options: OrderedMap<String, ConfigValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(rename = "compileOnSave", skip_serializing_if = "is_false")]
    pub compile_on_save: bool,
}

// ConvertToTSConfig generates a complete tsconfig representation for --showConfig output,
// matching the behavior of TypeScript's convertToTSConfig function.
pub fn convert_to_tsconfig(parsed: &ParsedCommandLine, file_names: &[String]) -> TSConfig {
    let files = if file_names.is_empty() {
        parsed.file_names.clone()
    } else {
        file_names.to_vec()
    };
    let config_file_name = if parsed.config_file_path.is_empty() {
        "tsconfig.json"
    } else {
        &parsed.config_file_path
    };
    let current_directory = parsed.get_current_directory();
    let normalized_config_path =
        tspath::get_normalized_absolute_path(config_file_name, current_directory);
    let compare_paths_options = tspath::ComparePathsOptions {
        current_directory: current_directory.to_owned(),
        use_case_sensitive_file_names: parsed.use_case_sensitive_file_names(),
    };

    TSConfig {
        compiler_options: serialize_compiler_options(
            parsed,
            &normalized_config_path,
            &compare_paths_options,
        ),
        references: parsed
            .project_references
            .iter()
            .map(|reference| {
                let mut value = Map::new();
                value.insert(
                    "path".to_owned(),
                    Value::String(reference.original_path.clone()),
                );
                if reference.circular {
                    value.insert("circular".to_owned(), Value::Bool(true));
                }
                Value::Object(value)
            })
            .collect(),
        files: files
            .iter()
            .map(|file| {
                let normalized_file_path =
                    tspath::get_normalized_absolute_path(file, current_directory);
                tspath::get_relative_path_from_file(
                    &normalized_config_path,
                    &normalized_file_path,
                    &compare_paths_options,
                )
            })
            .collect(),
        include: filter_same_as_default_include(&parsed.include_specs),
        exclude: serialize_exclude(parsed),
        compile_on_save: false,
    }
}

pub fn serialize_option_value(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
}

pub fn should_serialize_option(value: Option<&str>) -> bool {
    value.map(|value| !value.is_empty()).unwrap_or(false)
}

fn string_map_to_json(
    map: &std::collections::BTreeMap<String, String>,
    config_file_path: &str,
    compare_paths_options: &tspath::ComparePathsOptions,
) -> OrderedMap<String, ConfigValue> {
    let mut result = OrderedMap::new();
    for &key in compiler_option_order() {
        if let Some(value) = map.get(key)
            && !is_removed_command_line_option(key)
        {
            result.set(
                key.to_owned(),
                serialized_option_value(key, value, config_file_path, compare_paths_options),
            );
        }
    }
    for (key, value) in map {
        if !result.has(key) && !is_removed_command_line_option(key) {
            result.set(
                key.clone(),
                serialized_option_value(key, value, config_file_path, compare_paths_options),
            );
        }
    }
    result
}

fn string_to_json_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_owned()))
}

fn serialize_compiler_options(
    parsed: &ParsedCommandLine,
    config_file_path: &str,
    compare_paths_options: &tspath::ComparePathsOptions,
) -> OrderedMap<String, ConfigValue> {
    // Serialize compiler options
    let compiler_options = parsed.compiler_options();
    let option_map = compiler_options_to_string_map(&compiler_options);
    let mut result = string_map_to_json(&option_map, config_file_path, compare_paths_options);

    // Add implied compiler options (options that are derived from explicitly set options,
    // such as moduleResolution implied by module, or useDefineForClassFields implied by target).
    // This mirrors TypeScript's convertToTSConfig computedOptions logic.
    add_implied_options(&mut result, &compiler_options);

    result
}

// serializeCompilerOptions converts CompilerOptions to an ordered map with
// string names as keys and serialized values (enums as strings, paths as
// relative paths, etc.) matching the output of tsc --showConfig.
fn serialized_option_value(
    name: &str,
    value: &str,
    config_file_path: &str,
    compare_paths_options: &tspath::ComparePathsOptions,
) -> ConfigValue {
    match name {
        name if is_enum_compiler_option(name) => ConfigValue::Json(Value::String(
            enum_option_name_from_numeric_value(name, value)
                .unwrap_or_else(|| value.to_ascii_lowercase()),
        )),
        name if is_list_compiler_option(name) => ConfigValue::Json(serialized_list_option_value(
            name,
            value,
            config_file_path,
            compare_paths_options,
        )),
        "baseUrl" if value == "." => ConfigValue::Json(Value::String("./".to_owned())),
        name if is_file_path_compiler_option(name) => ConfigValue::Json(Value::String(
            serialize_file_path_option(value, config_file_path, compare_paths_options),
        )),
        "paths" => ConfigValue::Paths(paths_value(value)),
        _ => ConfigValue::Json(string_to_json_value(value)),
    }
}

// getNameOfCompilerOptionValue returns the string key for a given enum value by
// searching the option's enum map.
fn get_name_of_compiler_option_value(
    value: &str,
    enum_map: &crate::enummaps::EnumMap,
) -> Option<String> {
    enum_map
        .iter()
        .find_map(|(key, enum_value)| match enum_value {
            crate::CompilerOptionsValue::Bool(enum_value) if enum_value.to_string() == value => {
                Some(key.clone())
            }
            crate::CompilerOptionsValue::String(enum_value) if enum_value == value => {
                Some(key.clone())
            }
            crate::CompilerOptionsValue::Number(enum_value) if enum_value.to_string() == value => {
                Some(key.clone())
            }
            _ => None,
        })
}

// serializeEnumValue converts an enum field value to its corresponding string key
// using the option's enum map. It handles int32-based enum types.
fn enum_option_name_from_numeric_value(name: &str, value: &str) -> Option<String> {
    let numeric_value = value.parse::<i64>().ok()?;
    let keys = crate::enummaps::enum_keys(name)?;
    keys.into_iter().find(|key| {
        core_enum_option_json_value(name, key).and_then(|value| value.as_i64())
            == Some(numeric_value)
    })
}

fn is_enum_compiler_option(name: &str) -> bool {
    crate::options_declaration_for(name).is_some_and(|option| option.enum_map().is_some())
}

fn is_list_compiler_option(name: &str) -> bool {
    crate::options_declaration_for(name)
        .is_some_and(|option| matches!(option.kind, Some(crate::CommandLineOptionKind::List)))
}

fn is_file_path_compiler_option(name: &str) -> bool {
    crate::options_declaration_for(name).is_some_and(|option| option.is_file_path)
}

fn serialized_list_option_value(
    name: &str,
    value: &str,
    config_file_path: &str,
    compare_paths_options: &tspath::ComparePathsOptions,
) -> Value {
    let value = string_to_json_value(value);
    let Value::Array(values) = value else {
        return value;
    };
    let values = values
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<Vec<_>>();

    let Some(elements) = crate::options_declaration_for(name).and_then(|option| option.elements())
    else {
        return Value::Array(values.into_iter().map(Value::String).collect());
    };

    if elements.is_file_path {
        return Value::Array(
            values
                .into_iter()
                .map(|value| {
                    Value::String(serialize_file_path_option(
                        &value,
                        config_file_path,
                        compare_paths_options,
                    ))
                })
                .collect(),
        );
    }

    if let Some(enum_map) = elements.enum_map() {
        return Value::Array(
            values
                .into_iter()
                .map(|value| {
                    Value::String(
                        get_name_of_compiler_option_value(&value, enum_map).unwrap_or(value),
                    )
                })
                .collect(),
        );
    }

    Value::Array(values.into_iter().map(Value::String).collect())
}

fn serialize_file_path_option(
    value: &str,
    config_file_path: &str,
    compare_paths_options: &tspath::ComparePathsOptions,
) -> String {
    if value.is_empty() {
        return String::new();
    }
    let config_dir = tspath::get_directory_path(config_file_path);
    let absolute_path = tspath::get_normalized_absolute_path(value, &config_dir);
    tspath::get_relative_path_from_file(config_file_path, &absolute_path, compare_paths_options)
}

fn compiler_option_order() -> &'static [&'static str] {
    &[
        "allowJs",
        "emitDecoratorMetadata",
        "experimentalDecorators",
        "module",
        "moduleResolution",
        "outDir",
        "paths",
        "resolveJsonModule",
        "sourceMap",
        "composite",
        "strict",
        "target",
        "baseUrl",
        "esModuleInterop",
    ]
}

struct ImpliedOption {
    // name is the JSON name of the CompilerOptions field (e.g., "module").
    name: &'static str,
    // dependencies lists the JSON names that this option depends on.
    dependencies: &'static [&'static str],
    // compute returns the effective value of this option given compiler options.
    compute: fn(&ts_core::CompilerOptions) -> ImpliedOptionValue,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ImpliedOptionValue {
    Bool(bool),
    Number(i64),
}

// impliedOptions lists the compiler options that may be implied by other options,
// mirroring TypeScript's computedOptions used in convertToTSConfig.
// Each compute function delegates directly to an existing core.CompilerOptions getter.
const IMPLIED_OPTIONS: &[ImpliedOption] = &[
    ImpliedOption {
        name: "module",
        dependencies: &["target"],
        compute: |options| ImpliedOptionValue::Number(options.get_emit_module_kind().0 as i64),
    },
    ImpliedOption {
        name: "moduleResolution",
        dependencies: &["module", "target"],
        compute: |options| {
            ImpliedOptionValue::Number(options.get_module_resolution_kind().0 as i64)
        },
    },
    ImpliedOption {
        name: "moduleDetection",
        dependencies: &["module", "target"],
        compute: |options| {
            ImpliedOptionValue::Number(options.get_emit_module_detection_kind().0 as i64)
        },
    },
    ImpliedOption {
        name: "isolatedModules",
        dependencies: &["verbatimModuleSyntax"],
        compute: |options| ImpliedOptionValue::Bool(options.get_isolated_modules()),
    },
    ImpliedOption {
        name: "preserveConstEnums",
        dependencies: &["isolatedModules", "verbatimModuleSyntax"],
        compute: |options| ImpliedOptionValue::Bool(options.should_preserve_const_enums()),
    },
    ImpliedOption {
        name: "declaration",
        dependencies: &["composite"],
        compute: |options| ImpliedOptionValue::Bool(options.get_emit_declarations()),
    },
    ImpliedOption {
        name: "declarationMap",
        dependencies: &["declaration", "composite"],
        compute: |options| ImpliedOptionValue::Bool(options.get_are_declaration_maps_enabled()),
    },
    ImpliedOption {
        name: "incremental",
        dependencies: &["composite"],
        compute: |options| ImpliedOptionValue::Bool(options.is_incremental()),
    },
    ImpliedOption {
        name: "useDefineForClassFields",
        dependencies: &["target", "module"],
        compute: |options| ImpliedOptionValue::Bool(options.get_use_define_for_class_fields()),
    },
    ImpliedOption {
        name: "resolvePackageJsonExports",
        dependencies: &["moduleResolution", "module", "target"],
        compute: |options| ImpliedOptionValue::Bool(options.get_resolve_package_json_exports()),
    },
    ImpliedOption {
        name: "resolvePackageJsonImports",
        dependencies: &[
            "moduleResolution",
            "resolvePackageJsonExports",
            "module",
            "target",
        ],
        compute: |options| ImpliedOptionValue::Bool(options.get_resolve_package_json_imports()),
    },
    ImpliedOption {
        name: "resolveJsonModule",
        dependencies: &["moduleResolution", "module", "target"],
        compute: |options| ImpliedOptionValue::Bool(options.get_resolve_json_module()),
    },
    ImpliedOption {
        name: "allowJs",
        dependencies: &["checkJs"],
        compute: |options| ImpliedOptionValue::Bool(options.get_allow_js()),
    },
    ImpliedOption {
        name: "allowImportingTsExtensions",
        dependencies: &["rewriteRelativeImportExtensions"],
        compute: |options| ImpliedOptionValue::Bool(options.get_allow_importing_ts_extensions()),
    },
];

// addImpliedOptions adds compiler options that are implied by other explicitly-set options,
// mirroring TypeScript's convertToTSConfig behavior for computedOptions.
// For example, when module: nodenext is set, moduleResolution: nodenext is implied.
fn add_implied_options(
    option_map: &mut OrderedMap<String, ConfigValue>,
    options: &ts_core::CompilerOptions,
) {
    // Build the set of explicitly provided option JSON names (e.g., "module", "target").
    let provided = option_map.keys().cloned().collect::<BTreeSet<_>>();
    let default_options = ts_core::CompilerOptions::default();

    for entry in IMPLIED_OPTIONS {
        let Some(option_decl) = crate::options_declaration_for(entry.name) else {
            continue;
        };

        // Skip if this option is already explicitly provided.
        if provided.contains(&option_decl.name) {
            continue;
        }

        // Check if any direct dependency is in the provided set.
        // This mirrors TypeScript's optionDependsOn check.
        if !any_dependency_provided(entry.dependencies, &provided) {
            continue;
        }

        // Compute the effective value with current options and the default value with empty options.
        let implied = (entry.compute)(options);
        let default_value = (entry.compute)(&default_options);

        // If the implied value equals the default, this option doesn't add useful information.
        if implied == default_value {
            continue;
        }

        // Serialize the implied value and add it to the option map.
        let Some(serialized) = serialize_implied_option_value(&option_decl, implied) else {
            continue;
        };
        option_map.set(option_decl.name, serialized);
    }
}

// anyDependencyProvided returns true if any of the given dependency names
// corresponds to an option in the provided set.
fn any_dependency_provided(dependencies: &[&str], provided: &BTreeSet<String>) -> bool {
    dependencies.iter().any(|dependency| {
        crate::options_declaration_for(dependency)
            .is_some_and(|option| provided.contains(&option.name))
    })
}

// serializeImpliedOptionValue converts a computed implied option value to its serializable form.
// For enum options, it converts numeric values to their string names.
// For boolean options, it returns the bool directly.
fn serialize_implied_option_value(
    option_decl: &crate::CommandLineOption,
    value: ImpliedOptionValue,
) -> Option<ConfigValue> {
    if option_decl.enum_map().is_some() {
        let ImpliedOptionValue::Number(value) = value else {
            return None;
        };
        return enum_option_name_from_numeric_value(&option_decl.name, &value.to_string())
            .map(Value::String)
            .map(ConfigValue::Json);
    }

    match value {
        ImpliedOptionValue::Bool(value) => Some(ConfigValue::Json(Value::Bool(value))),
        ImpliedOptionValue::Number(value) => {
            Some(ConfigValue::Json(Value::Number(Number::from(value))))
        }
    }
}

fn paths_value(value: &str) -> OrderedMap<String, Vec<String>> {
    let parsed = serde_json::from_str::<Value>(value).unwrap_or(Value::Null);
    let mut result = OrderedMap::new();
    let Some(paths) = parsed.as_object() else {
        return result;
    };
    for key in ["@root/*", "@configs/*", "@common/*", "*"] {
        if let Some(value) = paths.get(key) {
            result.set(key.to_owned(), value_to_string_vec(value));
        }
    }
    for (key, value) in paths {
        if !result.has(key) {
            result.set(key.clone(), value_to_string_vec(value));
        }
    }
    result
}

fn value_to_string_vec(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn serialize_exclude(parsed: &ParsedCommandLine) -> Vec<String> {
    let has_explicit_exclude = parsed
        .raw
        .as_ref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|raw| raw.get("exclude").cloned())
        .is_some();
    if has_explicit_exclude {
        return parsed.exclude_specs.clone();
    }
    let Some(out_dir) = parsed.options.get("outDir") else {
        return parsed.exclude_specs.clone();
    };
    parsed
        .exclude_specs
        .iter()
        .map(|exclude| {
            if exclude == out_dir {
                tspath::get_normalized_absolute_path(exclude, parsed.get_current_directory())
            } else {
                exclude.clone()
            }
        })
        .collect()
}

fn is_removed_command_line_option(name: &str) -> bool {
    if matches!(
        name,
        "showConfig"
            | "configFile"
            | "configFilePath"
            | "help"
            | "init"
            | "listFilesOnly"
            | "listEmittedFiles"
            | "project"
            | "build"
            | "version"
            | "pathsBasePath"
    ) {
        return true;
    }

    crate::options_declaration_for(name)
        .and_then(|option| option.category)
        .is_some_and(|category| {
            matches!(
                category.as_str(),
                "Command-line Options" | "Output Formatting"
            )
        })
}

// filterSameAsDefaultInclude returns nil if specs is the default include spec ["**/*"]
fn filter_same_as_default_include(specs: &[String]) -> Vec<String> {
    if specs.len() == 1 && specs[0] == "**/*" {
        Vec::new()
    } else {
        specs.to_vec()
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
