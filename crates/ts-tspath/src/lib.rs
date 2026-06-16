#![forbid(unsafe_code)]
pub mod extension;
mod ignoredpaths;
#[cfg(test)]
mod ignoredpaths_test;
pub mod path;
#[cfg(test)]
mod path_test;
#[cfg(test)]
mod starts_with_directory_test;
#[cfg(test)]
mod untitled_test;

pub use extension::{
    ALL_SUPPORTED_EXTENSIONS, EXTENSION_CJS, EXTENSION_CTS, EXTENSION_DCTS, EXTENSION_DMTS,
    EXTENSION_DTS, EXTENSION_JS, EXTENSION_JSON, EXTENSION_JSX, EXTENSION_MJS, EXTENSION_MTS,
    EXTENSION_TS, EXTENSION_TS_BUILD_INFO, EXTENSION_TSX,
    EXTENSIONS_NOT_SUPPORTING_EXTENSIONLESS_RESOLUTION, SUPPORTED_DECLARATION_EXTENSIONS,
    SUPPORTED_JS_EXTENSIONS, SUPPORTED_JS_EXTENSIONS_FLAT, SUPPORTED_TS_EXTENSIONS,
    SUPPORTED_TS_EXTENSIONS_FLAT, SUPPORTED_TS_IMPLEMENTATION_EXTENSIONS,
    all_supported_extensions_with_json, change_any_extension, change_extension,
    change_full_extension, extension_is_one_of, extension_is_ts, file_extension_is_one_of,
    get_declaration_emit_extension_for_path, get_declaration_file_extension,
    get_possible_original_input_extension_for_extension, has_implementation_ts_file_extension,
    has_js_file_extension, has_json_file_extension, has_ts_file_extension,
    is_declaration_file_name, remove_extension, remove_file_extension,
    supported_ts_extensions_with_json, supported_ts_extensions_with_json_flat,
    try_extract_ts_extension, try_get_extension_from_path,
};

pub const SUPPORTED_TS_EXTENSIONS_WITH_JSON_FLAT: &[&str] = &[
    EXTENSION_TS,
    EXTENSION_TSX,
    EXTENSION_DTS,
    EXTENSION_CTS,
    EXTENSION_DCTS,
    EXTENSION_MTS,
    EXTENSION_DMTS,
    EXTENSION_JSON,
];

pub fn has_implementation_tsfile_extension(file_name: &str) -> bool {
    has_implementation_ts_file_extension(file_name)
}
pub use ignoredpaths::contains_ignored_path;
pub use path::*;

#[allow(non_snake_case, non_upper_case_globals)]
pub mod Extension {
    pub const None: &str = "";
    pub const Ts: &str = super::EXTENSION_TS;
    pub const Tsx: &str = super::EXTENSION_TSX;
    pub const Dts: &str = super::EXTENSION_DTS;
    pub const Js: &str = super::EXTENSION_JS;
    pub const Jsx: &str = super::EXTENSION_JSX;
    pub const Json: &str = super::EXTENSION_JSON;
    pub const TsBuildInfo: &str = super::EXTENSION_TS_BUILD_INFO;
    pub const Mjs: &str = super::EXTENSION_MJS;
    pub const Mts: &str = super::EXTENSION_MTS;
    pub const Dmts: &str = super::EXTENSION_DMTS;
    pub const Cjs: &str = super::EXTENSION_CJS;
    pub const Cts: &str = super::EXTENSION_CTS;
    pub const Dcts: &str = super::EXTENSION_DCTS;
}
