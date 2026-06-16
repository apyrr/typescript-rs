use std::sync::LazyLock;

use crate::{CompilerOptions, ResolvedModule, move_to_next_directory_separator_if_available};
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_tspath as tspath;

static TYPE_SCRIPT_VERSION: LazyLock<ts_semver::Version> = LazyLock::new(|| {
    ts_semver::try_parse_version(ts_core::version())
        .unwrap_or_else(|err| panic!("invalid TypeScript version: {err}"))
});

pub const INFERRED_TYPES_CONTAINING_FILE: &str = "__inferred type names__.ts";

pub fn is_applicable_versioned_types_key(key: &str) -> bool {
    let Some(range_text) = key.strip_prefix("types@") else {
        return false;
    };
    let (range, ok) = ts_semver::try_parse_version_range(range_text);
    ok && range.test(&TYPE_SCRIPT_VERSION)
}

pub fn parse_node_module_from_path(resolved: &str, is_folder: bool) -> String {
    let path = resolved.replace('\\', "/");
    let Some(idx) = path.rfind("/node_modules/") else {
        return String::new();
    };
    let index_after_node_modules = idx + "/node_modules/".len();
    let mut index_after_package_name =
        move_to_next_directory_separator_if_available(&path, index_after_node_modules, is_folder);
    if path[index_after_node_modules..].starts_with('@') {
        index_after_package_name = move_to_next_directory_separator_if_available(
            &path,
            index_after_package_name,
            is_folder,
        );
    }
    path[..index_after_package_name].to_string()
}

pub fn parse_package_name(module_name: &str) -> (String, String) {
    let mut idx = module_name.find('/');
    if module_name.starts_with('@')
        && let Some(first) = idx
    {
        idx = module_name[first + 1..]
            .find('/')
            .map(|next| first + 1 + next);
    }
    match idx {
        Some(idx) => (
            module_name[..idx].to_string(),
            module_name[idx + 1..].to_string(),
        ),
        None => (module_name.to_string(), String::new()),
    }
}

pub fn mangle_scoped_package_name(package_name: &str) -> String {
    if package_name.starts_with('@')
        && let Some(idx) = package_name.find('/')
    {
        return format!("{}__{}", &package_name[1..idx], &package_name[idx + 1..]);
    }
    package_name.to_string()
}

pub fn unmangle_scoped_package_name(package_name: &str) -> String {
    if let Some((before, after)) = package_name.split_once("__") {
        format!("@{before}/{after}")
    } else {
        package_name.to_string()
    }
}

pub fn get_types_package_name(package_name: &str) -> String {
    format!("@types/{}", mangle_scoped_package_name(package_name))
}

pub fn get_package_name_from_types_package_name(mangled_name: &str) -> String {
    mangled_name
        .strip_prefix("@types/")
        .map(unmangle_scoped_package_name)
        .unwrap_or_else(|| mangled_name.to_string())
}

pub fn compare_pattern_keys(a: &str, b: &str) -> std::cmp::Ordering {
    let a_pattern_index = a.find('*');
    let b_pattern_index = b.find('*');
    let base_len_a = a_pattern_index.map(|index| index + 1).unwrap_or(a.len());
    let base_len_b = b_pattern_index.map(|index| index + 1).unwrap_or(b.len());
    match base_len_b.cmp(&base_len_a) {
        std::cmp::Ordering::Equal => {}
        ordering => return ordering,
    }
    if a_pattern_index.is_none() {
        return std::cmp::Ordering::Greater;
    }
    if b_pattern_index.is_none() {
        return std::cmp::Ordering::Less;
    }
    b.len().cmp(&a.len())
}

// Returns a DiagnosticMessage if we won't include a resolved module due to its extension.
// The DiagnosticMessage's parameters are the imported module name, and the filename it resolved to.
// This returns a diagnostic even if the module will be an untyped module.
pub fn get_resolution_diagnostic(
    options: &CompilerOptions,
    resolved_module: &ResolvedModule,
    file_is_declaration: bool,
) -> Option<&'static diagnostics::Message> {
    let need_jsx = || {
        if options.jsx != core::JsxEmit::None {
            return None;
        }
        Some(&*diagnostics::Module_0_was_resolved_to_1_but_jsx_is_not_set)
    };

    let need_allow_js = || {
        if options.get_allow_js()
            || !options
                .no_implicit_any
                .default_if_unknown(options.strict)
                .is_true()
        {
            return None;
        }
        Some(&*diagnostics::Could_not_find_a_declaration_file_for_module_0_1_implicitly_has_an_any_type)
    };

    let need_resolve_json_module = || {
        if options.get_resolve_json_module() {
            return None;
        }
        Some(&*diagnostics::Module_0_was_resolved_to_1_but_resolveJsonModule_is_not_used)
    };

    let need_allow_arbitrary_extensions = || {
        if file_is_declaration || options.allow_arbitrary_extensions.is_true() {
            return None;
        }
        Some(&*diagnostics::Module_0_was_resolved_to_1_but_allowArbitraryExtensions_is_not_set)
    };

    match resolved_module.extension.as_str() {
        tspath::EXTENSION_TS
        | tspath::EXTENSION_DTS
        | tspath::EXTENSION_MTS
        | tspath::EXTENSION_DMTS
        | tspath::EXTENSION_CTS
        | tspath::EXTENSION_DCTS => None,
        tspath::EXTENSION_TSX => need_jsx(),
        tspath::EXTENSION_JSX => need_jsx().or_else(need_allow_js),
        tspath::EXTENSION_JS | tspath::EXTENSION_MJS | tspath::EXTENSION_CJS => need_allow_js(),
        tspath::EXTENSION_JSON => need_resolve_json_module(),
        _ => need_allow_arbitrary_extensions(),
    }
}

// TryGetJSExtensionForFile maps TS/JS/DTS extensions to the output JS-side extension.
// Returns an empty string if the extension is unsupported.
pub fn try_get_js_extension_for_file(file_name: &str, jsx_preserve: bool) -> String {
    let ext = extension(file_name);
    match ext.as_str() {
        ".ts" | ".d.ts" => ".js".to_string(),
        ".tsx" => {
            if jsx_preserve {
                ".jsx".to_string()
            } else {
                ".js".to_string()
            }
        }
        ".js" | ".jsx" | ".json" => ext,
        ".d.mts" | ".mts" | ".mjs" => ".mjs".to_string(),
        ".d.cts" | ".cts" | ".cjs" => ".cjs".to_string(),
        _ => String::new(),
    }
}

fn extension(file_name: &str) -> String {
    for ext in [
        ".d.mts", ".d.cts", ".d.ts", ".tsx", ".ts", ".jsx", ".js", ".json", ".mts", ".mjs", ".cts",
        ".cjs",
    ] {
        if file_name.ends_with(ext) {
            return ext.to_string();
        }
    }
    String::new()
}
