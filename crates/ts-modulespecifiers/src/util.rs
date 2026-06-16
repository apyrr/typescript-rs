#![allow(dead_code)]

use crate::ModulePath;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RegexPatternCacheKey {
    pattern: String,
    case_insensitive: bool,
}

static REGEX_PATTERN_CACHE: LazyLock<RwLock<HashMap<RegexPatternCacheKey, Option<regex::Regex>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn compare_paths_by_redirect(
    a: &ModulePath,
    b: &ModulePath,
    use_case_sensitive_file_names: bool,
) -> std::cmp::Ordering {
    if a.is_redirect == b.is_redirect {
        compare_paths(&a.file_name, &b.file_name, use_case_sensitive_file_names)
    } else if a.is_redirect {
        std::cmp::Ordering::Greater
    } else {
        std::cmp::Ordering::Less
    }
}

pub fn path_is_bare_specifier(path: &str) -> bool {
    !ts_tspath::path_is_absolute(path) && !ts_tspath::path_is_relative(path)
}

pub fn is_excluded_by_regex(module_specifier: &str, excludes: &[String]) -> bool {
    excludes
        .iter()
        .filter_map(|pattern| string_to_regex(pattern))
        .any(|re| re.is_match(module_specifier))
}

fn string_to_regex(pattern: &str) -> Option<regex::Regex> {
    let mut pattern = pattern.to_string();
    let mut case_insensitive = false;

    if pattern.len() > 2
        && pattern.starts_with('/')
        && let Some(last_slash) = pattern.rfind('/')
        && last_slash > 0
    {
        let bytes = pattern.as_bytes();
        let has_unescaped_middle_slash =
            (1..last_slash).any(|index| bytes[index] == b'/' && bytes[index - 1] != b'\\');
        if !has_unescaped_middle_slash {
            let flags = pattern[last_slash + 1..].to_string();
            pattern = pattern[1..last_slash].to_string();
            case_insensitive = flags.chars().any(|flag| flag == 'i');
        }
    }
    let key = RegexPatternCacheKey {
        pattern: pattern.clone(),
        case_insensitive,
    };

    if let Some(re) = REGEX_PATTERN_CACHE
        .read()
        .expect("regex pattern cache read lock poisoned")
        .get(&key)
    {
        return re.clone();
    }

    let mut cache = REGEX_PATTERN_CACHE
        .write()
        .expect("regex pattern cache write lock poisoned");
    if let Some(re) = cache.get(&key) {
        return re.clone();
    }
    if cache.len() > 1000 {
        cache.clear();
    }

    let compile_pattern = if case_insensitive {
        format!("(?i:{pattern})")
    } else {
        pattern.clone()
    };
    let compiled = regex::Regex::new(&compile_pattern).ok();
    cache.insert(key, compiled.clone());
    compiled
}

/// Ensures a path is either absolute (prefixed with `/` or `c:`) or dot-relative (prefixed
/// with `./` or `../`) so as not to be confused with an unprefixed module name.
///
/// ```text
/// ensure_path_is_non_module_name("/path/to/file.ext") == "/path/to/file.ext"
/// ensure_path_is_non_module_name("./path/to/file.ext") == "./path/to/file.ext"
/// ensure_path_is_non_module_name("../path/to/file.ext") == "../path/to/file.ext"
/// ensure_path_is_non_module_name("path/to/file.ext") == "./path/to/file.ext"
/// ```
pub fn ensure_path_is_non_module_name(path: &str) -> String {
    if path_is_bare_specifier(path) {
        format!("./{path}")
    } else {
        path.to_string()
    }
}

pub fn get_js_extension_for_declaration_file_extension(ext: &str) -> String {
    match ext {
        ".d.ts" => ".js".to_string(),
        ".d.mts" => ".mjs".to_string(),
        ".d.cts" => ".cjs".to_string(),
        _ => ext[".d".len()..ext.len() - ts_tspath::EXTENSION_TS.len()].to_string(),
    }
}

// TryGetRealFileNameForNonJSDeclarationFileName remaps files like `foo.d.json.ts` or
// `foo.module.d.css.ts` back to their real non-JS names.
pub fn try_get_real_file_name_for_non_js_declaration_file_name(file_name: &str) -> String {
    let base_name = ts_tspath::get_base_file_name(file_name);
    if !file_name.ends_with(".ts") || !base_name.contains(".d.") || base_name.ends_with(".d.ts") {
        return String::new();
    }
    let no_extension = ts_tspath::remove_extension(file_name, ts_tspath::EXTENSION_TS);
    let last_dot_index = no_extension.rfind('.').unwrap();
    let ext = &no_extension[last_dot_index..];
    let before = no_extension.split(".d.").next().unwrap_or(&no_extension);
    format!("{before}{ext}")
}

fn get_js_extension_for_file(file_name: &str, options: &ts_core::CompilerOptions) -> String {
    let result = ts_module::try_get_js_extension_for_file(
        file_name,
        options.jsx == ts_core::JsxEmit::Preserve,
    );
    if result.is_empty() {
        panic!(
            "Extension {} is unsupported:: FileName:: {}",
            extension_from_path(file_name),
            file_name
        );
    }
    result
}

pub fn extension_from_path(path: &str) -> String {
    let ext = ts_tspath::try_get_extension_from_path(path);
    if ext.is_empty() {
        panic!("File {path} has unknown extension.");
    }
    ext.to_string()
}

pub fn try_get_any_file_from_path(path: &str, mut file_exists: impl FnMut(&str) -> bool) -> bool {
    // !!! TODO: shouldn't this use readdir instead of fileexists for perf?
    // We check all js, `node` and `json` extensions in addition to TS, since node module resolution would also choose those over the directory
    let ext_groups = ts_tsoptions::get_supported_extensions(
        &ts_core::CompilerOptions {
            allow_js: ts_core::TS_TRUE,
            ..ts_core::CompilerOptions::default()
        },
        &[
            ts_tsoptions::FileExtensionInfo {
                extension: "node".to_string(),
                is_mixed_content: false,
                script_kind: ts_core::ScriptKind::External,
            },
            ts_tsoptions::FileExtensionInfo {
                extension: "json".to_string(),
                is_mixed_content: false,
                script_kind: ts_core::ScriptKind::JSON,
            },
        ],
    );
    ext_groups
        .iter()
        .flatten()
        .any(|ext| file_exists(&format!("{path}{ext}")))
}

pub fn get_paths_relative_to_root_dirs(
    path: &str,
    root_dirs: &[String],
    use_case_sensitive_file_names: bool,
) -> Vec<String> {
    let mut results = Vec::new();
    for root_dir in root_dirs {
        let relative_path =
            get_relative_path_if_in_same_volume(path, root_dir, use_case_sensitive_file_names);
        if !is_path_relative_to_parent(&relative_path) {
            results.push(relative_path);
        }
    }
    results
}

pub fn is_path_relative_to_parent(path: &str) -> bool {
    path.starts_with("..")
}

pub fn get_relative_path_if_in_same_volume(
    path: &str,
    directory_path: &str,
    use_case_sensitive_file_names: bool,
) -> String {
    let options = ts_tspath::ComparePathsOptions {
        use_case_sensitive_file_names,
        current_directory: directory_path.to_string(),
    };
    let relative_path =
        ts_tspath::get_relative_path_to_directory_or_url(directory_path, path, false, &options);
    if ts_tspath::is_rooted_disk_path(&relative_path) {
        return String::new();
    }
    relative_path
}

pub fn package_json_paths_are_equal(
    a: &str,
    b: &str,
    options: &ts_tspath::ComparePathsOptions,
) -> bool {
    if a == b {
        return true;
    }
    if a.is_empty() || b.is_empty() {
        return false;
    }
    ts_tspath::compare_paths(a, b, options) == std::cmp::Ordering::Equal
}

pub fn prefers_ts_extension(allowed_endings: &[crate::ModuleSpecifierEnding]) -> bool {
    let js_priority = allowed_endings
        .iter()
        .position(|ending| *ending == crate::ModuleSpecifierEnding::JsExtension);
    let ts_priority = allowed_endings
        .iter()
        .position(|ending| *ending == crate::ModuleSpecifierEnding::TsExtension);
    if let Some(ts_priority) = ts_priority {
        return js_priority.is_some_and(|js_priority| ts_priority < js_priority);
    }
    false
}

pub fn replace_first_star(s: &str, replacement: &str) -> String {
    s.replacen('*', replacement, 1)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NodeModulePathParts {
    pub top_level_node_modules_index: usize,
    pub top_level_package_name_index: usize,
    pub package_root_index: isize,
    pub file_name_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum NodeModulesPathParseState {
    BeforeNodeModules,
    NodeModules,
    Scope,
    PackageContent,
}

pub fn get_node_module_path_parts(full_path: &str) -> Option<NodeModulePathParts> {
    // If fullPath can't be valid module file within node_modules, returns undefined.
    // Example of expected pattern: /base/path/node_modules/[@scope/otherpackage/@otherscope/node_modules/]package/[subdirectory/]file.js
    // Returns indices:                       ^            ^                                                      ^             ^
    let mut top_level_node_modules_index = 0;
    let mut top_level_package_name_index = 0;
    let mut package_root_index = 0;

    let mut part_start = 0;
    let mut part_end = 0;
    let mut state = NodeModulesPathParseState::BeforeNodeModules;

    while part_end >= 0 {
        part_start = part_end as usize;
        part_end = ts_core::index_after(full_path, "/", part_start + 1);
        match state {
            NodeModulesPathParseState::BeforeNodeModules => {
                if full_path[part_start..].starts_with("/node_modules/") {
                    top_level_node_modules_index = part_start;
                    top_level_package_name_index = part_end.max(0) as usize;
                    state = NodeModulesPathParseState::NodeModules;
                }
            }
            NodeModulesPathParseState::NodeModules | NodeModulesPathParseState::Scope => {
                if state == NodeModulesPathParseState::NodeModules
                    && full_path.as_bytes().get(part_start + 1) == Some(&b'@')
                {
                    state = NodeModulesPathParseState::Scope;
                } else {
                    package_root_index = part_end;
                    state = NodeModulesPathParseState::PackageContent;
                }
            }
            NodeModulesPathParseState::PackageContent => {
                if full_path[part_start..].starts_with("/node_modules/") {
                    state = NodeModulesPathParseState::NodeModules;
                } else {
                    state = NodeModulesPathParseState::PackageContent;
                }
            }
        }
    }

    let file_name_index = part_start;

    if state > NodeModulesPathParseState::NodeModules {
        return Some(NodeModulePathParts {
            top_level_node_modules_index,
            top_level_package_name_index,
            package_root_index,
            file_name_index,
        });
    }
    None
}

pub fn get_package_name_from_directory(file_or_directory_path: &str) -> String {
    let Some(idx) = file_or_directory_path.rfind("/node_modules/") else {
        return String::new();
    };

    let basename = &file_or_directory_path[idx + "/node_modules/".len()..];
    if basename.as_bytes()[0] == b'.' {
        return String::new();
    }

    let Some(next_slash) = basename.find('/') else {
        return basename.to_string();
    };

    if basename.as_bytes()[0] != b'@' || next_slash == basename.len() - 1 {
        return basename[..next_slash].to_string();
    }

    let Some(second_slash) = basename[next_slash + 1..].find('/') else {
        return basename.to_string();
    };

    basename[..next_slash + 1 + second_slash].to_string()
}

pub fn get_node_modules_package_name(
    compiler_options: &ts_core::CompilerOptions,
    importing_source_file: &impl crate::SourceFileForSpecifierGeneration,
    node_modules_file_name: &str,
    host: &impl crate::ModuleSpecifierGenerationHost,
    preferences: crate::UserPreferences,
    options: crate::ModuleSpecifierOptions,
) -> String {
    let info = crate::specifiers::get_info(&importing_source_file.file_name(), host);
    let module_paths = crate::get_all_module_paths(
        info.clone(),
        node_modules_file_name,
        host,
        compiler_options,
        preferences.clone(),
        options,
    );
    for module_path in module_paths {
        let result = crate::specifiers::try_get_module_name_as_node_module(
            &module_path,
            &info,
            importing_source_file,
            host,
            compiler_options,
            crate::specifiers::NodeModuleSpecifierOptions {
                user_preferences: preferences.clone(),
                package_name_only: true,
                override_mode: options.override_import_mode,
            },
        );
        if !result.is_empty() {
            return result;
        }
    }
    String::new()
}

fn all_keys_start_with_dot(obj: &serde_json::Map<String, serde_json::Value>) -> bool {
    for k in obj.keys() {
        if !k.starts_with('.') {
            return false;
        }
    }
    true
}

// ProcessEntrypointEnding processes a pre-computed module specifier from a package.json exports
// entrypoint according to the entrypoint's Ending type and the user's preferred endings.
pub fn process_entrypoint_ending(
    entrypoint: &ts_module::ResolvedEntrypoint,
    prefs: crate::UserPreferences,
    host: &impl crate::ModuleSpecifierGenerationHost,
    options: &ts_core::CompilerOptions,
    importing_source_file: &impl crate::SourceFileForSpecifierGeneration,
    allowed_endings: &[crate::ModuleSpecifierEnding],
) -> String {
    let mut specifier = entrypoint.module_specifier.clone();
    if entrypoint.ending == ts_module::Ending::Fixed {
        return specifier;
    }

    let allowed_endings = if allowed_endings.is_empty() {
        struct FileNameOnly {
            file_name: String,
        }

        impl ts_ast::HasFileName for FileNameOnly {
            fn file_name(&self) -> String {
                self.file_name.clone()
            }

            fn path(&self) -> ts_tspath::Path {
                self.file_name.clone()
            }
        }

        let importing_file = FileNameOnly {
            file_name: importing_source_file.file_name(),
        };
        crate::get_allowed_endings_in_preferred_order(
            &prefs,
            host,
            options,
            importing_source_file,
            "",
            host.default_resolution_mode_for_file(&importing_file),
        )
    } else {
        allowed_endings.to_vec()
    };

    let preferred_ending = allowed_endings[0];

    // Handle declaration file extensions
    let dts_extension = ts_tspath::get_declaration_file_extension(&specifier);
    if !dts_extension.is_empty() {
        match preferred_ending {
            crate::ModuleSpecifierEnding::TsExtension
            | crate::ModuleSpecifierEnding::JsExtension => {
                // Map .d.ts -> .js, .d.mts -> .mjs, .d.cts -> .cjs
                let js_extension = get_js_extension_for_declaration_file_extension(&dts_extension);
                let extensions = [dts_extension.as_str()];
                return ts_tspath::change_any_extension(
                    &specifier,
                    &js_extension,
                    Some(&extensions),
                    false,
                );
            }
            crate::ModuleSpecifierEnding::Minimal | crate::ModuleSpecifierEnding::Index => {
                if entrypoint.ending == ts_module::Ending::Changeable {
                    // .d.mts/.d.cts must keep an extension; rewrite to .mjs/.cjs instead of dropping
                    if dts_extension == ts_tspath::EXTENSION_DTS {
                        specifier = ts_tspath::remove_extension(&specifier, &dts_extension);
                        if preferred_ending == crate::ModuleSpecifierEnding::Minimal {
                            specifier = specifier
                                .strip_suffix("/index")
                                .unwrap_or(&specifier)
                                .to_string();
                        }
                        return specifier;
                    }
                    let js_extension =
                        get_js_extension_for_declaration_file_extension(&dts_extension);
                    let extensions = [dts_extension.as_str()];
                    return ts_tspath::change_any_extension(
                        &specifier,
                        &js_extension,
                        Some(&extensions),
                        false,
                    );
                }
                // EndingExtensionChangeable - can only change extension, not remove it
                let js_extension = get_js_extension_for_declaration_file_extension(&dts_extension);
                let extensions = [dts_extension.as_str()];
                return ts_tspath::change_any_extension(
                    &specifier,
                    &js_extension,
                    Some(&extensions),
                    false,
                );
            }
        }
    }

    // Handle .ts/.tsx/.mts/.cts extensions
    if ts_tspath::file_extension_is_one_of(
        &specifier,
        &[
            ts_tspath::EXTENSION_TS,
            ts_tspath::EXTENSION_TSX,
            ts_tspath::EXTENSION_MTS,
            ts_tspath::EXTENSION_CTS,
        ],
    ) {
        match preferred_ending {
            crate::ModuleSpecifierEnding::TsExtension => return specifier,
            crate::ModuleSpecifierEnding::JsExtension => {
                let js_extension = ts_module::try_get_js_extension_for_file(
                    &specifier,
                    options.jsx == ts_core::JsxEmit::Preserve,
                );
                if !js_extension.is_empty() {
                    return format!(
                        "{}{}",
                        ts_tspath::remove_file_extension(&specifier),
                        js_extension
                    );
                }
                return specifier;
            }
            crate::ModuleSpecifierEnding::Minimal | crate::ModuleSpecifierEnding::Index => {
                if entrypoint.ending == ts_module::Ending::Changeable {
                    specifier = ts_tspath::remove_file_extension(&specifier);
                    if preferred_ending == crate::ModuleSpecifierEnding::Minimal {
                        specifier = specifier
                            .strip_suffix("/index")
                            .unwrap_or(&specifier)
                            .to_string();
                    }
                    return specifier;
                }
                // EndingExtensionChangeable - can only change extension, not remove it
                let js_extension = ts_module::try_get_js_extension_for_file(
                    &specifier,
                    options.jsx == ts_core::JsxEmit::Preserve,
                );
                if !js_extension.is_empty() {
                    return format!(
                        "{}{}",
                        ts_tspath::remove_file_extension(&specifier),
                        js_extension
                    );
                }
                return specifier;
            }
        }
    }

    // Handle .js/.jsx/.mjs/.cjs extensions
    if ts_tspath::file_extension_is_one_of(
        &specifier,
        &[
            ts_tspath::EXTENSION_JS,
            ts_tspath::EXTENSION_JSX,
            ts_tspath::EXTENSION_MJS,
            ts_tspath::EXTENSION_CJS,
        ],
    ) {
        match preferred_ending {
            crate::ModuleSpecifierEnding::TsExtension
            | crate::ModuleSpecifierEnding::JsExtension => return specifier,
            crate::ModuleSpecifierEnding::Minimal | crate::ModuleSpecifierEnding::Index => {
                if entrypoint.ending == ts_module::Ending::Changeable {
                    specifier = ts_tspath::remove_file_extension(&specifier);
                    if preferred_ending == crate::ModuleSpecifierEnding::Minimal {
                        specifier = specifier
                            .strip_suffix("/index")
                            .unwrap_or(&specifier)
                            .to_string();
                    }
                    return specifier;
                }
                // EndingExtensionChangeable - keep the extension
                return specifier;
            }
        }
    }

    // For other extensions (like .json), return as-is
    specifier
}

fn compare_paths(a: &str, b: &str, case_sensitive: bool) -> std::cmp::Ordering {
    let options = ts_tspath::ComparePathsOptions {
        use_case_sensitive_file_names: case_sensitive,
        current_directory: String::new(),
    };
    ts_tspath::compare_paths(a, b, &options)
}
