#![forbid(unsafe_code)]
mod compare;
mod preferences;
mod specifiers;
#[cfg(test)]
mod specifiers_test;
mod types;
mod util;

pub use compare::count_path_components;
pub use preferences::*;
pub use specifiers::*;
pub use types::*;
pub use util::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ResultKind {
    #[default]
    None,
    NodeModules,
    Paths,
    Redirect,
    Relative,
    Ambient,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum ImportModuleSpecifierPreference {
    #[serde(rename = "")]
    #[default]
    None,
    #[serde(rename = "shortest")]
    Shortest,
    #[serde(rename = "project-relative")]
    ProjectRelative,
    #[serde(rename = "relative")]
    Relative,
    #[serde(rename = "non-relative")]
    NonRelative,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub enum ImportModuleSpecifierEndingPreference {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "index")]
    Index,
    #[serde(rename = "js")]
    Js,
    #[serde(rename = "")]
    None,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ModuleSpecifierEnding {
    #[default]
    Minimal,
    Index,
    JsExtension,
    TsExtension,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UserPreferences {
    pub import_module_specifier_preference: ImportModuleSpecifierPreference,
    pub import_module_specifier_ending: ImportModuleSpecifierEndingPreference,
    pub auto_import_specifier_exclude_regexes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ModuleSpecifierOptions {
    pub override_import_mode: ts_core::ResolutionMode,
}

pub const RESULT_KIND_NONE: ResultKind = ResultKind::None;
pub const RESULT_KIND_NODE_MODULES: ResultKind = ResultKind::NodeModules;
pub const RESULT_KIND_PATHS: ResultKind = ResultKind::Paths;
pub const RESULT_KIND_REDIRECT: ResultKind = ResultKind::Redirect;
pub const RESULT_KIND_RELATIVE: ResultKind = ResultKind::Relative;
pub const RESULT_KIND_AMBIENT: ResultKind = ResultKind::Ambient;

pub const IMPORT_MODULE_SPECIFIER_PREFERENCE_PROJECT_RELATIVE: ImportModuleSpecifierPreference =
    ImportModuleSpecifierPreference::ProjectRelative;
pub const IMPORT_MODULE_SPECIFIER_PREFERENCE_NONE: ImportModuleSpecifierPreference =
    ImportModuleSpecifierPreference::None;
pub const IMPORT_MODULE_SPECIFIER_PREFERENCE_SHORTEST: ImportModuleSpecifierPreference =
    ImportModuleSpecifierPreference::Shortest;
pub const IMPORT_MODULE_SPECIFIER_PREFERENCE_RELATIVE: ImportModuleSpecifierPreference =
    ImportModuleSpecifierPreference::Relative;
pub const IMPORT_MODULE_SPECIFIER_PREFERENCE_NON_RELATIVE: ImportModuleSpecifierPreference =
    ImportModuleSpecifierPreference::NonRelative;

pub const IMPORT_MODULE_SPECIFIER_ENDING_PREFERENCE_NONE: ImportModuleSpecifierEndingPreference =
    ImportModuleSpecifierEndingPreference::None;
pub const IMPORT_MODULE_SPECIFIER_ENDING_PREFERENCE_AUTO: ImportModuleSpecifierEndingPreference =
    ImportModuleSpecifierEndingPreference::Auto;
pub const IMPORT_MODULE_SPECIFIER_ENDING_PREFERENCE_MINIMAL: ImportModuleSpecifierEndingPreference =
    ImportModuleSpecifierEndingPreference::Minimal;
pub const IMPORT_MODULE_SPECIFIER_ENDING_PREFERENCE_INDEX: ImportModuleSpecifierEndingPreference =
    ImportModuleSpecifierEndingPreference::Index;
pub const IMPORT_MODULE_SPECIFIER_ENDING_PREFERENCE_JS: ImportModuleSpecifierEndingPreference =
    ImportModuleSpecifierEndingPreference::Js;

pub const MODULE_SPECIFIER_ENDING_MINIMAL: ModuleSpecifierEnding = ModuleSpecifierEnding::Minimal;
pub const MODULE_SPECIFIER_ENDING_INDEX: ModuleSpecifierEnding = ModuleSpecifierEnding::Index;
pub const MODULE_SPECIFIER_ENDING_JS_EXTENSION: ModuleSpecifierEnding =
    ModuleSpecifierEnding::JsExtension;
pub const MODULE_SPECIFIER_ENDING_TS_EXTENSION: ModuleSpecifierEnding =
    ModuleSpecifierEnding::TsExtension;
