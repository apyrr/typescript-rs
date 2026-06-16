#![allow(dead_code)]

pub trait SourceFileForSpecifierGeneration {
    fn path(&self) -> ts_tspath::Path;
    fn file_name(&self) -> String;
    fn imports(&self) -> Vec<ts_ast::StringLiteralLike>;
    fn import_text(&self, import: &ts_ast::StringLiteralLike) -> String;
    fn is_js(&self) -> bool;
}

impl SourceFileForSpecifierGeneration for ts_ast::SourceFile {
    fn path(&self) -> ts_tspath::Path {
        ts_ast::SourceFile::path(self)
    }

    fn file_name(&self) -> String {
        ts_ast::SourceFile::file_name(self)
    }

    fn imports(&self) -> Vec<ts_ast::StringLiteralLike> {
        self.data().imports().to_vec()
    }

    fn import_text(&self, import: &ts_ast::StringLiteralLike) -> String {
        self.store().text(*import)
    }

    fn is_js(&self) -> bool {
        self.data().is_js()
    }
}

pub trait CheckerShape {
    fn source_file_store(&self, node: ts_ast::Node) -> Option<&ts_ast::AstStore>;
    fn source_node_symbol(&self, node: ts_ast::Node) -> Option<ts_ast::SymbolIdentity>;
    fn lookup_source_symbol_export(
        &mut self,
        symbol: ts_ast::SymbolIdentity,
        name: &str,
    ) -> Option<ts_ast::SymbolIdentity>;
    fn symbol_value_declaration(&self, symbol: ts_ast::SymbolIdentity) -> Option<ts_ast::Node>;
    fn get_symbol_at_location(&mut self, node: ts_ast::Node) -> Option<SpecifierSymbol>;
    fn get_aliased_symbol_at_location(&mut self, node: ts_ast::Node) -> Option<SpecifierSymbol>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleSymbolData {
    pub identity: ts_ast::SymbolIdentity,
    pub name: ts_ast::SymbolName,
    pub declarations: Vec<ts_ast::Node>,
    pub value_declaration: Option<ts_ast::Node>,
}

impl ModuleSymbolData {
    pub fn new(
        identity: ts_ast::SymbolIdentity,
        name: ts_ast::SymbolName,
        declarations: Vec<ts_ast::Node>,
        value_declaration: Option<ts_ast::Node>,
    ) -> Self {
        Self {
            identity,
            name,
            declarations,
            value_declaration,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecifierSymbol {
    identity: ts_ast::SymbolIdentity,
    flags: ts_ast::SymbolFlags,
}

impl SpecifierSymbol {
    pub fn new(identity: ts_ast::SymbolIdentity, flags: ts_ast::SymbolFlags) -> Self {
        Self { identity, flags }
    }

    pub fn identity(&self) -> ts_ast::SymbolIdentity {
        self.identity
    }

    pub fn flags(&self) -> ts_ast::SymbolFlags {
        self.flags
    }

    pub fn is_alias(&self) -> bool {
        self.flags & ts_ast::SYMBOL_FLAGS_ALIAS != 0
    }
}

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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModulePath {
    pub file_name: String,
    pub is_in_node_modules: bool,
    pub is_redirect: bool,
}

pub trait ModuleSpecifierGenerationHost {
    // GetModuleResolutionCache() any // TS-Go keeps this commented while the resolution cache model settles.
    fn symlink_cache(&self) -> Option<ts_symlinks::KnownSymlinks>;

    // GetFileIncludeReasons() any // TS-Go keeps this commented while the resolution cache model settles.
    fn common_source_directory(&self) -> String;
    fn global_typings_cache_location(&self) -> String;
    fn use_case_sensitive_file_names(&self) -> bool;
    fn current_directory(&self) -> String;

    fn project_reference_from_source(
        &self,
        path: ts_tspath::Path,
    ) -> Option<ts_tsoptions::SourceOutputAndProjectReference>;

    fn redirect_targets(&self, path: ts_tspath::Path) -> Vec<String>;

    fn source_of_project_reference_if_output_included(
        &self,
        file: &dyn ts_ast::HasFileName,
    ) -> String;

    fn file_exists(&self, path: &str) -> bool;

    fn nearest_ancestor_directory_with_package_json(&self, dirname: &str) -> String;

    fn package_json_info(&self, pkg_json_path: &str) -> Option<ts_packagejson::InfoCacheEntry>;

    fn default_resolution_mode_for_file(
        &self,
        file: &dyn ts_ast::HasFileName,
    ) -> ts_core::ResolutionMode;

    fn resolved_module_from_module_specifier(
        &self,
        file: &dyn ts_ast::HasFileName,
        module_specifier: &ts_ast::StringLiteralLike,
    ) -> Option<ts_module::ResolvedModule>;

    fn mode_for_usage_location(
        &self,
        file: &dyn ts_ast::HasFileName,
        module_specifier: &ts_ast::StringLiteralLike,
    ) -> ts_core::ResolutionMode;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImportModuleSpecifierPreference {
    #[default]
    None,
    Shortest,
    ProjectRelative,
    Relative,
    NonRelative,
}

impl ImportModuleSpecifierPreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Shortest => "shortest",
            Self::ProjectRelative => "project-relative",
            Self::Relative => "relative",
            Self::NonRelative => "non-relative",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImportModuleSpecifierEndingPreference {
    #[default]
    None,
    Auto,
    Minimal,
    Index,
    Js,
}

impl ImportModuleSpecifierEndingPreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Auto => "auto",
            Self::Minimal => "minimal",
            Self::Index => "index",
            Self::Js => "js",
        }
    }
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

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RelativePreferenceKind {
    #[default]
    Relative,
    NonRelative,
    Shortest,
    ExternalNonRelative,
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

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MatchingMode {
    #[default]
    Exact,
    Directory,
    Pattern,
}
