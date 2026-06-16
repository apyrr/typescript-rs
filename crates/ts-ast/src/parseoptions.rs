use ts_core as core;
use ts_tspath as tspath;

use crate::*;

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct SourceFileParseOptions {
    pub file_name: String,
    pub path: tspath::Path,
    pub external_module_indicator_options: ExternalModuleIndicatorOptions,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ExternalModuleIndicatorOptions {
    pub jsx: bool,
    pub force: bool,
}

pub fn get_external_module_indicator_options(
    file_name: &str,
    options: &core::CompilerOptions,
    metadata: SourceFileMetaData,
) -> ExternalModuleIndicatorOptions {
    if tspath::is_declaration_file_name(file_name) {
        return ExternalModuleIndicatorOptions::default();
    }

    match options.get_emit_module_detection_kind() {
        core::ModuleDetectionKind::Force => {
            // All non-declaration files are modules, declaration files still do the usual isFileProbablyExternalModule
            ExternalModuleIndicatorOptions {
                force: true,
                ..Default::default()
            }
        }
        core::ModuleDetectionKind::Legacy => {
            // Files are modules if they have imports, exports, or import.meta
            ExternalModuleIndicatorOptions::default()
        }
        core::ModuleDetectionKind::Auto => {
            // If module is nodenext or node16, all esm format files are modules
            // If jsx is react-jsx or react-jsxdev then jsx tags force module-ness
            // otherwise, the presence of import or export statments (or import.meta) implies module-ness
            ExternalModuleIndicatorOptions {
                jsx: options.jsx == core::JsxEmit::ReactJSX
                    || options.jsx == core::JsxEmit::ReactJSXDev,
                force: is_file_forced_to_be_module_by_format(file_name, options, metadata),
            }
        }
        _ => ExternalModuleIndicatorOptions::default(),
    }
}

static IS_FILE_FORCED_TO_BE_MODULE_BY_FORMAT_EXTENSIONS: &[&str] = &[
    tspath::EXTENSION_CJS,
    tspath::EXTENSION_CTS,
    tspath::EXTENSION_MJS,
    tspath::EXTENSION_MTS,
];

fn is_file_forced_to_be_module_by_format(
    file_name: &str,
    options: &core::CompilerOptions,
    metadata: SourceFileMetaData,
) -> bool {
    // Excludes declaration files - they still require an explicit `export {}` or the like
    // for back compat purposes. The only non-declaration files _not_ forced to be a module are `.js` files
    // that aren't esm-mode (meaning not in a `type: module` scope).
    if get_implied_node_format_for_emit_worker(file_name, options.get_emit_module_kind(), metadata)
        == core::ModuleKind::ESNext
        || tspath::file_extension_is_one_of(
            file_name,
            IS_FILE_FORCED_TO_BE_MODULE_BY_FORMAT_EXTENSIONS,
        )
    {
        return true;
    }
    false
}

pub fn get_implied_node_format_for_emit_worker(
    file_name: &str,
    emit_module_kind: core::ModuleKind,
    source_file_meta_data: SourceFileMetaData,
) -> core::ResolutionMode {
    if core::ModuleKind::Node16 <= emit_module_kind
        && emit_module_kind <= core::ModuleKind::NodeNext
    {
        return source_file_meta_data.implied_node_format;
    }
    if source_file_meta_data.implied_node_format == core::ModuleKind::CommonJS
        && (source_file_meta_data.package_json_type == "commonjs"
            || tspath::file_extension_is_one_of(
                file_name,
                &[tspath::EXTENSION_CJS, tspath::EXTENSION_CTS],
            ))
    {
        return core::ModuleKind::CommonJS;
    }
    if source_file_meta_data.implied_node_format == core::ModuleKind::ESNext
        && (source_file_meta_data.package_json_type == "module"
            || tspath::file_extension_is_one_of(
                file_name,
                &[tspath::EXTENSION_MJS, tspath::EXTENSION_MTS],
            ))
    {
        return core::ModuleKind::ESNext;
    }
    core::ModuleKind::None
}

pub fn get_implied_node_format_for_file(path: &str, package_json_type: &str) -> core::ModuleKind {
    if tspath::file_extension_is_one_of(
        path,
        &[
            tspath::EXTENSION_DMTS,
            tspath::EXTENSION_MTS,
            tspath::EXTENSION_MJS,
        ],
    ) {
        core::ResolutionMode::ESNext
    } else if tspath::file_extension_is_one_of(
        path,
        &[
            tspath::EXTENSION_DCTS,
            tspath::EXTENSION_CTS,
            tspath::EXTENSION_CJS,
        ],
    ) {
        core::ResolutionMode::CommonJS
    } else if tspath::file_extension_is_one_of(
        path,
        &[
            tspath::EXTENSION_DTS,
            tspath::EXTENSION_TS,
            tspath::EXTENSION_TSX,
            tspath::EXTENSION_JS,
            tspath::EXTENSION_JSX,
        ],
    ) {
        if package_json_type == "module" {
            core::ResolutionMode::ESNext
        } else {
            core::ResolutionMode::CommonJS
        }
    } else {
        core::ResolutionMode::None
    }
}

pub fn get_emit_module_format_of_file_worker(
    file_name: &str,
    options: &core::CompilerOptions,
    source_file_meta_data: SourceFileMetaData,
) -> core::ModuleKind {
    let result = get_implied_node_format_for_emit_worker(
        file_name,
        options.get_emit_module_kind(),
        source_file_meta_data,
    );
    if result != core::ModuleKind::None {
        return result;
    }
    options.get_emit_module_kind()
}
