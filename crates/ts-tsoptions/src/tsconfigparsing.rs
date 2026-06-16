#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::parsedcommandline::{ParsedCommandLine, compiler_options_to_string_map};
use crate::{CommandLineOptionKind, command_line_options_to_map, options_declarations};
use serde_json::Value;
use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_module as module;
use ts_parser as parser;
use ts_tspath as tspath;
use ts_vfs::{self as vfs, vfsmatch};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConfigFile {
    pub file_name: String,
    pub json: String,
}

pub trait ParseConfigHost: Send + Sync {
    fn fs(&self) -> &dyn vfs::Fs;
    fn get_current_directory(&self) -> String;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileExtensionInfo {
    pub extension: String,
    pub is_mixed_content: bool,
    pub script_kind: core::ScriptKind,
}

#[derive(Clone, Debug, Default)]
pub struct ExtendsResult {
    pub options: core::CompilerOptions,
    pub watch_options_copied: bool,
    pub include: Option<Vec<Value>>,
    pub exclude: Option<Vec<Value>>,
    pub files: Option<Vec<Value>>,
    pub compile_on_save: bool,
    pub extended_source_files: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConfigFileSpecs {
    pub files_specs: Option<Vec<Value>>,
    pub include_specs: Option<Vec<Value>>,
    pub exclude_specs: Option<Vec<Value>>,
    pub validated_files_spec: Vec<String>,
    pub validated_include_specs: Vec<String>,
    pub validated_exclude_specs: Vec<String>,
    pub validated_files_spec_before_substitution: Vec<String>,
    pub validated_include_specs_before_substitution: Vec<String>,
    pub is_default_include_spec: bool,
}

impl ConfigFileSpecs {
    pub fn matches_exclude(
        &self,
        file_name: &str,
        compare_paths_options: &tspath::ComparePathsOptions,
    ) -> bool {
        if self.validated_exclude_specs.is_empty() {
            return false;
        }
        let Some(exclude_matcher) = vfsmatch::new_spec_matcher(
            &self.validated_exclude_specs,
            &compare_paths_options.current_directory,
            vfsmatch::Usage::Exclude,
            compare_paths_options.use_case_sensitive_file_names,
        ) else {
            return false;
        };
        if exclude_matcher.match_string(file_name) {
            return true;
        }
        if !tspath::has_extension(file_name) {
            return exclude_matcher
                .match_string(&tspath::ensure_trailing_directory_separator(file_name));
        }
        false
    }

    pub fn get_matched_include_spec(
        &self,
        file_name: &str,
        compare_paths_options: &tspath::ComparePathsOptions,
    ) -> String {
        for (index, spec) in self.validated_include_specs.iter().enumerate() {
            let specs = vec![spec.clone()];
            let Some(include_matcher) = vfsmatch::new_spec_matcher(
                &specs,
                &compare_paths_options.current_directory,
                vfsmatch::Usage::Files,
                compare_paths_options.use_case_sensitive_file_names,
            ) else {
                continue;
            };
            if include_matcher.match_string(file_name) {
                return self
                    .validated_include_specs_before_substitution
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| spec.clone());
            }
        }
        String::new()
    }

    pub fn get_matched_file_spec(
        &self,
        file_name: &str,
        compare_paths_options: &tspath::ComparePathsOptions,
    ) -> String {
        if self.validated_files_spec.is_empty() {
            return String::new();
        }
        let file_path = tspath::to_path(
            file_name,
            &compare_paths_options.current_directory,
            compare_paths_options.use_case_sensitive_file_names,
        );
        for (index, spec) in self.validated_files_spec.iter().enumerate() {
            if tspath::to_path(
                spec,
                &compare_paths_options.current_directory,
                compare_paths_options.use_case_sensitive_file_names,
            ) == file_path
            {
                return self
                    .validated_files_spec_before_substitution
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| spec.clone());
            }
        }
        String::new()
    }
}

pub struct TsConfigSourceFile {
    pub source_file: ast::SourceFile,
    pub extended_source_files: Vec<String>,
    pub config_file_specs: Option<ConfigFileSpecs>,
    pub file_name: String,
    pub path: tspath::Path,
    pub json_text: String,
    pub json: Value,
    pub diagnostics: Vec<String>,
    pub ast_diagnostics: Vec<ast::Diagnostic>,
}

impl Clone for TsConfigSourceFile {
    fn clone(&self) -> Self {
        Self {
            source_file: self.source_file.share_readonly(),
            extended_source_files: self.extended_source_files.clone(),
            config_file_specs: self.config_file_specs.clone(),
            file_name: self.file_name.clone(),
            path: self.path.clone(),
            json_text: self.json_text.clone(),
            json: self.json.clone(),
            diagnostics: self.diagnostics.clone(),
            ast_diagnostics: self.ast_diagnostics.clone(),
        }
    }
}

impl std::fmt::Debug for TsConfigSourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TsConfigSourceFile")
            .field("file_name", &self.file_name)
            .field("path", &self.path)
            .field("json_text", &self.json_text)
            .field("json", &self.json)
            .field("diagnostics", &self.diagnostics)
            .finish_non_exhaustive()
    }
}

pub fn tsconfig_to_source_file(
    source_file: Option<&TsConfigSourceFile>,
) -> Option<&TsConfigSourceFile> {
    source_file
}

pub fn new_tsconfig_source_file_from_file_path(
    config_file_name: &str,
    config_path: tspath::Path,
    config_source_text: &str,
) -> TsConfigSourceFile {
    let (source_file, json, diagnostics, ast_diagnostics) =
        parse_config_file_text_to_json_source_file(
            config_file_name,
            &config_path,
            config_source_text,
        );
    TsConfigSourceFile {
        source_file,
        file_name: config_file_name.to_owned(),
        path: config_path,
        json_text: config_source_text.to_owned(),
        json,
        diagnostics,
        ast_diagnostics,
        extended_source_files: Vec::new(),
        config_file_specs: None,
    }
}

fn new_empty_tsconfig_source_file(
    file_name: &str,
    path: tspath::Path,
    json_text: String,
    json: Value,
) -> TsConfigSourceFile {
    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: file_name.to_owned(),
            path: path.clone(),
            ..Default::default()
        },
        json_text.clone(),
        core::ScriptKind::JSON,
    );
    TsConfigSourceFile {
        source_file,
        extended_source_files: Vec::new(),
        config_file_specs: None,
        file_name: file_name.to_owned(),
        path,
        json_text,
        json,
        diagnostics: Vec::new(),
        ast_diagnostics: Vec::new(),
    }
}

pub fn get_ts_config_prop_array_element_value(
    ts_config_source_file: &ast::SourceFile,
    prop_key: &str,
    element_value: &str,
) -> Option<ast::Node> {
    let callback = get_callback_for_finding_property_assignment_by_value(
        ts_config_source_file.store(),
        element_value,
    );
    for_each_ts_config_prop_array(Some(ts_config_source_file), prop_key, callback)
}

pub fn for_each_ts_config_prop_array<T, F>(
    ts_config_source_file: Option<&ast::SourceFile>,
    prop_key: &str,
    callback: F,
) -> Option<T>
where
    F: FnMut(ast::Node) -> Option<T>,
{
    ts_config_source_file.and_then(|source_file| {
        for_each_property_assignment(
            source_file.store(),
            get_ts_config_object_literal_expression(source_file),
            prop_key,
            callback,
            "",
        )
    })
}

pub fn create_diagnostic_at_reference_syntax(
    config: &ParsedCommandLine,
    index: usize,
    message: &diagnostics::Message,
    args: &[diagnostics::Argument],
) -> Option<ast::Diagnostic> {
    let source_file = config
        .config_file
        .as_ref()
        .map(|config_file| &config_file.source_file)?;
    for_each_ts_config_prop_array(Some(source_file), "references", |property| {
        let initializer = source_file.store().initializer(property)?;
        if ast::is_array_literal_expression(source_file.store(), initializer) {
            let elements = source_file.store().elements(initializer)?;
            if let Some(element) = elements.iter().nth(index) {
                return Some(crate::create_diagnostic_for_node_in_source_file(
                    source_file,
                    element,
                    message,
                    args,
                ));
            }
        }
        None
    })
}

pub fn get_callback_for_finding_property_assignment_by_value<'a>(
    store: &'a ast::AstStore,
    value: &'a str,
) -> impl Fn(ast::Node) -> Option<ast::Node> + 'a {
    move |property| {
        let initializer = store.initializer(property)?;
        if !ast::is_array_literal_expression(store, initializer) {
            return None;
        }
        store.elements(initializer)?.into_iter().find(|element| {
            ast::is_string_literal(store, *element) && store.text(*element) == value
        })
    }
}

pub fn get_options_syntax_by_array_element_value(
    store: &ast::AstStore,
    object_literal: Option<ast::Node>,
    prop_key: &str,
    element_value: &str,
) -> Option<ast::Node> {
    for_each_property_assignment(
        store,
        object_literal,
        prop_key,
        get_callback_for_finding_property_assignment_by_value(store, element_value),
        "",
    )
}

pub fn for_each_property_assignment<T, F>(
    store: &ast::AstStore,
    object_literal: Option<ast::Node>,
    key: &str,
    mut callback: F,
    key2: &str,
) -> Option<T>
where
    F: FnMut(ast::Node) -> Option<T>,
{
    let object_literal = object_literal?;
    for property in store.properties(object_literal)? {
        if !ast::is_property_assignment(store, property) {
            continue;
        }
        let Some(name) = store.name(property) else {
            continue;
        };
        let (prop_name, ok) = ast::try_get_text_of_property_name(store, name);
        if ok && (prop_name == key || (!key2.is_empty() && prop_name == key2)) {
            return callback(property);
        }
    }
    None
}

pub fn get_ts_config_object_literal_expression(
    ts_config_source_file: &ast::SourceFile,
) -> Option<ast::Node> {
    let store = ts_config_source_file.store();
    let statement = ts_config_source_file.statements_view().first()?;
    let expression = store.expression(statement)?;
    ast::is_object_literal_expression(store, expression).then_some(expression)
}

#[derive(Clone, Debug, Default)]
pub struct ParsedTsconfig {
    pub raw: Value,
    pub options: core::CompilerOptions,
    pub type_acquisition: core::TypeAcquisition,
    pub extended_config_path: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ExtendedConfigCacheEntry {
    pub extended_result: Option<TsConfigSourceFile>,
    pub extended_config: Option<ParsedTsconfig>,
    pub errors: Vec<String>,
}

impl ExtendedConfigCacheEntry {
    pub fn extended_file_names(&self) -> &[String] {
        self.extended_result
            .as_ref()
            .map(|result| result.extended_source_files.as_slice())
            .unwrap_or(&[])
    }
}

pub trait ExtendedConfigCache: Send + Sync {
    fn get_extended_config(
        &self,
        file_name: String,
        path: tspath::Path,
        resolution_stack: Vec<String>,
        host: &dyn ParseConfigHost,
    ) -> ExtendedConfigCacheEntry;
}

impl<T: ExtendedConfigCache + ?Sized> ExtendedConfigCache for Arc<T> {
    fn get_extended_config(
        &self,
        file_name: String,
        path: tspath::Path,
        resolution_stack: Vec<String>,
        host: &dyn ParseConfigHost,
    ) -> ExtendedConfigCacheEntry {
        (**self).get_extended_config(file_name, path, resolution_stack, host)
    }
}

struct ResolverHost<'a> {
    host: &'a dyn ParseConfigHost,
}

impl module::ResolutionHost for ResolverHost<'_> {
    fn get_current_directory(&self) -> String {
        self.host.get_current_directory()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        self.host.fs()
    }
}

pub fn parse_json_config_file_content(
    config: &ConfigFile,
    host: &dyn ParseConfigHost,
) -> ParsedCommandLine {
    let (json, mut errors) = parse_config_file_text_to_json(
        &config.file_name,
        &tspath::to_path(
            &config.file_name,
            &host.get_current_directory(),
            host.fs().use_case_sensitive_file_names(),
        ),
        &config.json,
    );
    let base_path = directory_of_combined_path(&config.file_name, &host.get_current_directory());
    let mut result = parse_json_config_file_content_worker(ParseJsonConfigFileContentWorkerInput {
        json: Some(json),
        source_file: None,
        host,
        base_path: &base_path,
        existing_options: None,
        existing_options_raw: None,
        config_file_name: &config.file_name,
        resolution_stack: &[],
        extra_file_extensions: &[],
        extended_config_cache: None,
    });
    result.errors.splice(0..0, errors.drain(..));
    result.raw = Some(config.json.clone());
    result
}

fn directory_of_combined_path(file_name: &str, base_path: &str) -> String {
    tspath::get_directory_path(&tspath::get_normalized_absolute_path(file_name, base_path))
}

pub fn parse_config(file_name: &str, json: &str, host: &dyn ParseConfigHost) -> ParsedCommandLine {
    let current_directory = host.get_current_directory();
    let path = tspath::to_path(
        file_name,
        &current_directory,
        host.fs().use_case_sensitive_file_names(),
    );
    let source_file = new_tsconfig_source_file_from_file_path(file_name, path, json);
    let base_path = directory_of_combined_path(file_name, &current_directory);
    parse_json_source_file_config_file_content(ParseJsonSourceFileConfigFileContentInput {
        source_file,
        host,
        base_path: &base_path,
        existing_options: None,
        existing_options_raw: None,
        config_file_name: file_name,
        resolution_stack: &[],
        extra_file_extensions: &[],
        extended_config_cache: None,
    })
}

pub fn parse_config_file_text_to_json(
    file_name: &str,
    path: &tspath::Path,
    json_text: &str,
) -> (Value, Vec<String>) {
    let (_, json, diagnostics, _) =
        parse_config_file_text_to_json_source_file(file_name, path, json_text);
    (json, diagnostics)
}

fn parse_config_file_text_to_json_source_file(
    file_name: &str,
    path: &tspath::Path,
    json_text: &str,
) -> (ast::SourceFile, Value, Vec<String>, Vec<ast::Diagnostic>) {
    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: file_name.to_owned(),
            path: path.clone(),
            ..Default::default()
        },
        json_text.to_owned(),
        core::ScriptKind::JSON,
    );
    let (json, mut errors) = convert_config_file_to_object(&source_file);
    let mut ast_errors = Vec::new();
    if let Some(diagnostic) = source_file.diagnostics().first() {
        errors = vec![diagnostic.to_string()];
        ast_errors = vec![diagnostic.clone()];
    }
    (source_file, json, errors, ast_errors)
}

fn convert_config_file_to_object(source_file: &ast::SourceFile) -> (Value, Vec<String>) {
    let root_expression = source_file
        .statements_view()
        .first()
        .and_then(|statement| source_file.store().expression(statement));
    if let Some(root_expression) = root_expression
        && source_file.store().kind(root_expression) != ast::Kind::ObjectLiteralExpression
    {
        let base_file_name =
            if tspath::get_base_file_name(&source_file.file_name()) == "jsconfig.json" {
                "jsconfig.json"
            } else {
                "tsconfig.json"
            };
        let errors = vec![format!(
            "The root value of a {base_file_name} file must be an object"
        )];
        if ast::is_array_literal_expression(source_file.store(), root_expression)
            && let Some(first_object) = source_file
                .store()
                .elements(root_expression)
                .into_iter()
                .flatten()
                .find(|node| ast::is_object_literal_expression(source_file.store(), *node))
        {
            return convert_to_json(source_file, Some(first_object), true);
        }
        return (Value::Object(Default::default()), errors);
    }
    convert_to_json(source_file, root_expression, true)
}

fn convert_to_json(
    source_file: &ast::SourceFile,
    root_expression: Option<ast::Node>,
    return_value: bool,
) -> (Value, Vec<String>) {
    match root_expression {
        Some(root_expression) => {
            convert_property_value_to_json(source_file, root_expression, return_value)
        }
        None if return_value => (Value::Object(Default::default()), Vec::new()),
        None => (Value::Null, Vec::new()),
    }
}

fn convert_property_value_to_json(
    source_file: &ast::SourceFile,
    value_expression: ast::Node,
    return_value: bool,
) -> (Value, Vec<String>) {
    let store = source_file.store();
    match store.kind(value_expression) {
        ast::Kind::TrueKeyword => return (Value::Bool(true), Vec::new()),
        ast::Kind::FalseKeyword => return (Value::Bool(false), Vec::new()),
        ast::Kind::NullKeyword => return (Value::Null, Vec::new()),
        ast::Kind::StringLiteral => {
            return (
                Value::String(source_file.store().text(value_expression)),
                Vec::new(),
            );
        }
        ast::Kind::NumericLiteral => {
            let value = source_file
                .store()
                .text(value_expression)
                .parse::<f64>()
                .expect("numeric literal text");
            return (
                serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .expect("finite numeric literal"),
                Vec::new(),
            );
        }
        ast::Kind::PrefixUnaryExpression => {
            if store.operator(value_expression) == Some(ast::Kind::MinusToken)
                && let Some(operand) = store.operand(value_expression)
                && store.kind(operand) == ast::Kind::NumericLiteral
            {
                let value = -source_file
                    .store()
                    .text(operand)
                    .parse::<f64>()
                    .expect("numeric literal text");
                return (
                    serde_json::Number::from_f64(value)
                        .map(Value::Number)
                        .expect("finite numeric literal"),
                    Vec::new(),
                );
            }
        }
        ast::Kind::ObjectLiteralExpression => {
            return convert_object_literal_expression_to_json(
                source_file,
                return_value,
                value_expression,
            );
        }
        ast::Kind::ArrayLiteralExpression => {
            let mut values = Vec::new();
            let mut errors = Vec::new();
            for element in source_file
                .store()
                .elements(value_expression)
                .into_iter()
                .flatten()
            {
                let (value, element_errors) =
                    convert_property_value_to_json(source_file, element, return_value);
                errors.extend(element_errors);
                if return_value {
                    values.push(value);
                }
            }
            return (Value::Array(values), errors);
        }
        _ => {}
    }
    (
        Value::Null,
        vec![
            diagnostics::PROPERTY_VALUE_CAN_ONLY_BE_STRING_LITERAL_NUMERIC_LITERAL_TRUE_FALSE_NULL_OBJECT_LITERAL_OR_ARRAY_LITERAL
                .string()
                .to_owned(),
        ],
    )
}

fn convert_object_literal_expression_to_json(
    source_file: &ast::SourceFile,
    return_value: bool,
    node: ast::Node,
) -> (Value, Vec<String>) {
    let store = source_file.store();
    let mut result = serde_json::Map::new();
    let mut errors = Vec::new();
    for element in store.properties(node).into_iter().flatten() {
        if !ast::is_property_assignment(store, element) {
            errors.push(
                diagnostics::PROPERTY_ASSIGNMENT_EXPECTED
                    .string()
                    .to_owned(),
            );
            continue;
        }
        let name = store.name(element);
        if name.is_some_and(|name| !ast::is_string_literal(store, name)) {
            errors.push(
                diagnostics::STRING_LITERAL_WITH_DOUBLE_QUOTES_EXPECTED
                    .string()
                    .to_owned(),
            );
        }
        let key_text = if let Some(name) = name
            .as_ref()
            .filter(|name| !ast::is_computed_non_literal_name(store, **name))
        {
            let (text, ok) = ast::try_get_text_of_property_name(store, *name);
            if ok { text } else { String::new() }
        } else {
            String::new()
        };
        let (value, value_errors) = if let Some(initializer) = store.initializer(element) {
            convert_property_value_to_json(source_file, initializer, return_value)
        } else {
            (Value::Null, Vec::new())
        };
        errors.extend(value_errors);
        if return_value && !key_text.is_empty() {
            result.insert(key_text, value);
        }
    }
    (Value::Object(result), errors)
}

fn get_source_file_compiler_option_diagnostics(
    source_file: &ast::SourceFile,
) -> Vec<ast::Diagnostic> {
    let store = source_file.store();
    let Some(root_expression) = source_file
        .statements_view()
        .first()
        .and_then(|statement| source_file.store().expression(statement))
    else {
        return Vec::new();
    };
    if !ast::is_object_literal_expression(store, root_expression) {
        return Vec::new();
    }
    let Some(compiler_options_property) =
        find_property_assignment(source_file.store(), root_expression, "compilerOptions")
    else {
        return Vec::new();
    };
    let Some(initializer) = source_file.store().initializer(compiler_options_property) else {
        return Vec::new();
    };
    if !ast::is_object_literal_expression(store, initializer) {
        return Vec::new();
    }

    let option_map = command_line_options_to_map(options_declarations());
    let mut diagnostics = Vec::new();
    {
        for element in source_file
            .store()
            .properties(initializer)
            .into_iter()
            .flatten()
        {
            if !ast::is_property_assignment(store, element) {
                continue;
            }
            let Some(name) = store.name(element) else {
                continue;
            };
            let (key, ok) = ast::try_get_text_of_property_name(source_file.store(), name);
            if !ok {
                continue;
            }
            let option = option_map
                .get(&key)
                .or_else(|| option_map.get(&key.to_lowercase()));
            let Some(option) = option else {
                diagnostics.push(crate::errors::create_diagnostic_for_ast_node_in_source_file_or_compiler_diagnostic(
                    Some(source_file),
                    Some(name),
                    &diagnostics::Unknown_compiler_option_0,
                    &[Box::new(key) as diagnostics::Argument],
                ));
                continue;
            };
            if option.name != key {
                diagnostics.push(crate::errors::create_diagnostic_for_ast_node_in_source_file_or_compiler_diagnostic(
                    Some(source_file),
                    Some(name),
                    &diagnostics::Unknown_compiler_option_0_Did_you_mean_1,
                    &[
                        Box::new(key) as diagnostics::Argument,
                        Box::new(option.name.clone()) as diagnostics::Argument,
                    ],
                ));
                continue;
            }
            if option.kind == Some(CommandLineOptionKind::Enum)
                && let Some(value) = store.initializer(element)
                && ast::is_string_literal(store, value)
                && option.enum_map().is_some_and(|enum_map| {
                    !enum_map.contains_key(&source_file.store().text(value).to_lowercase())
                })
            {
                let enum_keys = crate::enummaps::enum_keys(&option.name).expect("enum option keys");
                diagnostics.push(crate::errors::create_diagnostic_for_ast_node_in_source_file_or_compiler_diagnostic(
                    Some(source_file),
                    Some(value),
                    &diagnostics::Argument_for_0_option_must_be_Colon_1,
                    &[
                        Box::new(format!("--{}", option.name)) as diagnostics::Argument,
                        Box::new(crate::errors::format_enum_type_keys(option, enum_keys))
                            as diagnostics::Argument,
                    ],
                ));
            }
        }
    }
    diagnostics
}

fn find_property_assignment(
    store: &ast::AstStore,
    object_literal: ast::Node,
    key: &str,
) -> Option<ast::Node> {
    for element in store.properties(object_literal).into_iter().flatten() {
        if !ast::is_property_assignment(store, element) {
            continue;
        }
        let Some(name) = store.name(element) else {
            continue;
        };
        let (name, ok) = ast::try_get_text_of_property_name(store, name);
        if ok && name == key {
            return Some(element);
        }
    }
    None
}

pub struct ParseJsonSourceFileConfigFileContentInput<'a> {
    pub source_file: TsConfigSourceFile,
    pub host: &'a dyn ParseConfigHost,
    pub base_path: &'a str,
    pub existing_options: Option<&'a core::CompilerOptions>,
    pub existing_options_raw: Option<&'a Value>,
    pub config_file_name: &'a str,
    pub resolution_stack: &'a [tspath::Path],
    pub extra_file_extensions: &'a [FileExtensionInfo],
    pub extended_config_cache: Option<&'a dyn ExtendedConfigCache>,
}

pub fn parse_json_source_file_config_file_content(
    input: ParseJsonSourceFileConfigFileContentInput<'_>,
) -> ParsedCommandLine {
    let ParseJsonSourceFileConfigFileContentInput {
        source_file,
        host,
        base_path,
        existing_options,
        existing_options_raw,
        config_file_name,
        resolution_stack,
        extra_file_extensions,
        extended_config_cache,
    } = input;

    parse_json_config_file_content_worker(ParseJsonConfigFileContentWorkerInput {
        json: None,
        source_file: Some(source_file),
        host,
        base_path,
        existing_options,
        existing_options_raw,
        config_file_name,
        resolution_stack,
        extra_file_extensions,
        extended_config_cache,
    })
}

fn get_default_compiler_options(config_file_name: &str) -> core::CompilerOptions {
    let mut options = core::CompilerOptions::default();
    if !config_file_name.is_empty()
        && tspath::get_base_file_name(config_file_name) == "jsconfig.json"
    {
        options.allow_js = core::TS_TRUE;
        options.max_node_module_js_depth = Some(2);
        options.skip_lib_check = core::TS_TRUE;
        options.no_emit = core::TS_TRUE;
    }
    options
}

fn get_default_type_acquisition(config_file_name: &str) -> core::TypeAcquisition {
    let mut options = core::TypeAcquisition::default();
    if !config_file_name.is_empty()
        && tspath::get_base_file_name(config_file_name) == "jsconfig.json"
    {
        options.enable = core::TS_TRUE;
    }
    options
}

fn convert_compiler_options_from_json_worker(
    json_options: Option<&Value>,
    base_path: &str,
    config_file_name: &str,
) -> (core::CompilerOptions, Vec<String>) {
    let mut options = get_default_compiler_options(config_file_name);
    let mut errors = Vec::new();
    if let Some(json_options) = json_options.and_then(Value::as_object) {
        let option_map = command_line_options_to_map(options_declarations());
        for (key, value) in json_options {
            let Some(option) = option_map
                .get(key)
                .or_else(|| option_map.get(&key.to_lowercase()))
            else {
                errors.push(format!("Unknown_compiler_option_0: {key}"));
                continue;
            };
            if option.name != *key {
                errors.push(format!(
                    "Unknown_compiler_option_0_Did_you_mean_1: {key}\u{1f}{}",
                    option.name
                ));
                continue;
            }
            if option.kind == Some(CommandLineOptionKind::Enum)
                && let Some(value) = value.as_str()
                && !value.is_empty()
                && option
                    .enum_map()
                    .is_some_and(|enum_map| !enum_map.contains_key(&value.to_lowercase()))
            {
                errors.push(crate::commandlineparser::invalid_enum_error(option));
            }
        }
        let parsed = ParsedCommandLine {
            options: json_object_to_string_map(json_options),
            ..ParsedCommandLine::default()
        }
        .compiler_options();
        options = merge_compiler_options(options, parsed, None);
        normalize_compiler_option_paths(&mut options, base_path);
    }
    if !config_file_name.is_empty() {
        options.config_file_path = tspath::normalize_slashes(config_file_name);
    }
    (options, errors)
}

fn normalize_compiler_option_paths(options: &mut core::CompilerOptions, base_path: &str) {
    normalize_compiler_option_path(&mut options.generate_cpu_profile, base_path);
    normalize_compiler_option_path(&mut options.generate_trace, base_path);
    normalize_compiler_option_path(&mut options.out_file, base_path);
    normalize_compiler_option_path(&mut options.out_dir, base_path);
    normalize_compiler_option_path(&mut options.project, base_path);
    normalize_compiler_option_path(&mut options.root_dir, base_path);
    normalize_compiler_option_path(&mut options.ts_build_info_file, base_path);
    normalize_compiler_option_path(&mut options.base_url, base_path);
    normalize_compiler_option_path(&mut options.declaration_dir, base_path);
    normalize_compiler_option_path_list(&mut options.root_dirs, base_path);
    if options.type_roots_configured {
        normalize_compiler_option_path_list(&mut options.type_roots, base_path);
    }
}

fn normalize_compiler_option_path(value: &mut String, base_path: &str) {
    if value.is_empty() || starts_with_config_dir_template(value) {
        return;
    }
    *value = tspath::get_normalized_absolute_path(value, base_path);
    if value.is_empty() {
        *value = ".".to_owned();
    }
}

fn normalize_compiler_option_path_list(values: &mut [String], base_path: &str) {
    for value in values {
        normalize_compiler_option_path(value, base_path);
    }
}

fn convert_type_acquisition_from_json_worker(
    json_options: Option<&Value>,
    _base_path: &str,
    config_file_name: &str,
) -> (core::TypeAcquisition, Vec<String>) {
    let mut options = get_default_type_acquisition(config_file_name);
    if let Some(json_options) = json_options
        && let Ok(parsed) = serde_json::from_value::<core::TypeAcquisition>(json_options.clone())
    {
        options = parsed;
    }
    (options, Vec::new())
}

fn parse_own_config_of_json(
    json: Value,
    host: &dyn ParseConfigHost,
    base_path: &str,
    config_file_name: &str,
) -> (ParsedTsconfig, Vec<String>) {
    let mut errors = Vec::new();
    if json.get("excludes").is_some() {
        errors.push("Unknown option 'excludes'. Did you mean 'exclude'?".to_owned());
    }
    let (options, option_errors) = convert_compiler_options_from_json_worker(
        json.get("compilerOptions"),
        base_path,
        config_file_name,
    );
    let (type_acquisition, type_acquisition_errors) = convert_type_acquisition_from_json_worker(
        json.get("typeAcquisition"),
        base_path,
        config_file_name,
    );
    errors.extend(option_errors);
    errors.extend(type_acquisition_errors);
    let extended_config_path = json
        .get("extends")
        .filter(|value| !value.is_null())
        .map(|extends| {
            get_extends_config_path_or_array_value(extends, host, base_path, config_file_name)
        })
        .unwrap_or_default();
    (
        ParsedTsconfig {
            raw: json,
            options,
            type_acquisition,
            extended_config_path,
        },
        errors,
    )
}

fn parse_config_worker(
    json: Value,
    source_file: Option<&TsConfigSourceFile>,
    host: &dyn ParseConfigHost,
    base_path: &str,
    config_file_name: &str,
    resolution_stack: &[String],
    extended_config_cache: Option<&dyn ExtendedConfigCache>,
) -> (ParsedTsconfig, Vec<String>, BTreeSet<String>) {
    let base_path = tspath::normalize_slashes(base_path);
    let resolved_path = tspath::get_normalized_absolute_path(config_file_name, &base_path);
    if resolution_stack.contains(&resolved_path) {
        return (
            ParsedTsconfig {
                raw: json,
                ..ParsedTsconfig::default()
            },
            vec!["Circularity detected while resolving configuration".to_owned()],
            BTreeSet::new(),
        );
    }

    let (mut own_config, mut errors) =
        parse_own_config_of_json(json, host, &base_path, config_file_name);
    if !own_config.options.paths.is_empty() {
        own_config.options.paths_base_path = base_path.clone();
    }

    let mut extended_source_files = BTreeSet::new();
    if !own_config.extended_config_path.is_empty() {
        let mut next_stack = resolution_stack.to_vec();
        next_stack.push(resolved_path);
        let mut result = ExtendsResult::default();
        for extended_config_path in own_config.extended_config_path.clone() {
            let (extended_config, extended_errors) = get_extended_config(
                source_file,
                &extended_config_path,
                host,
                &next_stack,
                extended_config_cache,
                &mut result,
            );
            errors.extend(extended_errors);
            if let Some(extended_config) = extended_config {
                apply_extended_config(
                    &mut own_config,
                    &extended_config,
                    &extended_config_path,
                    &base_path,
                    host,
                    &mut result,
                );
            }
        }
        own_config.options =
            merge_compiler_options(result.options, own_config.options, Some(&own_config.raw));
        extended_source_files = result.extended_source_files;
    }
    (own_config, errors, extended_source_files)
}

struct ParseJsonConfigFileContentWorkerInput<'a> {
    json: Option<Value>,
    source_file: Option<TsConfigSourceFile>,
    host: &'a dyn ParseConfigHost,
    base_path: &'a str,
    existing_options: Option<&'a core::CompilerOptions>,
    existing_options_raw: Option<&'a Value>,
    config_file_name: &'a str,
    resolution_stack: &'a [tspath::Path],
    extra_file_extensions: &'a [FileExtensionInfo],
    extended_config_cache: Option<&'a dyn ExtendedConfigCache>,
}

fn parse_json_config_file_content_worker(
    input: ParseJsonConfigFileContentWorkerInput<'_>,
) -> ParsedCommandLine {
    let ParseJsonConfigFileContentWorkerInput {
        json,
        source_file,
        host,
        base_path,
        existing_options,
        existing_options_raw,
        config_file_name,
        resolution_stack,
        extra_file_extensions,
        extended_config_cache,
    } = input;

    debug_assert!(json.is_some() ^ source_file.is_some());
    let base_path_for_file_names = if config_file_name.is_empty() {
        tspath::normalize_path(base_path)
    } else {
        tspath::normalize_path(&directory_of_combined_path(config_file_name, base_path))
    };
    let json = json
        .or_else(|| {
            source_file
                .as_ref()
                .map(|source_file| source_file.json.clone())
        })
        .unwrap_or(Value::Object(Default::default()));
    let resolution_stack_string = Vec::new();
    let (mut parsed_config, mut errors, extended_source_files) = parse_config_worker(
        json,
        source_file.as_ref(),
        host,
        base_path,
        config_file_name,
        &resolution_stack_string,
        extended_config_cache,
    );
    let mut ast_diagnostics = if let Some(source_file) = source_file.as_ref() {
        source_file.ast_diagnostics.clone()
    } else {
        Vec::new()
    };
    if let Some(source_file) = source_file.as_ref() {
        let source_option_diagnostics =
            get_source_file_compiler_option_diagnostics(&source_file.source_file);
        if !source_option_diagnostics.is_empty() {
            errors.retain(|error| {
                !error.starts_with("Argument_for_0_option_must_be_Colon_1: ")
                    && !error.starts_with("Unknown_compiler_option_0: ")
                    && !error.starts_with("Unknown_compiler_option_0_Did_you_mean_1: ")
            });
            ast_diagnostics.extend(source_option_diagnostics);
        }
    }
    if let Some(existing_options) = existing_options {
        parsed_config.options = merge_compiler_options(
            parsed_config.options,
            existing_options.clone(),
            existing_options_raw,
        );
    }
    handle_option_config_dir_template_substitution(
        &mut parsed_config.options,
        &base_path_for_file_names,
    );
    if !config_file_name.is_empty() {
        parsed_config.options.config_file_path = tspath::normalize_slashes(config_file_name);
    }

    let raw_config = parse_json_to_string_key(&parsed_config.raw);
    let references_of_raw = get_prop_from_raw(
        &raw_config,
        "references",
        value_is_object,
        "object",
        &mut errors,
    );
    let file_specs = get_prop_from_raw(
        &raw_config,
        "files",
        Value::is_string,
        "string",
        &mut errors,
    );
    if let Some(files) = &file_specs.slice_value
        && files.is_empty()
        && (references_of_raw.wrong_value == "no-prop"
            || references_of_raw
                .slice_value
                .as_ref()
                .is_none_or(Vec::is_empty))
        && !raw_config.contains_key("extends")
    {
        errors.push(format!(
            "The files list in config file {config_file_name} is empty"
        ));
    }

    let mut include_specs = get_prop_from_raw(
        &raw_config,
        "include",
        Value::is_string,
        "string",
        &mut errors,
    );
    let mut exclude_specs = get_prop_from_raw(
        &raw_config,
        "exclude",
        Value::is_string,
        "string",
        &mut errors,
    );
    let mut is_default_include_spec = false;
    if exclude_specs.wrong_value == "no-prop" {
        let mut values = Vec::new();
        if !parsed_config.options.out_dir.is_empty() {
            values.push(Value::String(parsed_config.options.out_dir.clone()));
        }
        if !parsed_config.options.declaration_dir.is_empty() {
            values.push(Value::String(parsed_config.options.declaration_dir.clone()));
        }
        if !values.is_empty() {
            exclude_specs = PropOfRaw {
                slice_value: Some(values),
                wrong_value: String::new(),
            };
        }
    }
    if file_specs.slice_value.is_none() && include_specs.slice_value.is_none() {
        include_specs = PropOfRaw {
            slice_value: Some(vec![Value::String(DEFAULT_INCLUDE_SPEC.to_owned())]),
            wrong_value: String::new(),
        };
        is_default_include_spec = true;
    }

    let mut validated_include_specs_before_substitution = Vec::new();
    let mut validated_include_specs = Vec::new();
    if let Some(specs) = &include_specs.slice_value {
        validated_include_specs_before_substitution =
            validate_specs(specs, true, "include", &mut errors);
        validated_include_specs = get_substituted_string_array_with_config_dir_template(
            &validated_include_specs_before_substitution,
            &base_path_for_file_names,
        )
        .unwrap_or_else(|| validated_include_specs_before_substitution.clone());
    }
    let mut validated_exclude_specs = Vec::new();
    if let Some(specs) = &exclude_specs.slice_value {
        validated_exclude_specs = validate_specs(specs, false, "exclude", &mut errors);
        if let Some(substituted) = get_substituted_string_array_with_config_dir_template(
            &validated_exclude_specs,
            &base_path_for_file_names,
        ) {
            validated_exclude_specs = substituted;
        }
    }
    let mut validated_files_spec_before_substitution = Vec::new();
    let mut validated_files_spec = Vec::new();
    if let Some(specs) = &file_specs.slice_value {
        validated_files_spec_before_substitution = specs
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        validated_files_spec = get_substituted_string_array_with_config_dir_template(
            &validated_files_spec_before_substitution,
            &base_path_for_file_names,
        )
        .unwrap_or_else(|| validated_files_spec_before_substitution.clone());
    }

    let config_file_specs = ConfigFileSpecs {
        files_specs: file_specs.slice_value.clone(),
        include_specs: include_specs.slice_value.clone(),
        exclude_specs: exclude_specs.slice_value.clone(),
        validated_files_spec,
        validated_include_specs,
        validated_exclude_specs,
        validated_files_spec_before_substitution,
        validated_include_specs_before_substitution,
        is_default_include_spec,
    };
    let mut config_file = source_file.unwrap_or_else(|| {
        let path = tspath::to_path(
            config_file_name,
            base_path,
            host.fs().use_case_sensitive_file_names(),
        );
        new_empty_tsconfig_source_file(
            config_file_name,
            path,
            parsed_config.raw.to_string(),
            parsed_config.raw.clone(),
        )
    });
    config_file.ast_diagnostics.extend(ast_diagnostics);
    config_file.config_file_specs = Some(config_file_specs.clone());
    for extended_config_path in &parsed_config.extended_config_path {
        if !config_file
            .extended_source_files
            .contains(extended_config_path)
        {
            config_file
                .extended_source_files
                .push(extended_config_path.clone());
        }
    }
    for extended_source_file in extended_source_files {
        if !config_file
            .extended_source_files
            .contains(&extended_source_file)
        {
            config_file.extended_source_files.push(extended_source_file);
        }
    }
    let parsed_compiler_options = parsed_config.options.clone();
    let (file_names, literal_file_names_len) = get_file_names_from_config_specs(
        &config_file_specs,
        &base_path_for_file_names,
        &parsed_compiler_options,
        host.fs(),
        extra_file_extensions,
    );
    if should_report_no_input_files(
        &file_names,
        can_json_report_no_input_files(&raw_config),
        resolution_stack,
    ) {
        let include_specs = config_file_specs.include_specs.clone().unwrap_or_default();
        let exclude_specs = config_file_specs.exclude_specs.clone().unwrap_or_default();
        errors.push(format!(
            "No_inputs_were_found_in_config_file_0_Specified_include_paths_were_1_and_exclude_paths_were_2: {config_file_name}\u{1f}{}\u{1f}{}",
            Value::Array(include_specs),
            Value::Array(exclude_specs),
        ));
    }

    let mut options = compiler_options_to_string_map(&parsed_compiler_options);
    if !parsed_compiler_options.paths_base_path.is_empty() {
        options.insert(
            "pathsBasePath".to_owned(),
            parsed_compiler_options.paths_base_path.clone(),
        );
    }

    ParsedCommandLine {
        options,
        parsed_compiler_options: Some(parsed_compiler_options),
        type_acquisition: json_object_to_string_map_from_value(
            parsed_config.raw.get("typeAcquisition"),
        ),
        file_names,
        project_references: get_project_references(
            &raw_config,
            &base_path_for_file_names,
            &mut errors,
        ),
        config_file: Some(config_file),
        errors,
        include_specs: config_file_specs.validated_include_specs,
        exclude_specs: config_file_specs.validated_exclude_specs,
        extra_file_extensions: extra_file_extensions.to_vec(),
        compile_on_save: raw_config
            .get("compileOnSave")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        raw: Some(parsed_config.raw.to_string()),
        config_file_path: config_file_name.to_owned(),
        current_directory: base_path_for_file_names,
        use_case_sensitive_file_names: host.fs().use_case_sensitive_file_names(),
        literal_file_names_len,
        ..ParsedCommandLine::default()
    }
}

fn merge_compiler_options(
    mut base: core::CompilerOptions,
    overrides: core::CompilerOptions,
    raw_overrides: Option<&Value>,
) -> core::CompilerOptions {
    let base_paths_for_validation = base.paths_for_validation.clone();
    let override_paths_for_validation = overrides.paths_for_validation.clone();
    let override_paths_is_empty = overrides.paths.is_empty();
    let overrides_specify_paths =
        !override_paths_is_empty || !override_paths_for_validation.is_empty();
    let Ok(mut base_value) = serde_json::to_value(&base) else {
        return overrides;
    };
    let Ok(override_value) = serde_json::to_value(&overrides) else {
        return base;
    };
    let override_type_roots_configured = overrides.type_roots_configured;
    let override_type_roots = overrides.type_roots.clone();
    let override_charset = overrides.charset.clone();
    let override_target_is_es3 = overrides.target_is_es3;
    merge_json_objects(&mut base_value, override_value);
    apply_explicit_null_compiler_options(&mut base_value, raw_overrides);
    base = serde_json::from_value(base_value).unwrap_or(overrides);
    if overrides_specify_paths {
        base.paths_for_validation = override_paths_for_validation;
        if base.paths_for_validation.size() > 0 && override_paths_is_empty {
            base.paths.clear();
        }
    } else {
        base.paths_for_validation = base_paths_for_validation;
    }
    if override_type_roots_configured {
        base.type_roots = override_type_roots;
        base.type_roots_configured = true;
    }
    if !override_charset.is_empty() {
        base.charset = override_charset;
    }
    if override_target_is_es3 {
        base.target_is_es3 = true;
    }
    base
}

fn apply_explicit_null_compiler_options(target: &mut Value, raw_overrides: Option<&Value>) {
    let Some(Value::Object(raw)) = raw_overrides else {
        return;
    };
    let Some(Value::Object(compiler_options_raw)) = raw.get("compilerOptions") else {
        return;
    };
    let Ok(Value::Object(defaults)) = serde_json::to_value(core::CompilerOptions::default()) else {
        return;
    };
    let Some(target) = target.as_object_mut() else {
        return;
    };
    for (key, value) in compiler_options_raw {
        if value.is_null() {
            if let Some(default_value) = defaults.get(key) {
                target.insert(key.clone(), default_value.clone());
            } else {
                target.remove(key);
            }
        }
    }
}

fn merge_json_objects(target: &mut Value, source: Value) {
    let (Some(target), Value::Object(source)) = (target.as_object_mut(), source) else {
        return;
    };
    let defaults_value =
        serde_json::to_value(core::CompilerOptions::default()).expect("serialize default options");
    let defaults = defaults_value.as_object().expect("default options object");
    for (key, value) in source {
        if !value.is_null() && defaults.get(&key) != Some(&value) {
            target.insert(key, value);
        }
    }
}

fn get_extends_config_path_or_array_value(
    value: &Value,
    host: &dyn ParseConfigHost,
    base_path: &str,
    config_file_name: &str,
) -> Vec<String> {
    let new_base = if config_file_name.is_empty() {
        base_path.to_owned()
    } else {
        directory_of_combined_path(config_file_name, base_path)
    };
    match value {
        Value::String(path) => get_extends_config_path(path, host, &new_base)
            .into_iter()
            .collect(),
        Value::Array(paths) => paths
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|path| get_extends_config_path(path, host, &new_base))
            .collect(),
        _ => Vec::new(),
    }
}

fn get_extends_config_path(
    extended_config: &str,
    host: &dyn ParseConfigHost,
    base_path: &str,
) -> Option<String> {
    let extended_config = tspath::normalize_slashes(extended_config);
    if tspath::is_rooted_disk_path(&extended_config)
        || extended_config.starts_with("./")
        || extended_config.starts_with("../")
    {
        let mut extended_config_path =
            tspath::get_normalized_absolute_path(&extended_config, base_path);
        if !host.fs().file_exists(&extended_config_path)
            && !extended_config_path.ends_with(tspath::EXTENSION_JSON)
        {
            extended_config_path.push_str(tspath::EXTENSION_JSON);
            if !host.fs().file_exists(&extended_config_path) {
                return None;
            }
        }
        return Some(extended_config_path);
    }
    let resolver_host = ResolverHost { host };
    module::resolve_config(
        &extended_config,
        &tspath::combine_paths(base_path, &["tsconfig.json"]),
        &resolver_host,
    )
    .map(|resolved| resolved.resolved_file_name)
}

fn read_json_config_file(
    file_name: &str,
    path: tspath::Path,
    read_file: impl Fn(&str) -> (String, bool),
) -> (TsConfigSourceFile, Vec<String>) {
    let (text, ok) = read_file(file_name);
    if ok {
        let source_file = new_tsconfig_source_file_from_file_path(file_name, path, &text);
        let diagnostics = source_file.diagnostics.clone();
        (source_file, diagnostics)
    } else {
        (
            new_empty_tsconfig_source_file(file_name, path, String::new(), Value::Null),
            vec![format!("Cannot_read_file_0: {file_name}")],
        )
    }
}

fn get_extended_config(
    source_file: Option<&TsConfigSourceFile>,
    extended_config_file_name: &str,
    host: &dyn ParseConfigHost,
    resolution_stack: &[String],
    extended_config_cache: Option<&dyn ExtendedConfigCache>,
    result: &mut ExtendsResult,
) -> (Option<ParsedTsconfig>, Vec<String>) {
    let extended_config_path = tspath::to_path(
        extended_config_file_name,
        &host.get_current_directory(),
        host.fs().use_case_sensitive_file_names(),
    );
    let cache_entry = if let Some(cache) = extended_config_cache {
        cache.get_extended_config(
            extended_config_file_name.to_owned(),
            extended_config_path,
            resolution_stack.to_vec(),
            host,
        )
    } else {
        parse_extended_config(
            extended_config_file_name,
            extended_config_path,
            resolution_stack,
            host,
            extended_config_cache,
        )
    };
    if source_file.is_some()
        && let Some(extended_result) = &cache_entry.extended_result
    {
        result
            .extended_source_files
            .insert(extended_result.file_name.clone());
        result
            .extended_source_files
            .extend(extended_result.extended_source_files.iter().cloned());
    }
    (cache_entry.extended_config, cache_entry.errors)
}

pub fn parse_extended_config(
    file_name: &str,
    path: tspath::Path,
    resolution_stack: &[String],
    host: &dyn ParseConfigHost,
    extended_config_cache: Option<&dyn ExtendedConfigCache>,
) -> ExtendedConfigCacheEntry {
    let (mut extended_result, mut entry_errors) =
        read_json_config_file(file_name, path, |file_name| host.fs().read_file(file_name));
    let extended_config = if extended_result.diagnostics.is_empty() {
        let (config, errors, extended_source_files) = parse_config_worker(
            extended_result.json.clone(),
            Some(&extended_result),
            host,
            &tspath::get_directory_path(file_name),
            &tspath::get_base_file_name(file_name),
            resolution_stack,
            extended_config_cache,
        );
        entry_errors.extend(errors);
        extended_result.extended_source_files = extended_source_files.into_iter().collect();
        Some(config)
    } else {
        None
    };
    ExtendedConfigCacheEntry {
        extended_result: Some(extended_result),
        extended_config,
        errors: entry_errors,
    }
}

fn apply_extended_config(
    own_config: &mut ParsedTsconfig,
    extended_config: &ParsedTsconfig,
    extended_config_path: &str,
    base_path: &str,
    host: &dyn ParseConfigHost,
    result: &mut ExtendsResult,
) {
    let mut relative_difference = String::new();
    for property_name in ["include", "exclude", "files"] {
        if own_config.raw.get(property_name).is_some() {
            continue;
        }
        let Some(Value::Array(slice)) = extended_config.raw.get(property_name) else {
            continue;
        };
        let value = slice
            .iter()
            .filter_map(Value::as_str)
            .map(|path| {
                if starts_with_config_dir_template(path) || tspath::is_rooted_disk_path(path) {
                    Value::String(path.to_owned())
                } else {
                    if relative_difference.is_empty() {
                        relative_difference = tspath::convert_to_relative_path(
                            &tspath::get_directory_path(extended_config_path),
                            &tspath::ComparePathsOptions {
                                use_case_sensitive_file_names: host
                                    .fs()
                                    .use_case_sensitive_file_names(),
                                current_directory: base_path.to_owned(),
                            },
                        );
                    }
                    Value::String(tspath::combine_paths(&relative_difference, &[path]))
                }
            })
            .collect::<Vec<_>>();
        match property_name {
            "include" => result.include = Some(value.clone()),
            "exclude" => result.exclude = Some(value.clone()),
            "files" => result.files = Some(value.clone()),
            _ => {}
        }
        if let Some(raw) = own_config.raw.as_object_mut() {
            raw.insert(property_name.to_owned(), Value::Array(value));
        }
    }
    if let Some(compile_on_save) = extended_config
        .raw
        .get("compileOnSave")
        .and_then(Value::as_bool)
    {
        result.compile_on_save = compile_on_save;
        if own_config.raw.get("compileOnSave").is_none()
            && let Some(raw) = own_config.raw.as_object_mut()
        {
            raw.insert("compileOnSave".to_owned(), Value::Bool(compile_on_save));
        }
    }
    result.options = merge_compiler_options(
        result.options.clone(),
        extended_config.options.clone(),
        Some(&extended_config.raw),
    );
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PropOfRaw {
    slice_value: Option<Vec<Value>>,
    wrong_value: String,
}

fn value_is_object(value: &Value) -> bool {
    value.is_object()
}

fn get_prop_from_raw(
    raw_config: &BTreeMap<String, Value>,
    prop: &str,
    validate_element: fn(&Value) -> bool,
    element_type_name: &str,
    errors: &mut Vec<String>,
) -> PropOfRaw {
    let Some(value) = raw_config.get(prop) else {
        return PropOfRaw {
            slice_value: None,
            wrong_value: "no-prop".to_owned(),
        };
    };
    if value.is_null() {
        return PropOfRaw {
            slice_value: None,
            wrong_value: "no-prop".to_owned(),
        };
    }
    if let Value::Array(values) = value {
        if !values.iter().all(validate_element) {
            errors.push(format!(
                "Compiler option {prop} requires a value of type {element_type_name}"
            ));
        }
        return PropOfRaw {
            slice_value: Some(values.clone()),
            wrong_value: String::new(),
        };
    }
    errors.push(format!(
        "Compiler option {prop} requires a value of type Array"
    ));
    PropOfRaw {
        slice_value: None,
        wrong_value: "not-array".to_owned(),
    }
}

const DEFAULT_INCLUDE_SPEC: &str = "**/*";

fn parse_json_to_string_key(json: &Value) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    let Some(map) = json.as_object() else {
        return result;
    };
    for key in root_option_keys() {
        if let Some(value) = map.get(key) {
            if key == "extends"
                && let Some(path) = value.as_str()
            {
                result.insert(
                    key.to_owned(),
                    Value::Array(vec![Value::String(path.to_owned())]),
                );
                continue;
            }
            result.insert(key.to_owned(), value.clone());
        }
    }
    for key in legacy_root_option_keys() {
        if let Some(value) = map.get(key) {
            result.insert(key.to_owned(), value.clone());
        }
    }
    result
}

fn can_json_report_no_input_files(raw_config: &BTreeMap<String, Value>) -> bool {
    !raw_config.contains_key("files") && !raw_config.contains_key("references")
}

fn should_report_no_input_files(
    file_names: &[String],
    can_json_report_no_input_files: bool,
    resolution_stack: &[tspath::Path],
) -> bool {
    file_names.is_empty() && can_json_report_no_input_files && resolution_stack.is_empty()
}

fn validate_specs(
    specs: &[Value],
    disallow_trailing_recursion: bool,
    spec_key: &str,
    errors: &mut Vec<String>,
) -> Vec<String> {
    let mut final_specs = Vec::new();
    for spec in specs.iter().filter_map(Value::as_str) {
        if spec_to_diagnostic(spec, disallow_trailing_recursion).is_some() {
            errors.push(format!("Invalid {spec_key} file specification: {spec}"));
        } else {
            final_specs.push(spec.to_owned());
        }
    }
    final_specs
}

fn spec_to_diagnostic(spec: &str, disallow_trailing_recursion: bool) -> Option<&'static str> {
    if disallow_trailing_recursion && invalid_trailing_recursion(spec) {
        Some("File specification cannot end in a recursive directory wildcard")
    } else if !disallow_trailing_recursion && invalid_dot_dot_after_recursive_wildcard(spec) {
        Some(
            "File specification cannot contain a parent directory after a recursive directory wildcard",
        )
    } else {
        None
    }
}

fn handle_option_config_dir_template_substitution(
    compiler_options: &mut core::CompilerOptions,
    base_path: &str,
) {
    let path_substitutions = compiler_options
        .paths
        .entries()
        .filter_map(|(key, value)| {
            get_substituted_string_array_with_config_dir_template(value, base_path)
                .map(|substitution| (key.clone(), substitution))
        })
        .collect::<Vec<_>>();
    for (key, substitution) in path_substitutions {
        compiler_options.paths.set(key, substitution);
    }
    substitute_config_dir_template(&mut compiler_options.root_dirs, base_path);
    if compiler_options.type_roots_configured {
        substitute_config_dir_template(&mut compiler_options.type_roots, base_path);
    }
    substitute_config_dir_template_string(&mut compiler_options.generate_cpu_profile, base_path);
    substitute_config_dir_template_string(&mut compiler_options.generate_trace, base_path);
    substitute_config_dir_template_string(&mut compiler_options.out_file, base_path);
    substitute_config_dir_template_string(&mut compiler_options.out_dir, base_path);
    substitute_config_dir_template_string(&mut compiler_options.root_dir, base_path);
    substitute_config_dir_template_string(&mut compiler_options.ts_build_info_file, base_path);
    substitute_config_dir_template_string(&mut compiler_options.base_url, base_path);
    substitute_config_dir_template_string(&mut compiler_options.declaration_dir, base_path);
}

fn substitute_config_dir_template(values: &mut Vec<String>, base_path: &str) {
    if let Some(substitution) =
        get_substituted_string_array_with_config_dir_template(values, base_path)
    {
        *values = substitution;
    }
}

fn substitute_config_dir_template_string(value: &mut String, base_path: &str) {
    if starts_with_config_dir_template(value) {
        *value = get_substituted_path_with_config_dir_template(value, base_path);
    }
}

fn has_file_with_higher_priority_extension(
    file: &str,
    extensions: &[Vec<String>],
    has_file: impl Fn(&str) -> bool,
) -> bool {
    let extension_group = extensions
        .iter()
        .find(|group| {
            group
                .iter()
                .any(|extension| tspath::file_extension_is(file, extension))
        })
        .cloned()
        .unwrap_or_default();
    if extension_group.is_empty() {
        return false;
    }
    for ext in extension_group {
        if tspath::file_extension_is(file, &ext)
            && (ext != tspath::EXTENSION_TS
                || !tspath::file_extension_is(file, tspath::EXTENSION_DTS))
        {
            return false;
        }
        if has_file(&tspath::change_extension(file, &ext)) {
            if ext == tspath::EXTENSION_DTS
                && (tspath::file_extension_is(file, tspath::EXTENSION_JS)
                    || tspath::file_extension_is(file, tspath::EXTENSION_JSX))
            {
                continue;
            }
            return true;
        }
    }
    false
}

fn remove_wildcard_files_with_lower_priority_extension(
    file: &str,
    wildcard_files: &mut Vec<(String, String)>,
    extensions: &[Vec<String>],
    key_mapper: impl Fn(&str) -> String,
) {
    let extension_group = extensions
        .iter()
        .find(|group| {
            group
                .iter()
                .any(|extension| tspath::file_extension_is(file, extension))
        })
        .cloned()
        .unwrap_or_default();
    for ext in extension_group.iter().rev() {
        if tspath::file_extension_is(file, ext) {
            return;
        }
        let lower_priority_path = key_mapper(&tspath::change_extension(file, ext));
        wildcard_files.retain(|(key, _)| key != &lower_priority_path);
    }
}

pub(crate) fn get_file_names_from_config_specs(
    config_file_specs: &ConfigFileSpecs,
    base_path: &str,
    options: &core::CompilerOptions,
    host: &dyn vfs::Fs,
    extra_file_extensions: &[FileExtensionInfo],
) -> (Vec<String>, usize) {
    let base_path = tspath::normalize_path(base_path);
    let key_mapper =
        |value: &str| tspath::get_canonical_file_name(value, host.use_case_sensitive_file_names());
    let mut literal_file_map: Vec<(String, String)> = Vec::new();
    let mut wildcard_file_map: Vec<(String, String)> = Vec::new();
    let mut wildcard_json_file_map: Vec<(String, String)> = Vec::new();
    let supported_extensions = get_supported_extensions(options, extra_file_extensions);
    let supported_extensions_with_json = get_supported_extensions_with_json_if_resolve_json_module(
        Some(options),
        supported_extensions.clone(),
    );
    for file_name in &config_file_specs.validated_files_spec {
        let file = tspath::get_normalized_absolute_path(file_name, &base_path);
        insert_ordered_unique(&mut literal_file_map, key_mapper(file_name), file);
    }
    if !config_file_specs.validated_include_specs.is_empty() {
        let flat_extensions = supported_extensions_with_json
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let files = vfsmatch::read_directory(
            host,
            &base_path,
            &base_path,
            &flat_extensions,
            &config_file_specs.validated_exclude_specs,
            &config_file_specs.validated_include_specs,
            vfsmatch::UNLIMITED_DEPTH,
        );
        let mut json_only_include_matchers = None;
        for file in files {
            if tspath::file_extension_is(&file, tspath::EXTENSION_JSON) {
                if json_only_include_matchers.is_none() {
                    let includes = config_file_specs
                        .validated_include_specs
                        .iter()
                        .filter(|include| include.ends_with(tspath::EXTENSION_JSON))
                        .cloned()
                        .collect::<Vec<_>>();
                    json_only_include_matchers = vfsmatch::new_spec_matcher(
                        &includes,
                        &base_path,
                        vfsmatch::Usage::Files,
                        host.use_case_sensitive_file_names(),
                    );
                }
                if json_only_include_matchers
                    .as_ref()
                    .and_then(|matcher| matcher.match_index(&file))
                    .is_some()
                {
                    let key = key_mapper(&file);
                    if !map_has_key(&literal_file_map, &key)
                        && !map_has_key(&wildcard_json_file_map, &key)
                    {
                        wildcard_json_file_map.push((key, file));
                    }
                }
                continue;
            }
            if has_file_with_higher_priority_extension(&file, &supported_extensions, |file_name| {
                let canonical_file_name = key_mapper(file_name);
                map_has_key(&literal_file_map, &canonical_file_name)
                    || map_has_key(&wildcard_file_map, &canonical_file_name)
            }) {
                continue;
            }
            remove_wildcard_files_with_lower_priority_extension(
                &file,
                &mut wildcard_file_map,
                &supported_extensions,
                key_mapper,
            );
            let key = key_mapper(&file);
            if !map_has_key(&literal_file_map, &key) && !map_has_key(&wildcard_file_map, &key) {
                wildcard_file_map.push((key, file));
            }
        }
    }
    let literal_file_names_len = literal_file_map.len();
    let mut files = Vec::with_capacity(
        literal_file_map.len() + wildcard_file_map.len() + wildcard_json_file_map.len(),
    );
    files.extend(literal_file_map.into_iter().map(|(_, file)| file));
    files.extend(wildcard_file_map.into_iter().map(|(_, file)| file));
    files.extend(wildcard_json_file_map.into_iter().map(|(_, file)| file));
    (files, literal_file_names_len)
}

fn insert_ordered_unique(map: &mut Vec<(String, String)>, key: String, value: String) {
    if let Some((_, existing)) = map
        .iter_mut()
        .find(|(existing_key, _)| existing_key == &key)
    {
        *existing = value;
    } else {
        map.push((key, value));
    }
}

fn map_has_key(map: &[(String, String)], key: &str) -> bool {
    map.iter().any(|(existing_key, _)| existing_key == key)
}

fn parse_project_reference(json: &Value) -> Vec<core::ProjectReference> {
    let Some(object) = json.as_object() else {
        return Vec::new();
    };
    vec![core::ProjectReference {
        path: object
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        original_path: String::new(),
        circular: object
            .get("circular")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }]
}

fn get_project_references(
    raw_config: &BTreeMap<String, Value>,
    base_path: &str,
    errors: &mut Vec<String>,
) -> Vec<core::ProjectReference> {
    let Some(Value::Array(references)) = raw_config.get("references") else {
        return Vec::new();
    };
    let mut project_references = Vec::new();
    for reference in references {
        for mut reference in parse_project_reference(reference) {
            if reference.path.is_empty() {
                errors.push(
                    "Compiler option reference.path requires a value of type string".to_owned(),
                );
            } else {
                reference.original_path = reference.path.clone();
                reference.path = tspath::get_normalized_absolute_path(&reference.path, base_path);
                project_references.push(reference);
            }
        }
    }
    project_references
}

pub fn get_parsed_command_line_of_config_file(
    config_file_name: &str,
    options: Option<&core::CompilerOptions>,
    options_raw: Option<&Value>,
    sys: &dyn ParseConfigHost,
    extended_config_cache: Option<&dyn ExtendedConfigCache>,
) -> (Option<ParsedCommandLine>, Vec<String>) {
    let config_file_name =
        tspath::get_normalized_absolute_path(config_file_name, &sys.get_current_directory());
    get_parsed_command_line_of_config_file_path(
        &config_file_name,
        tspath::to_path(
            &config_file_name,
            &sys.get_current_directory(),
            sys.fs().use_case_sensitive_file_names(),
        ),
        options,
        options_raw,
        sys,
        extended_config_cache,
    )
}

pub fn get_parsed_command_line_of_config_file_path(
    config_file_name: &str,
    path: tspath::Path,
    options: Option<&core::CompilerOptions>,
    options_raw: Option<&Value>,
    sys: &dyn ParseConfigHost,
    extended_config_cache: Option<&dyn ExtendedConfigCache>,
) -> (Option<ParsedCommandLine>, Vec<String>) {
    let (config_file_text, ok) = sys.fs().read_file(config_file_name);
    if !ok {
        return (
            None,
            vec![format!("Cannot_read_file_0: {config_file_name}")],
        );
    }
    let tsconfig_source_file =
        new_tsconfig_source_file_from_file_path(config_file_name, path, &config_file_text);
    let parsed =
        parse_json_source_file_config_file_content(ParseJsonSourceFileConfigFileContentInput {
            source_file: tsconfig_source_file,
            host: sys,
            base_path: &tspath::get_directory_path(config_file_name),
            existing_options: options,
            existing_options_raw: options_raw,
            config_file_name,
            resolution_stack: &[],
            extra_file_extensions: &[],
            extended_config_cache,
        });
    (Some(parsed), Vec::new())
}

pub fn get_supported_extensions(
    compiler_options: &core::CompilerOptions,
    extra_file_extensions: &[FileExtensionInfo],
) -> Vec<Vec<String>> {
    let need_js_extensions = compiler_options.get_allow_js();
    if extra_file_extensions.is_empty() {
        if need_js_extensions {
            return extension_groups_to_owned(tspath::ALL_SUPPORTED_EXTENSIONS);
        }
        return extension_groups_to_owned(tspath::SUPPORTED_TS_EXTENSIONS);
    }

    let builtins = if need_js_extensions {
        extension_groups_to_owned(tspath::ALL_SUPPORTED_EXTENSIONS)
    } else {
        extension_groups_to_owned(tspath::SUPPORTED_TS_EXTENSIONS)
    };
    let flat_builtins = builtins
        .iter()
        .flat_map(|extensions| extensions.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for extension in extra_file_extensions {
        if extension.script_kind == core::ScriptKind::Deferred
            || need_js_extensions
                && (extension.script_kind == core::ScriptKind::JS
                    || extension.script_kind == core::ScriptKind::JSX)
                && !flat_builtins.contains(&extension.extension)
        {
            result.push(vec![extension.extension.clone()]);
        }
    }
    let mut extensions = builtins;
    extensions.extend(result);
    extensions
}

pub fn get_supported_extensions_with_json_if_resolve_json_module(
    compiler_options: Option<&core::CompilerOptions>,
    supported_extensions: Vec<Vec<String>>,
) -> Vec<Vec<String>> {
    let Some(compiler_options) = compiler_options else {
        return supported_extensions;
    };
    if !compiler_options.get_resolve_json_module() {
        return supported_extensions;
    }
    if supported_extensions == extension_groups_to_owned(tspath::ALL_SUPPORTED_EXTENSIONS) {
        return tspath::all_supported_extensions_with_json()
            .into_iter()
            .map(|extensions| extensions.into_iter().map(str::to_owned).collect())
            .collect();
    }
    if supported_extensions == extension_groups_to_owned(tspath::SUPPORTED_TS_EXTENSIONS) {
        return tspath::supported_ts_extensions_with_json()
            .into_iter()
            .map(|extensions| extensions.into_iter().map(str::to_owned).collect())
            .collect();
    }
    let mut result = supported_extensions;
    result.push(vec![tspath::EXTENSION_JSON.to_string()]);
    result
}

fn extension_groups_to_owned(groups: &[&[&str]]) -> Vec<Vec<String>> {
    groups
        .iter()
        .map(|extensions| {
            extensions
                .iter()
                .map(|extension| (*extension).to_string())
                .collect()
        })
        .collect()
}

pub fn get_tsconfig_options_object(raw: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(raw) else {
        return result;
    };
    for key in root_option_keys() {
        if let Some(value) = object.get(key) {
            result.insert(key.to_owned(), value_to_config_string(value));
        }
    }
    for key in legacy_root_option_keys() {
        if let Some(value) = object.get(key) {
            result.insert(key.to_owned(), value_to_config_string(value));
        }
    }
    result
}

pub fn get_extends_configs_path_or_array(raw: &str) -> Vec<String> {
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    match object.get("extends") {
        Some(Value::String(path)) => vec![path.clone()],
        Some(Value::Array(paths)) => paths
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn root_option_keys() -> [&'static str; 8] {
    [
        "include",
        "exclude",
        "files",
        "references",
        "extends",
        "compilerOptions",
        "excludes",
        "typeAcquisition",
    ]
}

fn legacy_root_option_keys() -> [&'static str; 2] {
    ["watchOptions", "compileOnSave"]
}

fn config_dir_template() -> &'static str {
    "${configDir}"
}

fn starts_with_config_dir_template(value: &str) -> bool {
    value
        .to_ascii_lowercase()
        .starts_with(&config_dir_template().to_ascii_lowercase())
}

fn get_substituted_path_with_config_dir_template(value: &str, base_path: &str) -> String {
    tspath::get_normalized_absolute_path(&value.replacen(config_dir_template(), "./", 1), base_path)
}

fn get_substituted_string_array_with_config_dir_template(
    list: &[String],
    base_path: &str,
) -> Option<Vec<String>> {
    let mut result = None;
    for (index, element) in list.iter().enumerate() {
        if starts_with_config_dir_template(element) {
            let result = result.get_or_insert_with(|| list.to_vec());
            result[index] = get_substituted_path_with_config_dir_template(element, base_path);
        }
    }
    result
}

fn json_object_to_string_map(object: &serde_json::Map<String, Value>) -> BTreeMap<String, String> {
    object
        .iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, value)| (key.clone(), value_to_config_string(value)))
        .collect()
}

fn json_object_to_string_map_from_value(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(json_object_to_string_map)
        .unwrap_or_default()
}

fn string_array_property(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Option<Vec<String>> {
    object.get(key).and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}

fn value_to_config_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

pub fn invalid_trailing_recursion(spec: &str) -> bool {
    let spec = spec.strip_suffix('/').unwrap_or(spec);
    spec == "**" || spec.ends_with("/**")
}

pub fn invalid_dot_dot_after_recursive_wildcard(spec: &str) -> bool {
    let wildcard_index = if spec.starts_with("**/") {
        Some(0)
    } else {
        spec.find("/**/")
    };
    let Some(wildcard_index) = wildcard_index else {
        return false;
    };
    let last_dot_index = if spec.ends_with("/..") {
        Some(spec.len())
    } else {
        spec.rfind("/../")
    };
    last_dot_index.is_some_and(|last_dot_index| last_dot_index > wildcard_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_file_text_to_json_accepts_jsonc_trailing_commas() {
        let json_text = r#"{
  // accepted by the TypeScript-Go JSON parser
  "compilerOptions": {
    "composite": true,
    "target": "ES5",
  },
  "include": [
    "src/**/*.ts",
  ],
}"#;

        let (json, errors) =
            parse_config_file_text_to_json("/apath/tsconfig.json", &"/apath".into(), json_text);

        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(
            json["compilerOptions"]["target"],
            Value::String("ES5".to_owned())
        );
        assert_eq!(json["include"][0], Value::String("src/**/*.ts".to_owned()));
    }

    #[test]
    fn merge_compiler_options_explicit_null_clears_composite() {
        let base = core::CompilerOptions {
            composite: core::TS_TRUE,
            ..Default::default()
        };
        let overrides = core::CompilerOptions::default();
        let raw_overrides = serde_json::json!({
            "compilerOptions": {
                "composite": null
            }
        });

        let merged = merge_compiler_options(base, overrides, Some(&raw_overrides));

        assert!(merged.composite.is_unknown());
    }

    #[test]
    fn command_line_raw_null_clears_config_composite() {
        let config_file_name = "/project/tsconfig.json";
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            config_file_name.to_owned(),
            r#"{"compilerOptions":{"composite":true}}"#.to_owned(),
        );
        let host = crate::tsoptionstest::VfsParseConfigHost::new(files, "/project", true);
        let command_line_options = core::CompilerOptions::default();
        let command_line_raw = serde_json::json!({
            "compilerOptions": {
                "composite": null
            }
        });

        let (parsed, errors) = get_parsed_command_line_of_config_file(
            config_file_name,
            Some(&command_line_options),
            Some(&command_line_raw),
            &host,
            None,
        );

        assert!(errors.is_empty(), "{errors:?}");
        let parsed = parsed.expect("config parse result");
        assert!(parsed.compiler_options().composite.is_unknown());
    }

    #[test]
    fn empty_type_roots_remains_configured() {
        let config_file_name = "/project/tsconfig.json";
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            config_file_name.to_owned(),
            r#"{"compilerOptions":{"typeRoots":[]}}"#.to_owned(),
        );
        let host = crate::tsoptionstest::VfsParseConfigHost::new(files, "/project", true);

        let (parsed, errors) =
            get_parsed_command_line_of_config_file(config_file_name, None, None, &host, None);

        assert!(errors.is_empty(), "{errors:?}");
        let parsed = parsed.expect("config parse result");
        let options = parsed.compiler_options();
        assert!(options.type_roots_configured);
        assert!(options.type_roots.is_empty());
        assert_eq!(
            options.get_effective_type_roots("/project"),
            (Vec::new(), true)
        );
    }

    #[test]
    fn file_path_options_are_normalized_relative_to_config_dir() {
        let config_file_name = "c:/root/src/tsconfig.json";
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            config_file_name.to_owned(),
            r#"{"compilerOptions":{"rootDirs":[".","../generated/src"],"baseUrl":"."}}"#.to_owned(),
        );
        let host = crate::tsoptionstest::VfsParseConfigHost::new(files, "c:/root/src", true);

        let (parsed, errors) =
            get_parsed_command_line_of_config_file(config_file_name, None, None, &host, None);

        assert!(errors.is_empty(), "{errors:?}");
        let parsed = parsed.expect("config parse result");
        let options = parsed.compiler_options();
        assert_eq!(
            options.root_dirs,
            vec!["c:/root/src".to_owned(), "c:/root/generated/src".to_owned()]
        );
        assert_eq!(options.base_url, "c:/root/src");
    }

    #[test]
    fn set_compiler_options_preserves_paths() {
        let mut paths = ts_collections::OrderedMap::new();
        paths.set(
            "fake:thing".to_owned(),
            vec!["./node_modules/fake/thing".to_owned()],
        );
        let options = core::CompilerOptions {
            paths,
            paths_base_path: "/a/b".to_owned(),
            ..Default::default()
        };
        let mut parsed = crate::ParsedCommandLine::default();

        parsed.set_compiler_options(options);
        let roundtrip = parsed.compiler_options();

        assert_eq!(roundtrip.paths_base_path, "/a/b");
        assert_eq!(
            roundtrip.paths.get(&"fake:thing".to_owned()),
            Some(&vec!["./node_modules/fake/thing".to_owned()])
        );
    }

    #[test]
    fn deprecated_options_preserve_es3_and_charset() {
        let config_file_name = "/project/tsconfig.json";
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            config_file_name.to_owned(),
            r#"{"compilerOptions":{"target":"ES3","charset":"utf8"}}"#.to_owned(),
        );
        let host = crate::tsoptionstest::VfsParseConfigHost::new(files, "/project", true);

        let (parsed, errors) =
            get_parsed_command_line_of_config_file(config_file_name, None, None, &host, None);

        assert!(errors.is_empty(), "{errors:?}");
        let parsed = parsed.expect("config parse result");
        let options = parsed.compiler_options();
        assert!(options.target_is_es3);
        assert_eq!(options.charset, "utf8");
    }

    #[test]
    fn command_line_false_overrides_config_composite_with_null_build_info() {
        let config_file_name = "/project/tsconfig.json";
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            config_file_name.to_owned(),
            r#"{"compilerOptions":{"composite":true,"tsBuildInfoFile":"./x.tsbuildinfo"}}"#
                .to_owned(),
        );
        let host = crate::tsoptionstest::VfsParseConfigHost::new(files, "/project", true);
        let command_line_options = core::CompilerOptions {
            composite: core::TS_FALSE,
            ..Default::default()
        };
        let command_line_raw = serde_json::json!({
            "compilerOptions": {
                "composite": false,
                "tsBuildInfoFile": null
            }
        });

        let (parsed, errors) = get_parsed_command_line_of_config_file(
            config_file_name,
            Some(&command_line_options),
            Some(&command_line_raw),
            &host,
            None,
        );

        assert!(errors.is_empty(), "{errors:?}");
        let parsed = parsed.expect("config parse result");
        assert!(parsed.compiler_options().composite.is_false());
        assert!(parsed.compiler_options().ts_build_info_file.is_empty());
    }
}
