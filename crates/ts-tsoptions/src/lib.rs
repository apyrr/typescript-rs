#![forbid(unsafe_code)]
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

pub mod commandlineoption;
pub mod commandlineparser;
pub mod declsbuild;
pub mod declscompiler;
pub mod diagnostics;
pub mod enummaps;
pub mod errors;
pub mod namemap;
pub mod parsedbuildcommandline;
pub mod parsedcommandline;
pub mod parsinghelpers;
pub mod showconfig;
pub mod tsconfigparsing;
pub mod tsoptionstest;
pub mod wildcarddirectories;

pub use commandlineoption as command_line_option;
pub use declsbuild::{build_opts, common_options_with_build, options_for_build};
pub use declscompiler::{default_true_option, options_declaration_for, options_declarations};
pub use diagnostics::{
    AlternateModeDiagnostics, DidYouMeanOptionsDiagnostics, ParseCommandLineWorkerDiagnostics,
    build_options_did_you_mean_diagnostics, get_parse_command_line_worker_diagnostics,
    watch_options_did_you_mean_diagnostics,
};
pub use enummaps::{
    EnumMap, enum_keys, fallback_enum_map, get_default_lib_file_name, get_lib_file_name,
    jsx_option_map, module_detection_option_map, module_option_map, module_resolution_option_map,
    new_line_option_map, target_option_map, target_to_lib_map, watch_directory_enum_map,
    watch_file_enum_map,
};
pub use errors::{
    create_diagnostic_for_node_in_source_file,
    create_diagnostic_for_node_in_source_file_or_compiler_diagnostic,
};
pub use namemap::{
    NameMap, build_name_map, compiler_name_map, get_name_map_from_list, watch_name_map,
};
pub use parsedbuildcommandline::ParsedBuildCommandLine;
pub use parsedcommandline::{ParsedCommandLine, SourceOutputAndProjectReference};
pub use parsinghelpers::{
    CompilerOptionsParseMode, convert_json_option, get_option_name, has_property,
    is_option_value_empty, parse_json_to_string_key, parse_number, parse_string,
    parse_string_array, parse_string_map, parse_tristate, validate_json_option_value,
};
pub use showconfig::convert_to_tsconfig;
pub use tsconfigparsing::{
    ConfigFile, ConfigFileSpecs, ExtendedConfigCache, ExtendedConfigCacheEntry, ExtendsResult,
    FileExtensionInfo, ParseConfigHost, ParseJsonSourceFileConfigFileContentInput, ParsedTsconfig,
    TsConfigSourceFile, create_diagnostic_at_reference_syntax, for_each_property_assignment,
    for_each_ts_config_prop_array, get_callback_for_finding_property_assignment_by_value,
    get_extends_configs_path_or_array, get_options_syntax_by_array_element_value,
    get_parsed_command_line_of_config_file, get_parsed_command_line_of_config_file_path,
    get_supported_extensions, get_supported_extensions_with_json_if_resolve_json_module,
    get_ts_config_prop_array_element_value, get_tsconfig_options_object,
    invalid_dot_dot_after_recursive_wildcard, invalid_trailing_recursion,
    new_tsconfig_source_file_from_file_path, parse_config, parse_config_file_text_to_json,
    parse_extended_config, parse_json_config_file_content,
    parse_json_source_file_config_file_content, tsconfig_to_source_file,
};
pub use wildcarddirectories::{WildcardDirectories, contains_wildcard, get_wildcard_directories};

pub type CommandLineOptionNameMap = BTreeMap<String, CommandLineOption>;

pub static COMMAND_LINE_COMPILER_OPTIONS_MAP: LazyLock<NameMap> =
    LazyLock::new(|| get_name_map_from_list(options_declarations()));

pub fn parse_command_line<T: ParseConfigHost>(args: &[String], host: T) -> ParsedCommandLine {
    let diagnostics = get_parse_command_line_worker_diagnostics(
        options_declarations(),
        build_name_map(&build_opts()),
    );
    let mut parsed = commandlineparser::parse_command_line_worker(diagnostics.did_you_mean, args);
    let watch_options = options_for_watch();
    parsed.raw = Some(command_line_options_to_raw_json(
        &parsed.options,
        &parsed.explicit_null_options,
        options_declarations(),
        &parsed.watch_options,
        &parsed.explicit_null_watch_options,
        &watch_options,
    ));
    let current_directory = host.get_current_directory();
    let options_map = COMMAND_LINE_COMPILER_OPTIONS_MAP.clone();
    for (name, value) in parsed.options.clone() {
        if parsed.explicit_null_options.contains(&name) {
            continue;
        }
        let (converted, should_update) = convert_option_to_absolute_path(
            &name,
            serde_json::Value::String(value),
            &options_map,
            &current_directory,
        );
        if should_update && let serde_json::Value::String(value) = converted {
            parsed.options.insert(name, value);
        }
    }
    parsed.current_directory = current_directory;
    parsed.use_case_sensitive_file_names = host.fs().use_case_sensitive_file_names();
    parsed
}

fn command_line_options_to_raw_json(
    options: &BTreeMap<String, String>,
    explicit_null_options: &BTreeSet<String>,
    option_declarations: &[CommandLineOption],
    watch_options: &BTreeMap<String, String>,
    explicit_null_watch_options: &BTreeSet<String>,
    watch_option_declarations: &[CommandLineOption],
) -> String {
    let mut raw = serde_json::Map::new();
    insert_command_line_raw_options(
        &mut raw,
        options,
        explicit_null_options,
        option_declarations,
        true,
    );
    insert_command_line_raw_options(
        &mut raw,
        watch_options,
        explicit_null_watch_options,
        watch_option_declarations,
        false,
    );
    serde_json::Value::Object(raw).to_string()
}

fn insert_command_line_raw_options(
    raw: &mut serde_json::Map<String, serde_json::Value>,
    options: &BTreeMap<String, String>,
    explicit_null_options: &BTreeSet<String>,
    option_declarations: &[CommandLineOption],
    convert_compiler_options: bool,
) {
    let enum_maps = command_line_option_enum_map();
    for (name, value) in options {
        let value = if explicit_null_options.contains(name) {
            serde_json::Value::Null
        } else {
            let kind = option_declarations
                .iter()
                .find(|option| option.name == *name)
                .and_then(|option| option.kind);
            if convert_compiler_options
                && let Some(kind) = kind
                && let Some(value) = parsedcommandline::compiler_option_json_value(
                    name,
                    value,
                    kind,
                    enum_maps.get(name),
                )
            {
                value
            } else {
                match kind {
                    Some(CommandLineOptionKind::Boolean) => {
                        serde_json::Value::Bool(value == "true")
                    }
                    Some(CommandLineOptionKind::Number) => value
                        .parse::<i64>()
                        .map(serde_json::Number::from)
                        .map(serde_json::Value::Number)
                        .unwrap_or_else(|_| serde_json::Value::String(value.clone())),
                    Some(CommandLineOptionKind::List)
                    | Some(CommandLineOptionKind::ListOrElement) => {
                        let value = value.trim();
                        serde_json::Value::Array(if value.is_empty() {
                            Vec::new()
                        } else {
                            value
                                .split(',')
                                .map(|item| serde_json::Value::String(item.to_owned()))
                                .collect()
                        })
                    }
                    _ => serde_json::Value::String(value.clone()),
                }
            }
        };
        raw.insert(name.clone(), value);
    }
}

pub fn parse_build_command_line<T: ParseConfigHost>(
    args: &[String],
    host: T,
) -> ParsedBuildCommandLine {
    parsedbuildcommandline::parse_build_command_line(
        args,
        host.get_current_directory(),
        host.fs().use_case_sensitive_file_names(),
    )
}

pub fn convert_to_ts_config(
    parsed: &ParsedCommandLine,
    _config_file_name: &str,
) -> showconfig::TSConfig {
    convert_to_tsconfig(parsed, &[])
}

pub fn parse_compiler_options(
    option: &str,
    value: serde_json::Value,
    options: &mut ts_core::CompilerOptions,
) -> bool {
    let Ok(mut object) = serde_json::to_value(&*options) else {
        return false;
    };
    let serde_json::Value::Object(ref mut map) = object else {
        return false;
    };
    map.insert(option.to_owned(), value);
    match serde_json::from_value(object) {
        Ok(parsed) => {
            *options = parsed;
            true
        }
        Err(_) => false,
    }
}

pub fn convert_option_to_absolute_path(
    option: &str,
    value: serde_json::Value,
    option_map: &NameMap,
    base_path: &str,
) -> (serde_json::Value, bool) {
    let Some(option_decl) = option_map.get_option_declaration_from_name(option, false) else {
        return (value, false);
    };
    if !option_decl.is_file_path {
        return (value, false);
    }
    match value {
        serde_json::Value::String(path) if !path.is_empty() => (
            serde_json::Value::String(ts_tspath::get_normalized_absolute_path(&path, base_path)),
            true,
        ),
        other => (other, false),
    }
}

pub fn compiler_options_affect_semantic_diagnostics<A, B>(old: A, new: B) -> bool
where
    A: Borrow<ts_core::CompilerOptions>,
    B: Borrow<ts_core::CompilerOptions>,
{
    compiler_options_changed_for(old.borrow(), new.borrow(), |option| {
        option.affects_semantic_diagnostics
    })
}

pub fn compiler_options_affect_declaration_path<A, B>(old: A, new: B) -> bool
where
    A: Borrow<ts_core::CompilerOptions>,
    B: Borrow<ts_core::CompilerOptions>,
{
    compiler_options_changed_for(old.borrow(), new.borrow(), |option| {
        option.affects_declaration_path
    })
}

pub fn compiler_options_affect_emit<A, B>(old: A, new: B) -> bool
where
    A: Borrow<ts_core::CompilerOptions>,
    B: Borrow<ts_core::CompilerOptions>,
{
    compiler_options_changed_for(old.borrow(), new.borrow(), |option| option.affects_emit)
}

fn compiler_options_changed_for(
    old: &ts_core::CompilerOptions,
    new: &ts_core::CompilerOptions,
    affects: impl Fn(&CommandLineOption) -> bool,
) -> bool {
    let old_value = serde_json::to_value(old).unwrap_or_default();
    let new_value = serde_json::to_value(new).unwrap_or_default();
    options_declarations()
        .iter()
        .filter(|option| affects(option))
        .any(|option| {
            if option.strict_flag {
                return old
                    .get_strict_option_value(tristate_option_value(&old_value, &option.name))
                    != new
                        .get_strict_option_value(tristate_option_value(&new_value, &option.name));
            }
            if option.allow_js_flag {
                return old.get_allow_js() != new.get_allow_js();
            }
            old_value.get(&option.name) != new_value.get(&option.name)
        })
}

fn tristate_option_value(options: &serde_json::Value, name: &str) -> ts_core::Tristate {
    match options.get(name).and_then(serde_json::Value::as_bool) {
        Some(true) => ts_core::TSTrue,
        Some(false) => ts_core::TSFalse,
        None => ts_core::TSUnknown,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandLineOptionKind {
    String,
    Number,
    Boolean,
    Object,
    List,
    ListOrElement,
    Enum, // map
}

impl CommandLineOptionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CommandLineOptionKind::String => "string",
            CommandLineOptionKind::Number => "number",
            CommandLineOptionKind::Boolean => "boolean",
            CommandLineOptionKind::Object => "object",
            CommandLineOptionKind::List => "list",
            CommandLineOptionKind::ListOrElement => "listOrElement",
            CommandLineOptionKind::Enum => "enum",
        }
    }
}

pub const COMMAND_LINE_OPTION_TYPE_STRING: CommandLineOptionKind = CommandLineOptionKind::String;
pub const COMMAND_LINE_OPTION_TYPE_NUMBER: CommandLineOptionKind = CommandLineOptionKind::Number;
pub const COMMAND_LINE_OPTION_TYPE_BOOLEAN: CommandLineOptionKind = CommandLineOptionKind::Boolean;
pub const COMMAND_LINE_OPTION_TYPE_OBJECT: CommandLineOptionKind = CommandLineOptionKind::Object;
pub const COMMAND_LINE_OPTION_TYPE_LIST: CommandLineOptionKind = CommandLineOptionKind::List;
pub const COMMAND_LINE_OPTION_TYPE_LIST_OR_ELEMENT: CommandLineOptionKind =
    CommandLineOptionKind::ListOrElement;
pub const COMMAND_LINE_OPTION_TYPE_ENUM: CommandLineOptionKind = CommandLineOptionKind::Enum;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefaultValueDescription {
    Bool(bool),
    String(String),
    Number(i32),
    Unknown,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandLineOption {
    pub name: String,
    pub short_name: String,
    pub kind: Option<CommandLineOptionKind>,

    // used in parsing
    pub is_file_path: bool,
    pub is_tsconfig_only: bool,
    pub is_command_line_only: bool,

    // used in output
    pub description: Option<String>,
    pub default_value_description: Option<DefaultValueDescription>,
    pub show_in_simplified_help_view: bool,

    // used in output in serializing and generate tsconfig
    pub category: Option<String>,

    // What kind of extra validation `validateJsonOptionValue` should do
    pub extra_validation: ExtraValidation,

    // checks that option with number type has value >= minValue
    pub min_value: i32,

    // true or undefined
    // used for configDirTemplateSubstitutionOptions
    pub allow_config_dir_template_substitution: bool,

    // used for filter in compilerrunner
    pub affects_declaration_path: bool,
    pub affects_program_structure: bool,
    pub affects_semantic_diagnostics: bool,
    pub affects_build_info: bool,
    pub affects_bind_diagnostics: bool,
    pub affects_source_file: bool,
    pub affects_module_resolution: bool,
    pub affects_emit: bool,

    pub allow_js_flag: bool,
    pub strict_flag: bool,

    // used in transpileoptions worker
    pub transpile_option_value: Tristate,

    // used for CommandLineOptionTypeList
    pub list_preserve_falsy_values: bool,
    // used for compilerOptionsDeclaration
    pub element_options: Option<CommandLineOptionNameMap>,
}

impl CommandLineOption {
    pub fn new(name: impl Into<String>, kind: CommandLineOptionKind) -> Self {
        Self {
            name: name.into(),
            kind: Some(kind),
            ..Self::default()
        }
    }

    pub fn deprecated_keys(&self) -> Option<BTreeSet<String>> {
        if self.kind != Some(CommandLineOptionKind::Enum) {
            return None;
        }
        command_line_option_deprecated().get(&self.name).cloned()
    }

    pub fn enum_map(&self) -> Option<&'static enummaps::EnumMap> {
        if self.kind != Some(CommandLineOptionKind::Enum) {
            return None;
        }
        command_line_option_enum_map().get(&self.name)
    }

    pub fn elements(&self) -> Option<CommandLineOption> {
        if self.kind != Some(CommandLineOptionKind::List)
            && self.kind != Some(CommandLineOptionKind::ListOrElement)
        {
            return None;
        }
        command_line_option_elements().get(&self.name).cloned()
    }

    pub fn disallow_null_or_undefined(&self) -> bool {
        self.name == "extends"
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExtraValidation {
    #[default]
    None,
    Spec,
    Locale,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tristate {
    #[default]
    Unknown,
    False,
    True,
}

// CommandLineOption.Elements()
pub fn command_line_option_elements() -> BTreeMap<String, CommandLineOption> {
    let mut result = BTreeMap::new();
    result.insert(
        "lib".to_owned(),
        CommandLineOption {
            name: "lib".to_owned(),
            kind: Some(CommandLineOptionKind::Enum), // libMap,
            default_value_description: Some(DefaultValueDescription::Unknown),
            ..CommandLineOption::default()
        },
    );
    result.insert(
        "rootDirs".to_owned(),
        CommandLineOption {
            name: "rootDirs".to_owned(),
            kind: Some(CommandLineOptionKind::String),
            is_file_path: true,
            ..CommandLineOption::default()
        },
    );
    result.insert(
        "typeRoots".to_owned(),
        CommandLineOption {
            name: "typeRoots".to_owned(),
            kind: Some(CommandLineOptionKind::String),
            is_file_path: true,
            ..CommandLineOption::default()
        },
    );
    for (key, name, kind) in [
        ("types", "types", CommandLineOptionKind::String),
        (
            "moduleSuffixes",
            "moduleSuffixes",
            CommandLineOptionKind::String,
        ),
        (
            "customConditions",
            "condition",
            CommandLineOptionKind::String,
        ),
        ("plugins", "plugin", CommandLineOptionKind::Object),
        // For tsconfig root options
        ("references", "references", CommandLineOptionKind::Object),
        ("files", "files", CommandLineOptionKind::String),
        ("include", "include", CommandLineOptionKind::String),
        ("exclude", "exclude", CommandLineOptionKind::String),
        ("extends", "extends", CommandLineOptionKind::String),
    ] {
        result.insert(key.to_owned(), CommandLineOption::new(name, kind));
    }
    result.insert(
        "excludeDirectories".to_owned(),
        CommandLineOption {
            name: "excludeDirectory".to_owned(),
            kind: Some(CommandLineOptionKind::String),
            is_file_path: true,
            extra_validation: ExtraValidation::Spec,
            ..CommandLineOption::default()
        },
    );
    result.insert(
        "excludeFiles".to_owned(),
        CommandLineOption {
            name: "excludeFile".to_owned(),
            kind: Some(CommandLineOptionKind::String),
            is_file_path: true,
            extra_validation: ExtraValidation::Spec,
            ..CommandLineOption::default()
        },
    );
    // Test infra options
    result.insert(
        "libFiles".to_owned(),
        CommandLineOption::new("libFiles", CommandLineOptionKind::String),
    );
    result
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilerOptionsValue {
    Bool(bool),
    String(String),
    Number(i32),
    Unknown,
}

// CommandLineOption.EnumMap()
static COMMAND_LINE_OPTION_ENUM_MAP: LazyLock<BTreeMap<String, enummaps::EnumMap>> =
    LazyLock::new(|| {
        let lib_map = enummaps::lib_map()
            .iter()
            .map(|(key, value)| (key.clone(), CompilerOptionsValue::String(value.clone())))
            .collect();
        [
            ("lib", lib_map),
            (
                "moduleResolution",
                enummaps::module_resolution_option_map().clone(),
            ),
            ("module", enummaps::module_option_map().clone()),
            ("target", enummaps::target_option_map().clone()),
            (
                "moduleDetection",
                enummaps::module_detection_option_map().clone(),
            ),
            ("jsx", enummaps::jsx_option_map().clone()),
            ("newLine", enummaps::new_line_option_map().clone()),
            ("watchFile", enummaps::watch_file_enum_map().clone()),
            (
                "watchDirectory",
                enummaps::watch_directory_enum_map().clone(),
            ),
            ("fallbackPolling", enummaps::fallback_enum_map().clone()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
    });

pub fn command_line_option_enum_map() -> &'static BTreeMap<String, enummaps::EnumMap> {
    &COMMAND_LINE_OPTION_ENUM_MAP
}

// CommandLineOption.DeprecatedKeys()
pub fn command_line_option_deprecated() -> BTreeMap<String, BTreeSet<String>> {
    [
        ("module", ["none", "amd", "system", "umd"].as_slice()),
        ("moduleResolution", ["node", "classic", "node10"].as_slice()),
        ("target", ["es5"].as_slice()),
    ]
    .into_iter()
    .map(|(key, values)| {
        (
            key.to_owned(),
            values.iter().map(|value| (*value).to_owned()).collect(),
        )
    })
    .collect()
}

pub fn command_line_options_to_map(options: &[CommandLineOption]) -> CommandLineOptionNameMap {
    let mut result = BTreeMap::new();
    for option in options {
        result.insert(option.name.clone(), option.clone());
        result.insert(option.name.to_lowercase(), option.clone());
    }
    result
}

pub fn lib_map() -> &'static BTreeMap<String, String> {
    enummaps::lib_map()
}

pub fn libs() -> Vec<String> {
    enummaps::lib_names()
}

pub static LIBS: LazyLock<Vec<String>> = LazyLock::new(libs);

pub fn lib_files_set() -> BTreeSet<String> {
    lib_map().values().cloned().collect()
}

pub fn type_acquisition_declaration() -> CommandLineOption {
    CommandLineOption {
        name: "typeAcquisition".to_owned(),
        kind: Some(CommandLineOptionKind::Object),
        element_options: Some(command_line_options_to_map(&type_acquisition_decls())),
        ..CommandLineOption::default()
    }
}

// Do not delete this without updating the website's tsconfig generation.
pub fn type_acquisition_decls() -> Vec<CommandLineOption> {
    vec![
        CommandLineOption {
            name: "enable".to_owned(),
            kind: Some(CommandLineOptionKind::Boolean),
            default_value_description: Some(DefaultValueDescription::Bool(false)),
            ..CommandLineOption::default()
        },
        CommandLineOption::new("include", CommandLineOptionKind::List),
        CommandLineOption::new("exclude", CommandLineOptionKind::List),
        CommandLineOption {
            name: "disableFilenameBasedTypeAcquisition".to_owned(),
            kind: Some(CommandLineOptionKind::Boolean),
            default_value_description: Some(DefaultValueDescription::Bool(false)),
            ..CommandLineOption::default()
        },
    ]
}

pub fn options_for_watch() -> Vec<CommandLineOption> {
    vec![
        CommandLineOption {
            name: "watchInterval".to_owned(),
            kind: Some(CommandLineOptionKind::Number),
            category: Some("Watch and Build Modes".to_owned()),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "watchFile".to_owned(),
            kind: Some(CommandLineOptionKind::Enum),
            // new Map(Object.entries({
            //     fixedpollinginterval: WatchFileKind.FixedPollingInterval,
            //     prioritypollinginterval: WatchFileKind.PriorityPollingInterval,
            //     dynamicprioritypolling: WatchFileKind.DynamicPriorityPolling,
            //     fixedchunksizepolling: WatchFileKind.FixedChunkSizePolling,
            //     usefsevents: WatchFileKind.UseFsEvents,
            //     usefseventsonparentdirectory: WatchFileKind.UseFsEventsOnParentDirectory,
            // })),
            category: Some("Watch and Build Modes".to_owned()),
            description: Some("Specify how the TypeScript watch mode works.".to_owned()),
            default_value_description: Some(DefaultValueDescription::String(
                "UseFsEvents".to_owned(),
            )),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "watchDirectory".to_owned(),
            kind: Some(CommandLineOptionKind::Enum),
            // new Map(Object.entries({
            //     usefsevents: WatchDirectoryKind.UseFsEvents,
            //     fixedpollinginterval: WatchDirectoryKind.FixedPollingInterval,
            //     dynamicprioritypolling: WatchDirectoryKind.DynamicPriorityPolling,
            //     fixedchunksizepolling: WatchDirectoryKind.FixedChunkSizePolling,
            // })),
            category: Some("Watch and Build Modes".to_owned()),
            description: Some(
                "Specify how directories are watched on systems that lack recursive file-watching functionality."
                    .to_owned(),
            ),
            default_value_description: Some(DefaultValueDescription::String(
                "UseFsEvents".to_owned(),
            )),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "fallbackPolling".to_owned(),
            kind: Some(CommandLineOptionKind::Enum),
            // new Map(Object.entries({
            //     fixedinterval: PollingWatchKind.FixedInterval,
            //     priorityinterval: PollingWatchKind.PriorityInterval,
            //     dynamicpriority: PollingWatchKind.DynamicPriority,
            //     fixedchunksize: PollingWatchKind.FixedChunkSize,
            // })),
            category: Some("Watch and Build Modes".to_owned()),
            description: Some(
                "Specify what approach the watcher should use if the system runs out of native file watchers."
                    .to_owned(),
            ),
            default_value_description: Some(DefaultValueDescription::String(
                "PriorityInterval".to_owned(),
            )),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "synchronousWatchDirectory".to_owned(),
            kind: Some(CommandLineOptionKind::Boolean),
            category: Some("Watch and Build Modes".to_owned()),
            description: Some(
                "Synchronously call callbacks and update the state of directory watchers on platforms that don`t support recursive watching natively."
                    .to_owned(),
            ),
            default_value_description: Some(DefaultValueDescription::Bool(false)),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "excludeDirectories".to_owned(),
            kind: Some(CommandLineOptionKind::List),
            // element: {
            //     Name: "excludeDirectory",
            //     Kind: "string",
            //     isFilePath: true,
            //     extraValidation: specToDiagnostic,
            // },
            allow_config_dir_template_substitution: true,
            category: Some("Watch and Build Modes".to_owned()),
            description: Some("Remove a list of directories from the watch process.".to_owned()),
            ..CommandLineOption::default()
        },
        CommandLineOption {
            name: "excludeFiles".to_owned(),
            kind: Some(CommandLineOptionKind::List),
            // element: {
            //     Name: "excludeFile",
            //     Kind: "string",
            //     isFilePath: true,
            //     extraValidation: specToDiagnostic,
            // },
            allow_config_dir_template_substitution: true,
            category: Some("Watch and Build Modes".to_owned()),
            description: Some(
                "Remove a list of files from the watch mode's processing.".to_owned(),
            ),
            ..CommandLineOption::default()
        },
    ]
}

pub fn new_parsed_command_line(
    compiler_options: ts_core::CompilerOptions,
    file_names: Vec<String>,
    compare_paths_options: ts_tspath::ComparePathsOptions,
) -> ParsedCommandLine {
    let mut command_line = ParsedCommandLine {
        file_names,
        current_directory: compare_paths_options.current_directory,
        use_case_sensitive_file_names: compare_paths_options.use_case_sensitive_file_names,
        ..Default::default()
    };
    command_line.set_compiler_options(compiler_options);
    command_line
}
