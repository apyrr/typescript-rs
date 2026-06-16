use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use crate::resolver::{CompilerOptions, PackageId, ParsedPatterns};

pub type ResolutionMode = ts_core::ResolutionMode;
pub type ModeAwareCache<T> = HashMap<ModeAwareCacheKey, T>;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct ModeAwareCacheKey {
    pub name: String,
    pub mode: ResolutionMode,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct ModuleResolutionCacheKey {
    pub containing_directory: String,
    pub module_name: String,
    pub resolution_mode: ResolutionMode,
    pub redirect_config_name: String,
}

pub type ResolutionDiagnostic = ts_ast::Diagnostic;

#[derive(Clone, Default)]
pub struct ResolvedModule {
    pub resolution_diagnostics: Vec<ResolutionDiagnostic>,
    pub resolved_file_name: String,
    pub original_path: String,
    pub extension: String,
    pub resolved_using_ts_extension: bool,
    pub package_id: PackageId,
    pub is_external_library_import: bool,
    pub alternate_result: String,
}

#[derive(Default)]
pub struct ModuleResolutionCache {
    pub cache: Mutex<HashMap<ModuleResolutionCacheKey, ResolvedModule>>,
}

impl ModuleResolutionCache {
    pub fn get(&self, key: &ModuleResolutionCacheKey) -> Option<ResolvedModule> {
        self.cache
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get(key)
            .cloned()
    }

    pub fn set(&self, key: ModuleResolutionCacheKey, value: ResolvedModule) {
        self.cache
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(key, value);
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct TypeRefDirectiveResolutionCacheKey {
    pub containing_directory: String,
    pub type_reference_name: String,
    pub resolution_mode: ResolutionMode,
    pub redirect_config_name: String,
    pub from_inferred_types_containing_file: bool,
}

#[derive(Clone, Default)]
pub struct ResolvedTypeReferenceDirective {
    pub resolution_diagnostics: Vec<ResolutionDiagnostic>,
    pub primary: bool,
    pub resolved_file_name: String,
    pub original_path: String,
    pub package_id: PackageId,
    pub is_external_library_import: bool,
}

#[derive(Default)]
pub struct TypeRefDirectiveResolutionCache {
    pub cache: Mutex<HashMap<TypeRefDirectiveResolutionCacheKey, ResolvedTypeReferenceDirective>>,
}

impl TypeRefDirectiveResolutionCache {
    pub fn get(
        &self,
        key: &TypeRefDirectiveResolutionCacheKey,
    ) -> Option<ResolvedTypeReferenceDirective> {
        self.cache
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get(key)
            .cloned()
    }

    pub fn set(
        &self,
        key: TypeRefDirectiveResolutionCacheKey,
        value: ResolvedTypeReferenceDirective,
    ) {
        self.cache
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(key, value);
    }
}

#[derive(Default)]
pub struct FileLookupCache {
    pub cache: Mutex<HashMap<String, bool>>,
}

impl FileLookupCache {
    pub fn mark(&self, file_name: &str, exists: bool) -> Option<bool> {
        self.cache
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(file_name.to_string(), exists)
    }
}

#[derive(Default)]
pub struct Caches {
    pub file_lookup_cache: FileLookupCache,
    pub package_json_info_cache: ts_packagejson::InfoCache,
    pub module_resolution_cache: ModuleResolutionCache,
    pub type_ref_directive_resolution_cache: TypeRefDirectiveResolutionCache,
    // Cached representation for `core.CompilerOptions.paths`.
    // Doesn't handle other path patterns like in `typesVersions`.
    pub parsed_patterns_for_paths_once: OnceLock<ParsedPatterns>,
    pub parsed_patterns_for_paths: Option<ParsedPatterns>,
}

pub fn new_caches(
    current_directory: &str,
    use_case_sensitive_file_names: bool,
    _options: &CompilerOptions,
) -> Caches {
    Caches {
        package_json_info_cache: ts_packagejson::new_info_cache(
            current_directory.to_string(),
            use_case_sensitive_file_names,
        ),
        ..Caches::default()
    }
}

pub trait ResolvedProjectReference {
    fn config_name(&self) -> String;

    fn compiler_options(&self) -> CompilerOptions {
        CompilerOptions::default()
    }
}

pub fn get_redirect_config_name(redirect: Option<&dyn ResolvedProjectReference>) -> String {
    redirect
        .map(|redirect| redirect.config_name())
        .unwrap_or_default()
}
