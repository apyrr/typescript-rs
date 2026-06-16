use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use crate::tsconfigparsing::{
    ConfigFileSpecs, FileExtensionInfo, TsConfigSourceFile, get_file_names_from_config_specs,
};
use crate::wildcarddirectories::{WildcardDirectories, get_wildcard_directories};
use crate::{CommandLineOptionKind, CompilerOptionsValue, command_line_option_enum_map};
use serde_json::{Map, Number, Value};
use ts_ast as ast;
use ts_collections::OrderedMap;
use ts_core as core;
use ts_locale as locale;
use ts_outputpaths as outputpaths;
use ts_tspath as tspath;
use ts_vfs as vfs;

const FILE_GLOB_PATTERN: &str = "*.{js,jsx,mjs,cjs,ts,tsx,mts,cts,json}";
const RECURSIVE_FILE_GLOB_PATTERN: &str = "**/*.{js,jsx,mjs,cjs,ts,tsx,mts,cts,json}";

#[derive(Clone, Debug, Default)]
pub struct SourceOutputAndProjectReference {
    pub source: String,
    pub output_dts: String,
    pub resolved: Option<Box<ParsedCommandLine>>,
}

#[derive(Clone, Debug, Default)]
pub struct ParsedCommandLine {
    pub options: BTreeMap<String, String>,
    pub parsed_compiler_options: Option<core::CompilerOptions>,
    pub explicit_null_options: BTreeSet<String>,
    pub watch_options: BTreeMap<String, String>,
    pub explicit_null_watch_options: BTreeSet<String>,
    pub type_acquisition: BTreeMap<String, String>,
    pub file_names: Vec<String>,
    pub project_references: Vec<core::ProjectReference>,
    pub config_file: Option<TsConfigSourceFile>,
    pub errors: Vec<String>,
    pub wildcard_directories: WildcardDirectories,
    pub include_specs: Vec<String>,
    pub exclude_specs: Vec<String>,
    pub extra_file_extensions: Vec<FileExtensionInfo>,
    pub compile_on_save: bool,
    pub raw: Option<String>,
    pub config_file_path: String,
    pub current_directory: String,
    pub use_case_sensitive_file_names: bool,
    pub literal_file_names_len: usize,
    pub source_to_project_reference: BTreeMap<tspath::Path, SourceOutputAndProjectReference>,
    pub output_dts_to_project_reference: BTreeMap<tspath::Path, SourceOutputAndProjectReference>,
    pub common_source_directory: String,
    pub resolved_project_reference_paths: Vec<String>,
}

impl PartialEq for ParsedCommandLine {
    fn eq(&self, other: &Self) -> bool {
        self.options == other.options
            && self.watch_options == other.watch_options
            && self.explicit_null_options == other.explicit_null_options
            && self.explicit_null_watch_options == other.explicit_null_watch_options
            && self.type_acquisition == other.type_acquisition
            && self.file_names == other.file_names
            && self.project_references == other.project_references
            && self.errors == other.errors
            && self.include_specs == other.include_specs
            && self.exclude_specs == other.exclude_specs
            && self.extra_file_extensions == other.extra_file_extensions
            && self.compile_on_save == other.compile_on_save
            && self.raw == other.raw
            && self.config_file_path == other.config_file_path
            && self.current_directory == other.current_directory
            && self.use_case_sensitive_file_names == other.use_case_sensitive_file_names
            && self.literal_file_names_len == other.literal_file_names_len
    }
}

impl ParsedCommandLine {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn get_current_directory(&self) -> &str {
        &self.current_directory
    }

    pub fn use_case_sensitive_file_names(&self) -> bool {
        self.use_case_sensitive_file_names
    }

    pub fn config_name(&self) -> String {
        self.config_file
            .as_ref()
            .map(|config_file| config_file.file_name.clone())
            .unwrap_or_default()
    }

    pub fn compiler_options(&self) -> core::CompilerOptions {
        if let Some(options) = &self.parsed_compiler_options {
            return options.clone();
        }

        let mut json = Map::new();
        let enum_maps = command_line_option_enum_map();

        for (name, value) in &self.options {
            if self.explicit_null_options.contains(name) {
                continue;
            }
            if name == "build" {
                json.insert(name.clone(), Value::Bool(value != "false"));
                continue;
            }
            if name == "allowNonTsExtensions" {
                json.insert(name.clone(), Value::Bool(parse_tristate(value).is_true()));
                continue;
            }
            let Some(option) = crate::declscompiler::options_declaration_for(name) else {
                continue;
            };
            let Some(kind) = option.kind else {
                continue;
            };
            let Some(json_value) =
                compiler_option_json_value(name, value, kind, enum_maps.get(name))
            else {
                continue;
            };
            json.insert(name.clone(), json_value);
        }

        if !self.config_file_path.is_empty() {
            json.insert(
                "configFilePath".to_owned(),
                Value::String(self.config_file_path.clone()),
            );
        }

        let paths_for_validation = paths_for_validation(json.get("paths"));
        if let Some(paths) = json.get_mut("paths") {
            *paths = compiler_options_paths_json_value(paths);
        }

        let mut options = serde_json::from_value::<core::CompilerOptions>(Value::Object(json))
            .expect("compiler options parsed from validated tsconfig values");
        options.paths_for_validation = paths_for_validation;
        options.target_is_es3 = self
            .options
            .get("target")
            .is_some_and(|value| value.eq_ignore_ascii_case("es3"));
        options.type_roots_configured = self.options.contains_key("typeRoots")
            && !self.explicit_null_options.contains("typeRoots");
        if let Some(paths_base_path) = self.options.get("pathsBasePath") {
            if !self.explicit_null_options.contains("pathsBasePath") {
                options.paths_base_path = paths_base_path.clone();
            }
        }
        if options.config_file_path.is_empty() {
            options.config_file_path = self.config_file_path.clone();
        }
        options
    }

    pub fn watch_options(&self) -> core::WatchOptions {
        if self.watch_options.is_empty() {
            return core::WatchOptions::default();
        }
        let watch_options = crate::options_for_watch();
        let watch_name_map = crate::watch_name_map(&watch_options);
        let mut json = Map::new();
        for (key, value) in &self.watch_options {
            if self.explicit_null_watch_options.contains(key) {
                continue;
            }
            let Some(option) = watch_name_map.get(key) else {
                continue;
            };
            let Some(kind) = option.kind else {
                continue;
            };
            let json_value = match kind {
                CommandLineOptionKind::Boolean => Some(Value::Bool(value == "true")),
                CommandLineOptionKind::Number => value
                    .parse::<i64>()
                    .ok()
                    .map(Number::from)
                    .map(Value::Number),
                CommandLineOptionKind::String => Some(Value::String(value.clone())),
                CommandLineOptionKind::List | CommandLineOptionKind::ListOrElement => {
                    let value = value.trim();
                    Some(Value::Array(if value.is_empty() {
                        Vec::new()
                    } else {
                        value
                            .split(',')
                            .filter_map(|item| {
                                let item = item.trim();
                                (!item.is_empty()).then(|| Value::String(item.to_owned()))
                            })
                            .collect()
                    }))
                }
                CommandLineOptionKind::Enum => watch_enum_option_json_value(&option.name, value),
                CommandLineOptionKind::Object => serde_json::from_str(value).ok(),
            };
            if let Some(json_value) = json_value {
                json.insert(option.name.clone(), json_value);
            }
        }
        serde_json::from_value(Value::Object(json)).unwrap_or_default()
    }

    pub fn set_compiler_options(&mut self, mut options: core::CompilerOptions) {
        if options.config_file_path.is_empty() {
            options.config_file_path = self.config_file_path.clone();
        }
        self.options = compiler_options_to_string_map(&options);
        self.parsed_compiler_options = Some(options);
        self.explicit_null_options.clear();
    }

    pub fn set_type_acquisition(&mut self, type_acquisition: core::TypeAcquisition) {
        self.type_acquisition = json_object_to_string_map(&type_acquisition);
    }

    pub fn type_acquisition(&self) -> Option<core::TypeAcquisition> {
        if self.type_acquisition.is_empty() {
            return Some(core::TypeAcquisition::default());
        }

        serde_json::from_value(Value::Object(string_map_to_json_object(
            &self.type_acquisition,
        )))
        .ok()
    }

    pub fn source_to_project_reference(
        &self,
    ) -> &BTreeMap<tspath::Path, SourceOutputAndProjectReference> {
        &self.source_to_project_reference
    }

    pub fn output_dts_to_project_reference(
        &self,
    ) -> &BTreeMap<tspath::Path, SourceOutputAndProjectReference> {
        &self.output_dts_to_project_reference
    }

    pub fn parse_input_output_names(&mut self) {
        if !self.source_to_project_reference.is_empty()
            || !self.output_dts_to_project_reference.is_empty()
        {
            return;
        }

        let mut source_to_output = BTreeMap::new();
        let mut output_dts_to_source = BTreeMap::new();

        for (output_dts, source) in self.get_output_declaration_and_source_file_names() {
            let path = tspath::to_path(
                &source,
                &self.current_directory,
                self.use_case_sensitive_file_names,
            );
            let project_reference = SourceOutputAndProjectReference {
                source: source.clone(),
                output_dts: output_dts.clone(),
                resolved: Some(Box::new(self.clone())),
            };
            if !output_dts.is_empty() {
                output_dts_to_source.insert(
                    tspath::to_path(
                        &output_dts,
                        &self.current_directory,
                        self.use_case_sensitive_file_names,
                    ),
                    project_reference.clone(),
                );
            }
            source_to_output.insert(path, project_reference);
        }

        self.output_dts_to_project_reference = output_dts_to_source;
        self.source_to_project_reference = source_to_output;
    }

    pub fn common_source_directory(&mut self) -> String {
        if !self.common_source_directory.is_empty() {
            return self.common_source_directory.clone();
        }

        let current_directory = self.current_directory.clone();
        let use_case_sensitive_file_names = self.use_case_sensitive_file_names;
        let compare_paths_options = self.compare_paths_options();
        let options = self.compiler_options();
        let root_dir_errors = RefCell::new(Vec::new());
        self.common_source_directory = outputpaths::get_common_source_directory(
            &options,
            || {
                self.file_names
                    .iter()
                    .filter(|file| {
                        !(tspath::is_declaration_file_name(file)
                            || options.no_emit_for_js_files.is_true()
                                && tspath::has_js_file_extension(file))
                    })
                    .cloned()
                    .collect()
            },
            &self.current_directory,
            self.use_case_sensitive_file_names,
            Some(|source_files: Vec<String>, root_directory: &str| {
                check_source_files_belong_to_path(
                    &source_files,
                    root_directory,
                    &current_directory,
                    use_case_sensitive_file_names,
                    &compare_paths_options,
                    &mut root_dir_errors.borrow_mut(),
                )
            }),
        );
        self.errors.extend(root_dir_errors.into_inner());
        self.common_source_directory.clone()
    }

    fn compute_common_source_directory(&self) -> String {
        if !self.common_source_directory.is_empty() {
            return self.common_source_directory.clone();
        }

        let options = self.compiler_options();
        outputpaths::get_common_source_directory(
            &options,
            || {
                self.file_names
                    .iter()
                    .filter(|file| {
                        !(tspath::is_declaration_file_name(file)
                            || options.no_emit_for_js_files.is_true()
                                && tspath::has_js_file_extension(file))
                    })
                    .cloned()
                    .collect()
            },
            &self.current_directory,
            self.use_case_sensitive_file_names,
            None::<fn(Vec<String>, &str) -> bool>,
        )
    }

    pub fn get_output_declaration_and_source_file_names(&self) -> Vec<(String, String)> {
        let options = self.compiler_options();
        self.file_names
            .iter()
            .map(|file_name| {
                let output_dts = if !tspath::is_declaration_file_name(file_name)
                    && !tspath::file_extension_is(file_name, tspath::EXTENSION_JSON)
                {
                    outputpaths::get_output_declaration_file_name_worker(file_name, &options, self)
                } else {
                    String::new()
                };
                (output_dts, file_name.clone())
            })
            .collect()
    }

    pub fn get_output_file_names(&self) -> Vec<String> {
        let compiler_options = self.compiler_options();
        let mut output_names = Vec::new();

        for file_name in &self.file_names {
            if tspath::is_declaration_file_name(file_name) {
                continue;
            }
            let js_file_name =
                outputpaths::get_output_js_file_name(file_name, &compiler_options, self);
            let is_json = tspath::file_extension_is(file_name, tspath::EXTENSION_JSON);
            if !js_file_name.is_empty() {
                output_names.push(js_file_name.clone());
                if !is_json {
                    let source_map =
                        outputpaths::get_source_map_file_path(&js_file_name, &compiler_options);
                    if !source_map.is_empty() {
                        output_names.push(source_map);
                    }
                }
            }
            if is_json {
                continue;
            }
            if compiler_options.get_emit_declarations() {
                let dts_file_name = outputpaths::get_output_declaration_file_name_worker(
                    file_name,
                    &compiler_options,
                    self,
                );
                if !dts_file_name.is_empty() {
                    output_names.push(dts_file_name.clone());
                    if compiler_options.get_are_declaration_maps_enabled() {
                        output_names.push(dts_file_name + ".map");
                    }
                }
            }
        }

        output_names
    }

    pub fn get_build_info_file_name(&self) -> String {
        outputpaths::get_build_info_file_name(
            &self.compiler_options(),
            self.compare_paths_options(),
        )
    }

    pub fn file_names(&self) -> &[String] {
        &self.file_names
    }

    pub fn literal_file_names(&self) -> &[String] {
        if self.config_file.is_none() {
            return &[];
        }
        &self.file_names[..self.literal_file_names_len.min(self.file_names.len())]
    }

    pub fn project_references(&self) -> &[core::ProjectReference] {
        &self.project_references
    }

    pub fn resolved_project_reference_paths(&mut self) -> &[String] {
        if self.resolved_project_reference_paths.is_empty() && !self.project_references.is_empty() {
            self.resolved_project_reference_paths = self
                .project_references
                .iter()
                .map(core::resolve_project_reference_path)
                .collect();
        }
        &self.resolved_project_reference_paths
    }

    pub fn wildcard_directories(&mut self) -> &WildcardDirectories {
        if self.wildcard_directories.is_empty() {
            let Some(config_file_specs) = self.config_file_specs() else {
                return &self.wildcard_directories;
            };
            let include_specs = config_file_specs.validated_include_specs.clone();
            let exclude_specs = config_file_specs.validated_exclude_specs.clone();
            let compare_paths_options = self.compare_paths_options();
            self.wildcard_directories =
                get_wildcard_directories(&include_specs, &exclude_specs, &compare_paths_options);
        }
        &self.wildcard_directories
    }

    pub fn file_names_by_path(&self) -> BTreeMap<tspath::Path, String> {
        self.file_names
            .iter()
            .map(|file_name| {
                (
                    tspath::to_path(
                        file_name,
                        &self.current_directory,
                        self.use_case_sensitive_file_names,
                    ),
                    file_name.clone(),
                )
            })
            .collect()
    }

    pub fn possibly_matches_file_name(&self, file_name: &str) -> bool {
        let path = tspath::to_path(
            file_name,
            &self.current_directory,
            self.use_case_sensitive_file_names,
        );
        if self.file_names_by_path().contains_key(&path) {
            return true;
        }
        let include_specs = self
            .config_file_specs()
            .map_or(&[][..], |specs| specs.validated_include_specs.as_slice());
        if include_specs.iter().any(|include| {
            !include.contains(['*', '?'])
                && !ts_vfs::vfsmatch::is_implicit_glob(include)
                && tspath::to_path(
                    include,
                    &self.current_directory,
                    self.use_case_sensitive_file_names,
                ) == path
        }) {
            return true;
        }

        self.wildcard_directory_globs_for_matching()
            .iter()
            .any(|glob| glob.match_input(file_name))
    }

    pub fn possibly_matches_directory_name(&self, directory_path: &str) -> bool {
        let current_directory = self.current_directory.clone();
        let use_case_sensitive_file_names = self.use_case_sensitive_file_names;
        let wildcard_directories = self.wildcard_directories_for_matching();
        wildcard_directories
            .iter()
            .any(|(wildcard_dir, recursive)| {
                let wildcard_dir_path = tspath::to_path(
                    wildcard_dir,
                    &current_directory,
                    use_case_sensitive_file_names,
                );
                if *recursive {
                    tspath::path_contains_path(&wildcard_dir_path, &directory_path.to_owned())
                } else {
                    wildcard_dir_path == directory_path
                }
            })
    }

    pub fn get_matched_file_spec(&self, file_name: &str) -> String {
        self.config_file_specs()
            .map(|specs| specs.get_matched_file_spec(file_name, &self.compare_paths_options()))
            .unwrap_or_default()
    }

    pub fn get_matched_include_spec(&self, file_name: &str) -> (String, bool) {
        let Some(config_file_specs) = self.config_file_specs() else {
            return (String::new(), false);
        };
        if config_file_specs.validated_include_specs.is_empty() {
            return (String::new(), false);
        }
        if config_file_specs.is_default_include_spec {
            return (config_file_specs.validated_include_specs[0].clone(), true);
        }

        (
            config_file_specs.get_matched_include_spec(file_name, &self.compare_paths_options()),
            false,
        )
    }

    pub fn reload_file_names_of_parsed_command_line(&self, fs: &dyn vfs::Fs) -> ParsedCommandLine {
        let Some(config_file_specs) = self.config_file_specs() else {
            return self.clone();
        };
        let (file_names, literal_file_names_len) = get_file_names_from_config_specs(
            config_file_specs,
            &self.current_directory,
            &self.compiler_options(),
            fs,
            &self.extra_file_extensions,
        );
        ParsedCommandLine {
            file_names,
            literal_file_names_len,
            source_to_project_reference: BTreeMap::new(),
            output_dts_to_project_reference: BTreeMap::new(),
            common_source_directory: String::new(),
            resolved_project_reference_paths: Vec::new(),
            ..self.clone()
        }
    }

    pub fn extended_source_files(&self) -> &[String] {
        self.config_file
            .as_ref()
            .map(|config_file| config_file.extended_source_files.as_slice())
            .unwrap_or(&[])
    }

    pub fn get_config_file_parsing_diagnostics(&self) -> Vec<String> {
        let Some(config_file) = &self.config_file else {
            return self.errors.clone();
        };
        config_file
            .diagnostics
            .iter()
            .chain(self.errors.iter())
            .cloned()
            .collect()
    }

    pub fn get_config_file_parsing_ast_diagnostics(&self) -> Vec<ast::Diagnostic> {
        if let Some(config_file) = self.config_file.as_ref() {
            let mut diagnostics = config_file.source_file.diagnostics().to_vec();
            for diagnostic in &config_file.ast_diagnostics {
                if !diagnostics
                    .iter()
                    .any(|existing| ast::equal_diagnostics(existing, diagnostic))
                {
                    diagnostics.push(diagnostic.clone());
                }
            }
            diagnostics
        } else {
            Vec::new()
        }
    }

    pub fn locale(&self) -> locale::Locale {
        locale::parse(&self.compiler_options().locale).0
    }

    fn config_file_specs(&self) -> Option<&ConfigFileSpecs> {
        self.config_file
            .as_ref()
            .and_then(|config_file| config_file.config_file_specs.as_ref())
    }

    fn wildcard_directories_for_matching(&self) -> WildcardDirectories {
        if !self.wildcard_directories.is_empty() {
            return self.wildcard_directories.clone();
        }
        let Some(config_file_specs) = self.config_file_specs() else {
            return WildcardDirectories::default();
        };
        let compare_paths_options = self.compare_paths_options();
        get_wildcard_directories(
            &config_file_specs.validated_include_specs,
            &config_file_specs.validated_exclude_specs,
            &compare_paths_options,
        )
    }

    fn wildcard_directory_globs_for_matching(&self) -> Vec<ts_glob::Glob> {
        let wildcard_directories = self.wildcard_directories_for_matching();
        wildcard_directories
            .into_iter()
            .filter_map(|(dir, recursive)| {
                let spec = format!(
                    "{}/{}",
                    tspath::normalize_path(&dir),
                    if recursive {
                        RECURSIVE_FILE_GLOB_PATTERN
                    } else {
                        FILE_GLOB_PATTERN
                    }
                );
                ts_glob::parse(&spec).ok()
            })
            .collect()
    }

    pub fn compare_paths_options(&self) -> tspath::ComparePathsOptions {
        tspath::ComparePathsOptions {
            use_case_sensitive_file_names: self.use_case_sensitive_file_names,
            current_directory: self.current_directory.clone(),
        }
    }
}

impl ts_module::ResolvedProjectReference for ParsedCommandLine {
    fn config_name(&self) -> String {
        ParsedCommandLine::config_name(self)
    }

    fn compiler_options(&self) -> core::CompilerOptions {
        ParsedCommandLine::compiler_options(self)
    }
}

impl outputpaths::OutputPathsHost for ParsedCommandLine {
    fn common_source_directory(&self) -> String {
        self.compute_common_source_directory()
    }

    fn get_current_directory(&self) -> String {
        self.current_directory.clone()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.use_case_sensitive_file_names
    }
}

fn check_source_files_belong_to_path(
    source_files: &[String],
    root_directory: &str,
    current_directory: &str,
    use_case_sensitive_file_names: bool,
    compare_paths_options: &tspath::ComparePathsOptions,
    errors: &mut Vec<String>,
) -> bool {
    let mut all_files_belong_to_path = true;
    for file in source_files {
        let absolute_source_file_path = tspath::get_canonical_file_name(
            &tspath::get_normalized_absolute_path(file, current_directory),
            use_case_sensitive_file_names,
        );
        if !tspath::contains_path(root_directory, file, compare_paths_options) {
            errors.push(format!(
                "File '{absolute_source_file_path}' is not under 'rootDir' '{root_directory}'. 'rootDir' is expected to contain all source files."
            ));
            all_files_belong_to_path = false;
        }
    }

    all_files_belong_to_path
}

pub(crate) fn compiler_options_to_string_map(
    options: &core::CompilerOptions,
) -> BTreeMap<String, String> {
    let mut result = json_object_to_string_map(options);
    if options.type_roots_configured {
        let value = serde_json::to_string(&options.type_roots).unwrap_or_else(|_| "[]".to_owned());
        result.insert("typeRoots".to_owned(), value);
    }
    if !options.paths_base_path.is_empty() {
        result.insert("pathsBasePath".to_owned(), options.paths_base_path.clone());
    }
    if options.target_is_es3 {
        result.insert("target".to_owned(), "es3".to_owned());
    }
    result
}

fn json_object_to_string_map<T: serde::Serialize>(value: &T) -> BTreeMap<String, String> {
    let Ok(Value::Object(object)) = serde_json::to_value(value) else {
        return BTreeMap::new();
    };
    object
        .into_iter()
        .map(|(key, value)| (key, json_value_to_option_string(value)))
        .collect()
}

fn json_value_to_option_string(value: Value) -> String {
    match value {
        Value::String(value) => value,
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        value => value.to_string(),
    }
}

fn string_map_to_json_object(values: &BTreeMap<String, String>) -> Map<String, Value> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), option_string_to_json_value(value)))
        .collect()
}

fn option_string_to_json_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_owned()))
}

fn paths_for_validation(value: Option<&Value>) -> OrderedMap<String, Value> {
    let Some(object) = value.and_then(Value::as_object) else {
        return OrderedMap::new();
    };

    let has_invalid_path = object.values().any(|value| match value {
        Value::Array(values) => values.iter().any(|value| !value.is_string()),
        _ => true,
    });
    if !has_invalid_path {
        return OrderedMap::new();
    }

    let mut result = OrderedMap::with_size_hint(object.len());
    for (key, value) in object {
        result.set(key.clone(), value.clone());
    }
    result
}

fn compiler_options_paths_json_value(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Object(Map::new());
    };

    let mut paths = Map::new();
    for (key, value) in object {
        let Value::Array(values) = value else {
            continue;
        };
        paths.insert(
            key.clone(),
            Value::Array(
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|value| Value::String(value.to_owned()))
                    .collect(),
            ),
        );
    }
    Value::Object(paths)
}

pub fn compiler_option_json_value(
    name: &str,
    value: &str,
    kind: CommandLineOptionKind,
    enum_map: Option<&BTreeMap<String, CompilerOptionsValue>>,
) -> Option<Value> {
    match kind {
        CommandLineOptionKind::Boolean => Some(Value::Bool(parse_tristate(value).is_true())),
        CommandLineOptionKind::Number => value
            .parse::<i64>()
            .ok()
            .map(Number::from)
            .map(Value::Number),
        CommandLineOptionKind::String => Some(Value::String(value.to_owned())),
        CommandLineOptionKind::Object => {
            if value == "null" {
                None
            } else {
                serde_json::from_str(value)
                    .ok()
                    .or_else(|| Some(Value::String(value.to_owned())))
            }
        }
        CommandLineOptionKind::List | CommandLineOptionKind::ListOrElement => {
            if value == "null" {
                return None;
            }
            let value = parse_list_value(value)?;
            let Some(enum_map) = enum_map else {
                return Some(value);
            };
            let Value::Array(values) = value else {
                return Some(value);
            };
            Some(Value::Array(
                values
                    .into_iter()
                    .filter_map(|value| match value {
                        Value::String(value) => {
                            let key = value.trim().to_ascii_lowercase();
                            if key.is_empty() {
                                None
                            } else {
                                enum_map.get(&key).and_then(compiler_options_value_to_json)
                            }
                        }
                        value => Some(value),
                    })
                    .collect(),
            ))
        }
        CommandLineOptionKind::Enum => {
            if value == "null" {
                return None;
            }
            if let Ok(value) = value.parse::<i64>() {
                return Some(Value::Number(Number::from(value)));
            }
            if let Some(value) = core_enum_option_json_value(name, value) {
                return Some(value);
            }
            let mapped = enum_map
                .and_then(|map| map.get(&value.to_ascii_lowercase()))
                .and_then(compiler_options_value_to_json);

            mapped.or_else(|| match name {
                "newLine" => match value.to_ascii_lowercase().as_str() {
                    "crlf" => Some(Value::Number(Number::from(core::NewLineKind::CRLF.0))),
                    "lf" => Some(Value::Number(Number::from(core::NewLineKind::LF.0))),
                    _ => None,
                },
                _ => None,
            })
        }
    }
}

fn compiler_options_value_to_json(value: &CompilerOptionsValue) -> Option<Value> {
    match value {
        CompilerOptionsValue::Bool(value) => Some(Value::Bool(*value)),
        CompilerOptionsValue::String(value) => Some(Value::String(value.clone())),
        CompilerOptionsValue::Number(value) => Some(Value::Number(Number::from(*value))),
        CompilerOptionsValue::Unknown => None,
    }
}

pub(crate) fn core_enum_option_json_value(name: &str, value: &str) -> Option<Value> {
    let value = value.to_ascii_lowercase();
    let numeric = match name {
        "target" => match value.as_str() {
            "es3" => core::ScriptTarget::None.0,
            "es5" => core::ScriptTarget::ES5.0,
            "es6" | "es2015" => core::ScriptTarget::ES2015.0,
            "es2016" => core::ScriptTarget::ES2016.0,
            "es2017" => core::ScriptTarget::ES2017.0,
            "es2018" => core::ScriptTarget::ES2018.0,
            "es2019" => core::ScriptTarget::ES2019.0,
            "es2020" => core::ScriptTarget::ES2020.0,
            "es2021" => core::ScriptTarget::ES2021.0,
            "es2022" => core::ScriptTarget::ES2022.0,
            "es2023" => core::ScriptTarget::ES2023.0,
            "es2024" => core::ScriptTarget::ES2024.0,
            "es2025" => core::ScriptTarget::ES2025.0,
            "esnext" => core::ScriptTarget::ESNext.0,
            _ => return None,
        },
        "module" => match value.as_str() {
            "commonjs" => core::ModuleKind::CommonJS.0,
            "amd" => core::ModuleKind::AMD.0,
            "system" => core::ModuleKind::System.0,
            "umd" => core::ModuleKind::UMD.0,
            "es6" | "es2015" => core::ModuleKind::ES2015.0,
            "es2020" => core::ModuleKind::ES2020.0,
            "es2022" => core::ModuleKind::ES2022.0,
            "esnext" => core::ModuleKind::ESNext.0,
            "node16" => core::ModuleKind::Node16.0,
            "node18" => core::ModuleKind::Node18.0,
            "node20" => core::ModuleKind::Node20.0,
            "nodenext" => core::ModuleKind::NodeNext.0,
            "preserve" => core::ModuleKind::Preserve.0,
            _ => return None,
        },
        "jsx" => match value.as_str() {
            "preserve" => core::JsxEmit::Preserve.0,
            "react-native" => core::JsxEmit::ReactNative.0,
            "react" => core::JsxEmit::React.0,
            "react-jsx" => core::JsxEmit::ReactJSX.0,
            "react-jsxdev" => core::JsxEmit::ReactJSXDev.0,
            _ => return None,
        },
        "moduleResolution" => match value.as_str() {
            "classic" => core::ModuleResolutionKind::Classic.0,
            "node" | "node10" => core::ModuleResolutionKind::Node10.0,
            "node16" => core::ModuleResolutionKind::Node16.0,
            "nodenext" => core::ModuleResolutionKind::NodeNext.0,
            "bundler" => core::ModuleResolutionKind::Bundler.0,
            _ => return None,
        },
        "moduleDetection" => match value.as_str() {
            "auto" => core::ModuleDetectionKind::Auto.0,
            "legacy" => core::ModuleDetectionKind::Legacy.0,
            "force" => core::ModuleDetectionKind::Force.0,
            _ => return None,
        },
        "newLine" => match value.as_str() {
            "crlf" => core::NewLineKind::CRLF.0,
            "lf" => core::NewLineKind::LF.0,
            _ => return None,
        },
        _ => return None,
    };
    Some(Value::Number(Number::from(numeric)))
}

fn watch_enum_option_json_value(name: &str, value: &str) -> Option<Value> {
    let value = value.to_ascii_lowercase();
    let numeric = match name {
        "watchFile" => match value.as_str() {
            "fixedpollinginterval" => core::WatchFileKind::FixedPollingInterval.0,
            "prioritypollinginterval" => core::WatchFileKind::PriorityPollingInterval.0,
            "dynamicprioritypolling" => core::WatchFileKind::DynamicPriorityPolling.0,
            "fixedchunksizepolling" => core::WatchFileKind::FixedChunkSizePolling.0,
            "usefsevents" => core::WatchFileKind::UseFsEvents.0,
            "usefseventsonparentdirectory" => core::WatchFileKind::UseFsEventsOnParentDirectory.0,
            _ => return None,
        },
        "watchDirectory" => match value.as_str() {
            "usefsevents" => core::WatchDirectoryKind::UseFsEvents.0,
            "fixedpollinginterval" => core::WatchDirectoryKind::FixedPollingInterval.0,
            "dynamicprioritypolling" => core::WatchDirectoryKind::DynamicPriorityPolling.0,
            "fixedchunksizepolling" => core::WatchDirectoryKind::FixedChunkSizePolling.0,
            _ => return None,
        },
        "fallbackPolling" => match value.as_str() {
            "fixedinterval" => core::PollingKind::FixedInterval.0,
            "priorityinterval" => core::PollingKind::PriorityInterval.0,
            "dynamicpriority" => core::PollingKind::DynamicPriority.0,
            "fixedchunksize" => core::PollingKind::FixedChunkSize.0,
            _ => return None,
        },
        _ => return None,
    };
    Some(Value::Number(Number::from(numeric)))
}

fn parse_list_value(value: &str) -> Option<Value> {
    if let Ok(parsed) = serde_json::from_str::<Value>(value) {
        return Some(parsed);
    }
    let value = value.trim();
    if value.is_empty() {
        return Some(Value::Array(Vec::new()));
    }
    Some(Value::Array(
        value
            .split(',')
            .map(|entry| Value::String(entry.to_owned()))
            .collect(),
    ))
}

fn parse_tristate(value: &str) -> core::Tristate {
    match value {
        "true" | "True" | "1" => core::TS_TRUE,
        "false" | "False" | "0" => core::TS_FALSE,
        _ => core::TS_UNKNOWN,
    }
}

pub fn parsed_command_line_options(
    options: BTreeMap<String, String>,
    file_names: Vec<String>,
) -> ParsedCommandLine {
    ParsedCommandLine {
        options,
        file_names,
        literal_file_names_len: 0,
        ..ParsedCommandLine::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_options_converts_lib_names_to_lib_file_names() {
        let parsed = parsed_command_line_options(
            BTreeMap::from([("lib".to_owned(), "es6 ".to_owned())]),
            Vec::new(),
        );

        assert_eq!(parsed.compiler_options().lib, vec!["lib.es2015.d.ts"]);
    }

    #[test]
    fn compiler_options_filters_invalid_lib_names() {
        let parsed = parsed_command_line_options(
            BTreeMap::from([("lib".to_owned(), "es6,missing,dom".to_owned())]),
            Vec::new(),
        );

        assert_eq!(
            parsed.compiler_options().lib,
            vec!["lib.es2015.d.ts", "lib.dom.d.ts"]
        );
    }

    #[test]
    fn watch_options_converts_watch_file_to_core_enum() {
        let parsed = ParsedCommandLine {
            watch_options: BTreeMap::from([("watchFile".to_owned(), "useFsEvents".to_owned())]),
            ..ParsedCommandLine::default()
        };

        assert_eq!(
            parsed.watch_options().file_kind,
            core::WatchFileKind::UseFsEvents
        );
    }

    #[test]
    fn compiler_options_does_not_configure_type_roots_when_explicitly_null() {
        let parsed = ParsedCommandLine {
            options: BTreeMap::from([("typeRoots".to_owned(), "null".to_owned())]),
            explicit_null_options: BTreeSet::from(["typeRoots".to_owned()]),
            ..ParsedCommandLine::default()
        };

        assert!(!parsed.compiler_options().type_roots_configured);
    }
}
