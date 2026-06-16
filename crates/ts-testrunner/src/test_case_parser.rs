use std::collections::{BTreeMap, HashMap};

use crate::{ParsedCommandLine, SRC_FOLDER, TestCaseContent, TestUnit};
use ts_core as core;
use ts_scanner as scanner;
use ts_testutil::harnessutil;
use ts_tsoptions as tsoptions;

pub type RawCompilerSettings = HashMap<String, String>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ParseTestFilesOptions {
    pub allow_implicit_first_file: bool,
}

pub fn make_units_from_test(code: &str, file_name: &str) -> TestCaseContent {
    let (test_units, symlinks, current_directory, global_options, _) =
        parse_test_files_and_symlinks(code, file_name, |filename, content, _file_options| {
            Ok(TestUnit {
                content,
                name: filename,
            })
        });

    let current_directory = if current_directory.is_empty() {
        SRC_FOLDER.to_string()
    } else {
        current_directory
    };

    let mut test_units = test_units;
    let all_files = test_units
        .iter()
        .map(|unit| {
            (
                normalize_absolute_path(&unit.name, &current_directory),
                unit.content.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut tsconfig: Option<ParsedCommandLine> = None;
    let mut tsconfig_file_unit_data = None;
    if let Some(index) = test_units
        .iter()
        .position(|unit| get_config_name_from_file_name(&unit.name).is_some())
    {
        let unit = test_units.remove(index);
        let config_file_name = normalize_absolute_path(&unit.name, &current_directory);
        let use_case_sensitive_file_names = global_options
            .get("usecasesensitivefilenames")
            .map(|value| parse_bool(value))
            .unwrap_or(false);
        let parse_config_host = tsoptions::tsoptionstest::VfsParseConfigHost::new(
            all_files,
            &current_directory,
            use_case_sensitive_file_names,
        );
        let parsed = tsoptions::parse_config(&config_file_name, &unit.content, &parse_config_host);
        tsconfig = Some(to_harness_parsed_command_line(parsed));
        tsconfig_file_unit_data = Some(unit);
    }

    TestCaseContent {
        test_unit_data: test_units,
        tsconfig,
        tsconfig_file_unit_data,
        symlinks,
    }
}

fn to_harness_parsed_command_line(parsed: tsoptions::ParsedCommandLine) -> ParsedCommandLine {
    let options = parsed.compiler_options();
    ParsedCommandLine {
        compiler_options: harnessutil::CompilerOptions {
            module: options.module.to_string(),
            module_resolution: module_resolution_kind_name(options.module_resolution),
            module_detection: module_detection_kind_name(options.module_detection),
            target: if options.target_is_es3 {
                "ES3".to_owned()
            } else {
                options.target.to_string()
            },
            jsx: if options.jsx == core::JsxEmit::None {
                String::new()
            } else {
                options.jsx.string().to_string()
            },
            jsx_factory: options.jsx_factory,
            jsx_fragment_factory: options.jsx_fragment_factory,
            jsx_import_source: options.jsx_import_source,
            react_namespace: options.react_namespace,
            strict: tristate_to_bool(options.strict),
            strict_null_checks: tristate_to_bool(options.strict_null_checks),
            exact_optional_property_types: tristate_to_bool(options.exact_optional_property_types),
            no_implicit_any: tristate_to_bool(options.no_implicit_any),
            no_implicit_this: tristate_to_bool(options.no_implicit_this),
            no_implicit_returns: tristate_to_bool(options.no_implicit_returns),
            no_implicit_override: tristate_to_bool(options.no_implicit_override),
            strict_function_types: tristate_to_bool(options.strict_function_types),
            strict_bind_call_apply: tristate_to_bool(options.strict_bind_call_apply),
            strict_builtin_iterator_return: tristate_to_bool(
                options.strict_builtin_iterator_return,
            ),
            strict_property_initialization: tristate_to_bool(
                options.strict_property_initialization,
            ),
            stable_type_ordering: tristate_to_bool(options.stable_type_ordering),
            no_property_access_from_index_signature: tristate_to_bool(
                options.no_property_access_from_index_signature,
            ),
            no_unchecked_indexed_access: tristate_to_bool(options.no_unchecked_indexed_access),
            use_unknown_in_catch_variables: tristate_to_bool(
                options.use_unknown_in_catch_variables,
            ),
            use_define_for_class_fields: tristate_to_bool(options.use_define_for_class_fields),
            experimental_decorators: tristate_to_bool(options.experimental_decorators),
            emit_decorator_metadata: tristate_to_bool(options.emit_decorator_metadata),
            isolated_modules: tristate_to_bool(options.isolated_modules),
            isolated_declarations: tristate_to_bool(options.isolated_declarations),
            verbatim_module_syntax: tristate_to_bool(options.verbatim_module_syntax),
            erasable_syntax_only: tristate_to_bool(options.erasable_syntax_only),
            es_module_interop: tristate_to_bool(options.es_module_interop),
            es_module_interop_is_false: options.es_module_interop.is_false(),
            allow_synthetic_default_imports: tristate_to_bool(
                options.allow_synthetic_default_imports,
            ),
            allow_synthetic_default_imports_is_false: options
                .allow_synthetic_default_imports
                .is_false(),
            always_strict_is_false: options.always_strict.is_false(),
            downlevel_iteration: tristate_to_bool(options.downlevel_iteration),
            charset: options.charset,
            ignore_deprecations: options.ignore_deprecations,
            keyof_strings_only: tristate_to_bool(options.keyof_strings_only),
            no_implicit_use_strict: tristate_to_bool(options.no_implicit_use_strict),
            no_strict_generic_checks: tristate_to_bool(options.no_strict_generic_checks),
            out: options.out,
            suppress_excess_property_errors: tristate_to_bool(
                options.suppress_excess_property_errors,
            ),
            suppress_implicit_any_index_errors: tristate_to_bool(
                options.suppress_implicit_any_index_errors,
            ),
            target_is_es3: options.target_is_es3,
            new_line: if options.new_line == core::NEW_LINE_KIND_NONE {
                String::new()
            } else {
                options.new_line.get_new_line_character().to_owned()
            },
            pretty: options.pretty.is_true(),
            skip_lib_check: options.skip_lib_check.is_true(),
            skip_default_lib_check: options.skip_default_lib_check.is_true(),
            no_error_truncation: options.no_error_truncation.is_true(),
            allow_arbitrary_extensions: tristate_to_bool(options.allow_arbitrary_extensions),
            allow_importing_ts_extensions: tristate_to_bool(options.allow_importing_ts_extensions),
            allow_js: tristate_to_bool(options.allow_js),
            check_js: tristate_to_bool(options.check_js),
            allow_umd_global_access: tristate_to_bool(options.allow_umd_global_access),
            allow_unreachable_code: tristate_to_bool(options.allow_unreachable_code),
            allow_unused_labels: tristate_to_bool(options.allow_unused_labels),
            no_fallthrough_cases_in_switch: tristate_to_bool(
                options.no_fallthrough_cases_in_switch,
            ),
            no_unused_locals: tristate_to_bool(options.no_unused_locals),
            no_unused_parameters: tristate_to_bool(options.no_unused_parameters),
            no_unchecked_side_effect_imports: tristate_to_bool(
                options.no_unchecked_side_effect_imports,
            ),
            preserve_symlinks: tristate_to_bool(options.preserve_symlinks),
            resolve_json_module: tristate_to_bool(options.resolve_json_module),
            resolve_package_json_exports: tristate_to_bool(options.resolve_package_json_exports),
            resolve_package_json_imports: tristate_to_bool(options.resolve_package_json_imports),
            rewrite_relative_import_extensions: tristate_to_bool(
                options.rewrite_relative_import_extensions,
            ),
            out_dir: options.out_dir,
            out_file: options.out_file,
            project: options.project,
            root_dir: options.root_dir,
            ts_build_info_file: options.ts_build_info_file,
            base_url: options.base_url,
            paths: options.paths,
            paths_base_path: options.paths_base_path,
            declaration_dir: options.declaration_dir,
            root_dirs: options.root_dirs,
            type_roots: options.type_roots,
            type_roots_configured: options.type_roots_configured,
            types: options.types,
            custom_conditions: options.custom_conditions,
            lib: options.lib,
            lib_replacement: options.lib_replacement.is_true(),
            module_suffixes: options.module_suffixes,
            no_lib: tristate_to_bool(options.no_lib),
            no_resolve: tristate_to_bool(options.no_resolve),
            trace_resolution: options.trace_resolution.is_true(),
            no_check: options.no_check.is_true(),
            no_emit: options.no_emit.is_true(),
            no_emit_helpers: options.no_emit_helpers.is_true(),
            no_emit_on_error: options.no_emit_on_error.is_true(),
            remove_comments: options.remove_comments.is_true(),
            strip_internal: tristate_to_bool(options.strip_internal),
            source_map: options.source_map.is_true(),
            source_root: options.source_root,
            map_root: options.map_root,
            inline_source_map: options.inline_source_map.is_true(),
            inline_sources: options.inline_sources.is_true(),
            declaration: options.declaration.is_true(),
            declaration_map: options.declaration_map.is_true(),
            composite: options.composite.is_true(),
            incremental: tristate_to_bool(options.incremental),
            preserve_const_enums: tristate_to_bool(options.preserve_const_enums),
            emit_declaration_only: options.emit_declaration_only.is_true(),
            emit_bom: tristate_to_bool(options.emit_bom),
            import_helpers: tristate_to_bool(options.import_helpers),
            max_node_module_js_depth: options.max_node_module_js_depth,
        },
        file_names: parsed.file_names,
        errors: parsed.errors,
        config_file: parsed.config_file,
        config_file_path: parsed.config_file_path,
    }
}

fn tristate_to_bool(value: core::Tristate) -> Option<bool> {
    if value.is_true() {
        Some(true)
    } else if value.is_false() {
        Some(false)
    } else {
        None
    }
}

fn module_resolution_kind_name(kind: ts_core::ModuleResolutionKind) -> String {
    match kind {
        ts_core::ModuleResolutionKind::Classic => "Classic".to_string(),
        ts_core::ModuleResolutionKind::Node10 => "Node10".to_string(),
        ts_core::ModuleResolutionKind::Node16 => "Node16".to_string(),
        ts_core::ModuleResolutionKind::NodeNext => "NodeNext".to_string(),
        ts_core::ModuleResolutionKind::Bundler => "Bundler".to_string(),
        ts_core::ModuleResolutionKind::Unknown => String::new(),
        _ => format!("ModuleResolutionKind({})", kind.0),
    }
}

fn module_detection_kind_name(kind: ts_core::ModuleDetectionKind) -> String {
    match kind {
        ts_core::ModuleDetectionKind::Auto => "auto".to_string(),
        ts_core::ModuleDetectionKind::Legacy => "legacy".to_string(),
        ts_core::ModuleDetectionKind::Force => "force".to_string(),
        ts_core::ModuleDetectionKind::None => String::new(),
        _ => format!("ModuleDetectionKind({})", kind.0),
    }
}

pub fn parse_test_files_and_symlinks<T, F>(
    code: &str,
    file_name: &str,
    parse_file: F,
) -> ParsedTestFilesAndSymlinks<T>
where
    F: FnMut(String, String, HashMap<String, String>) -> Result<T, String>,
{
    parse_test_files_and_symlinks_with_options(
        code,
        file_name,
        parse_file,
        ParseTestFilesOptions::default(),
    )
}

pub fn parse_test_files_and_symlinks_with_options<T, F>(
    code: &str,
    file_name: &str,
    mut parse_file: F,
    options: ParseTestFilesOptions,
) -> ParsedTestFilesAndSymlinks<T>
where
    F: FnMut(String, String, HashMap<String, String>) -> Result<T, String>,
{
    let mut test_units = Vec::new();
    let lines = split_lines(code);

    let mut current_file_content = String::new();
    let mut current_file_name = if options.allow_implicit_first_file {
        file_name.to_string()
    } else {
        String::new()
    };
    let mut seen_content_line = false;
    let mut has_seen_file = false;
    let mut current_directory = String::new();
    let mut parse_error = None;
    let mut current_file_options = HashMap::new();
    let mut symlinks = HashMap::new();
    let mut global_options = HashMap::new();

    for line in lines {
        if parse_symlink_from_test(line, &mut symlinks) {
            continue;
        }

        if let Some((metadata_name, metadata_value)) = parse_option_line(line) {
            let metadata_name = metadata_name.to_ascii_lowercase();
            let metadata_value = metadata_value.trim().to_string();
            if metadata_name == "currentdirectory" {
                current_directory = metadata_value.clone();
            }
            if metadata_name != "filename" {
                if metadata_name == "symlink" && !current_file_name.is_empty() {
                    for link in metadata_value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        symlinks.insert(link.to_string(), current_file_name.clone());
                    }
                } else if FOURSLSH_DIRECTIVES.contains(&metadata_name.as_str()) {
                    current_file_options.insert(metadata_name, metadata_value);
                } else {
                    global_options.insert(metadata_name, metadata_value);
                }
                continue;
            }

            if !current_file_name.is_empty() {
                let should_save_file = !options.allow_implicit_first_file
                    || !current_file_content.is_empty()
                    || has_seen_file;
                if should_save_file {
                    has_seen_file = true;
                    match parse_file(
                        current_file_name.clone(),
                        current_file_content.clone(),
                        current_file_options.clone(),
                    ) {
                        Ok(new_test_file) => test_units.push(new_test_file),
                        Err(err) => {
                            parse_error = Some(err);
                            break;
                        }
                    }
                }

                current_file_content.clear();
                seen_content_line = false;
                current_file_name = metadata_value;
                current_file_options = HashMap::new();
            } else {
                let has_content_before_first_filename = !current_file_content.is_empty()
                    && scanner::skip_trivia(&current_file_content, 0) != current_file_content.len();
                if has_content_before_first_filename && !options.allow_implicit_first_file {
                    panic!(
                        "Non-comment test content appears before the first '// @Filename' directive"
                    );
                }

                if has_content_before_first_filename && options.allow_implicit_first_file {
                    has_seen_file = true;
                    match parse_file(
                        current_file_name.clone(),
                        current_file_content.clone(),
                        current_file_options.clone(),
                    ) {
                        Ok(new_test_file) => test_units.push(new_test_file),
                        Err(err) => {
                            parse_error = Some(err);
                            break;
                        }
                    }
                }

                current_file_content.clear();
                seen_content_line = false;
                current_file_name = metadata_value.trim().to_string();
                current_file_options = HashMap::new();
            }
        } else {
            if options.allow_implicit_first_file {
                if seen_content_line {
                    current_file_content.push('\n');
                }
                seen_content_line = true;
            } else if !current_file_content.is_empty() {
                current_file_content.push('\n');
            }
            current_file_content.push_str(line);
        }
    }

    if test_units.is_empty() && current_file_name.is_empty() {
        current_file_name = get_base_file_name(file_name);
    }

    if parse_error.is_none() {
        match parse_file(
            current_file_name,
            current_file_content,
            current_file_options,
        ) {
            Ok(new_test_file) => test_units.push(new_test_file),
            Err(err) => parse_error = Some(err),
        }
    }

    (
        test_units,
        symlinks,
        current_directory,
        global_options,
        parse_error,
    )
}

pub type ParsedTestFilesAndSymlinks<T> = (
    Vec<T>,
    HashMap<String, String>,
    String,
    HashMap<String, String>,
    Option<String>,
);

pub fn extract_compiler_settings(content: &str) -> RawCompilerSettings {
    let mut opts = HashMap::new();
    for line in split_lines(content) {
        if let Some((name, value)) = parse_option_line(line) {
            opts.insert(
                name.to_ascii_lowercase(),
                value.trim().trim_end_matches(';').to_string(),
            );
        }
    }
    opts
}

pub fn parse_symlink_from_test(line: &str, symlinks: &mut HashMap<String, String>) -> bool {
    let Some((name, value)) = parse_option_line(line) else {
        return false;
    };
    if !name.eq_ignore_ascii_case("link") {
        return false;
    }
    let Some((target, link)) = value.split_once("->") else {
        return false;
    };
    symlinks.insert(link.trim().to_string(), target.trim().to_string());
    true
}

const FOURSLSH_DIRECTIVES: &[&str] = &["emitthisfile"];

fn split_lines(code: &str) -> Vec<&str> {
    let code = code.strip_prefix('\u{FEFF}').unwrap_or(code);
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = code.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if index + 1 < bytes.len() && bytes[index + 1] == b'\n' => {
                lines.push(&code[start..index]);
                index += 2;
                start = index;
            }
            b'\n' => {
                lines.push(&code[start..index]);
                index += 1;
                start = index;
            }
            _ => index += 1,
        }
    }
    lines.push(&code[start..]);
    lines
}

fn trim_metadata_value(value: &str) -> &str {
    match value.find(['\r', '\n']) {
        Some(index) => &value[..index],
        None => value,
    }
}

fn parse_option_line(line: &str) -> Option<(String, String)> {
    let line = line.trim_start();
    let rest = line.strip_prefix("//")?.trim_start();
    let rest = rest.strip_prefix('@')?;
    let (name, value) = rest.split_once(':')?;
    let name = name.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some((name.to_string(), trim_metadata_value(value).to_string()))
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value
            .trim()
            .trim_end_matches(';')
            .to_ascii_lowercase()
            .as_str(),
        "true" | "1"
    )
}

fn get_base_file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn normalize_absolute_path(file_name: &str, current_directory: &str) -> String {
    if file_name.starts_with('/') || is_rooted_disk_path(file_name) {
        file_name.replace('\\', "/")
    } else if current_directory.ends_with('/') {
        format!("{current_directory}{file_name}")
    } else {
        format!("{current_directory}/{file_name}")
    }
}

fn is_rooted_disk_path(path: &str) -> bool {
    path.len() > 2 && path.as_bytes()[1] == b':'
}

fn get_config_name_from_file_name(file_name: &str) -> Option<&str> {
    let basename = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    if basename.eq_ignore_ascii_case("tsconfig.json") {
        Some(basename)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsconfig_paths_keep_derived_base_path() {
        let content = r#"
// @filename: /a/b/tsconfig.json
{"compilerOptions":{"paths":{"fake:thing":["./node_modules/fake/thing"]}}}
// @filename: /a/b/main.ts
import "fake:thing";
"#;

        let parsed = make_units_from_test(content, "case.ts");
        let tsconfig = parsed.tsconfig.expect("tsconfig");
        let options = tsconfig.compiler_options;

        assert_eq!(options.paths_base_path, "/a/b");
        assert_eq!(
            options.paths.get(&"fake:thing".to_owned()),
            Some(&vec!["./node_modules/fake/thing".to_owned()])
        );
    }

    #[test]
    fn tsconfig_disk_rooted_files_stay_absolute() {
        let content = r#"
// @filename: c:/root/tsconfig.json
{"compilerOptions":{"paths":{"*":["*"]}}}
// @filename: c:/root/f1.ts
export var x = 1;
"#;

        let parsed = make_units_from_test(content, "case.ts");
        let tsconfig = parsed.tsconfig.expect("tsconfig");

        assert_eq!(tsconfig.file_names, vec!["c:/root/f1.ts".to_owned()]);
        assert_eq!(tsconfig.compiler_options.paths_base_path, "c:/root");
    }

    #[test]
    fn tsconfig_no_emit_incremental_keeps_source_files() {
        let content = r#"
// @filename: /a.ts
const x = 10;

// @filename: /tsconfig.json
{"compilerOptions":{"noEmit":true,"incremental":true}}
"#;

        let parsed = make_units_from_test(content, "case.ts");
        let tsconfig = parsed.tsconfig.expect("tsconfig");

        assert_eq!(tsconfig.file_names, vec!["/a.ts".to_owned()]);
    }

    #[test]
    fn parse_test_files_and_symlinks_skips_leading_bom_before_metadata() {
        let content = "\u{FEFF}// @target: es2015\n// @filename: /a.ts\nexport const a = 1;";

        let (test_units, _, _, global_options, parse_error) =
            parse_test_files_and_symlinks(content, "case.ts", |filename, content, _| {
                Ok(TestUnit {
                    name: filename,
                    content,
                })
            });

        assert_eq!(parse_error, None);
        assert_eq!(global_options.get("target"), Some(&"es2015".to_string()));
        assert_eq!(test_units.len(), 1);
        assert_eq!(test_units[0].name, "/a.ts");
        assert_eq!(test_units[0].content, "export const a = 1;");
    }

    #[test]
    fn parse_test_files_and_symlinks_does_not_split_bare_carriage_returns() {
        let content = "\u{FEFF}// @target: es6\r\r// newlines are <CR>\r`\r\\\r`";

        let (test_units, _, _, global_options, parse_error) =
            parse_test_files_and_symlinks(content, "case.ts", |filename, content, _| {
                Ok(TestUnit {
                    name: filename,
                    content,
                })
            });

        assert_eq!(parse_error, None);
        assert_eq!(global_options.get("target"), Some(&"es6".to_string()));
        assert_eq!(test_units.len(), 1);
        assert_eq!(test_units[0].name, "case.ts");
        assert_eq!(test_units[0].content, "");
    }
}
