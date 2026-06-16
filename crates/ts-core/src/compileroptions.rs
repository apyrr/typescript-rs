#![allow(dead_code)]

use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_collections::OrderedMap;
use ts_tspath as tspath;

use crate::{
    ModuleKind, SCRIPT_TARGET_LATEST_STANDARD, ScriptTarget, TS_FALSE, TS_TRUE, TS_UNKNOWN,
    Tristate,
};

//go:generate go tool golang.org/x/tools/cmd/stringer -type=ModuleKind -trimprefix=ModuleKind -output=modulekind_stringer_generated.go
//go:generate go tool golang.org/x/tools/cmd/stringer -type=ScriptTarget -trimprefix=ScriptTarget -output=scripttarget_stringer_generated.go
//go:generate npx dprint fmt modulekind_stringer_generated.go scripttarget_stringer_generated.go
// PORT NOTE: Rust stringer equivalents live in *_stringer_generated.rs.

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompilerOptions {
    #[serde(rename = "allowJs", skip_serializing_if = "Tristate::is_unknown")]
    pub allow_js: Tristate,
    #[serde(
        rename = "allowArbitraryExtensions",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub allow_arbitrary_extensions: Tristate,
    #[serde(
        rename = "allowImportingTsExtensions",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub allow_importing_ts_extensions: Tristate,
    #[serde(
        rename = "allowNonTsExtensions",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub allow_non_ts_extensions: Tristate,
    #[serde(
        rename = "allowUmdGlobalAccess",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub allow_umd_global_access: Tristate,
    #[serde(
        rename = "allowUnreachableCode",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub allow_unreachable_code: Tristate,
    #[serde(
        rename = "allowUnusedLabels",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub allow_unused_labels: Tristate,
    #[serde(
        rename = "assumeChangesOnlyAffectDirectDependencies",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub assume_changes_only_affect_direct_dependencies: Tristate,
    #[serde(rename = "checkJs", skip_serializing_if = "Tristate::is_unknown")]
    pub check_js: Tristate,
    #[serde(rename = "customConditions", skip_serializing_if = "Vec::is_empty")]
    pub custom_conditions: Vec<String>,
    #[serde(rename = "composite", skip_serializing_if = "Tristate::is_unknown")]
    pub composite: Tristate,
    #[serde(
        rename = "emitDeclarationOnly",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub emit_declaration_only: Tristate,
    #[serde(rename = "emitBOM", skip_serializing_if = "Tristate::is_unknown")]
    pub emit_bom: Tristate,
    #[serde(
        rename = "emitDecoratorMetadata",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub emit_decorator_metadata: Tristate,
    #[serde(rename = "declaration", skip_serializing_if = "Tristate::is_unknown")]
    pub declaration: Tristate,
    #[serde(rename = "declarationDir", skip_serializing_if = "String::is_empty")]
    pub declaration_dir: String,
    #[serde(
        rename = "declarationMap",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub declaration_map: Tristate,
    #[serde(
        rename = "deduplicatePackages",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub deduplicate_packages: Tristate,
    #[serde(
        rename = "disableSizeLimit",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub disable_size_limit: Tristate,
    #[serde(
        rename = "disableSourceOfProjectReferenceRedirect",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub disable_source_of_project_reference_redirect: Tristate,
    #[serde(
        rename = "disableSolutionSearching",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub disable_solution_searching: Tristate,
    #[serde(
        rename = "disableReferencedProjectLoad",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub disable_referenced_project_load: Tristate,
    #[serde(
        rename = "erasableSyntaxOnly",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub erasable_syntax_only: Tristate,
    #[serde(
        rename = "exactOptionalPropertyTypes",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub exact_optional_property_types: Tristate,
    #[serde(
        rename = "experimentalDecorators",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub experimental_decorators: Tristate,
    #[serde(
        rename = "forceConsistentCasingInFileNames",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub force_consistent_casing_in_file_names: Tristate,
    #[serde(
        rename = "isolatedModules",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub isolated_modules: Tristate,
    #[serde(
        rename = "isolatedDeclarations",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub isolated_declarations: Tristate,
    #[serde(rename = "ignoreConfig", skip_serializing_if = "Tristate::is_unknown")]
    pub ignore_config: Tristate,
    #[serde(
        rename = "ignoreDeprecations",
        skip_serializing_if = "String::is_empty"
    )]
    pub ignore_deprecations: String,
    #[serde(rename = "importHelpers", skip_serializing_if = "Tristate::is_unknown")]
    pub import_helpers: Tristate,
    #[serde(
        rename = "inlineSourceMap",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub inline_source_map: Tristate,
    #[serde(rename = "inlineSources", skip_serializing_if = "Tristate::is_unknown")]
    pub inline_sources: Tristate,
    #[serde(rename = "init", skip_serializing_if = "Tristate::is_unknown")]
    pub init: Tristate,
    #[serde(rename = "incremental", skip_serializing_if = "Tristate::is_unknown")]
    pub incremental: Tristate,
    #[serde(rename = "jsx", skip_serializing_if = "is_default")]
    pub jsx: JsxEmit,
    #[serde(rename = "jsxFactory", skip_serializing_if = "String::is_empty")]
    pub jsx_factory: String,
    #[serde(
        rename = "jsxFragmentFactory",
        skip_serializing_if = "String::is_empty"
    )]
    pub jsx_fragment_factory: String,
    #[serde(rename = "jsxImportSource", skip_serializing_if = "String::is_empty")]
    pub jsx_import_source: String,
    #[serde(rename = "lib", skip_serializing_if = "Vec::is_empty")]
    pub lib: Vec<String>,
    #[serde(
        rename = "libReplacement",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub lib_replacement: Tristate,
    #[serde(rename = "locale", skip_serializing_if = "String::is_empty")]
    pub locale: String,
    #[serde(rename = "mapRoot", skip_serializing_if = "String::is_empty")]
    pub map_root: String,
    #[serde(rename = "module", skip_serializing_if = "is_default")]
    pub module: ModuleKind,
    #[serde(rename = "moduleResolution", skip_serializing_if = "is_default")]
    pub module_resolution: ModuleResolutionKind,
    #[serde(rename = "moduleSuffixes", skip_serializing_if = "Vec::is_empty")]
    pub module_suffixes: Vec<String>,
    #[serde(rename = "moduleDetection", skip_serializing_if = "is_default")]
    pub module_detection: ModuleDetectionKind,
    #[serde(rename = "newLine", skip_serializing_if = "is_default")]
    pub new_line: NewLineKind,
    #[serde(rename = "noEmit", skip_serializing_if = "Tristate::is_unknown")]
    pub no_emit: Tristate,
    #[serde(rename = "noCheck", skip_serializing_if = "Tristate::is_unknown")]
    pub no_check: Tristate,
    #[serde(
        rename = "noErrorTruncation",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_error_truncation: Tristate,
    #[serde(
        rename = "noFallthroughCasesInSwitch",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_fallthrough_cases_in_switch: Tristate,
    #[serde(rename = "noImplicitAny", skip_serializing_if = "Tristate::is_unknown")]
    pub no_implicit_any: Tristate,
    #[serde(
        rename = "noImplicitThis",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_implicit_this: Tristate,
    #[serde(
        rename = "noImplicitReturns",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_implicit_returns: Tristate,
    #[serde(rename = "noEmitHelpers", skip_serializing_if = "Tristate::is_unknown")]
    pub no_emit_helpers: Tristate,
    #[serde(rename = "noLib", skip_serializing_if = "Tristate::is_unknown")]
    pub no_lib: Tristate,
    #[serde(
        rename = "noPropertyAccessFromIndexSignature",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_property_access_from_index_signature: Tristate,
    #[serde(
        rename = "noUncheckedIndexedAccess",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_unchecked_indexed_access: Tristate,
    #[serde(rename = "noEmitOnError", skip_serializing_if = "Tristate::is_unknown")]
    pub no_emit_on_error: Tristate,
    #[serde(
        rename = "noUnusedLocals",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_unused_locals: Tristate,
    #[serde(
        rename = "noUnusedParameters",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_unused_parameters: Tristate,
    #[serde(rename = "noResolve", skip_serializing_if = "Tristate::is_unknown")]
    pub no_resolve: Tristate,
    #[serde(
        rename = "noImplicitOverride",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_implicit_override: Tristate,
    #[serde(
        rename = "noUncheckedSideEffectImports",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_unchecked_side_effect_imports: Tristate,
    #[serde(rename = "outDir", skip_serializing_if = "String::is_empty")]
    pub out_dir: String,
    #[serde(rename = "paths", skip_serializing_if = "is_ordered_map_empty")]
    pub paths: OrderedMap<String, Vec<String>>,
    #[serde(skip)]
    pub paths_for_validation: OrderedMap<String, Value>,
    #[serde(
        rename = "preserveConstEnums",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub preserve_const_enums: Tristate,
    #[serde(
        rename = "preserveSymlinks",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub preserve_symlinks: Tristate,
    #[serde(rename = "project", skip_serializing_if = "String::is_empty")]
    pub project: String,
    #[serde(
        rename = "resolveJsonModule",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub resolve_json_module: Tristate,
    #[serde(
        rename = "resolvePackageJsonExports",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub resolve_package_json_exports: Tristate,
    #[serde(
        rename = "resolvePackageJsonImports",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub resolve_package_json_imports: Tristate,
    #[serde(
        rename = "removeComments",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub remove_comments: Tristate,
    #[serde(
        rename = "rewriteRelativeImportExtensions",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub rewrite_relative_import_extensions: Tristate,
    #[serde(rename = "reactNamespace", skip_serializing_if = "String::is_empty")]
    pub react_namespace: String,
    #[serde(rename = "rootDir", skip_serializing_if = "String::is_empty")]
    pub root_dir: String,
    #[serde(rename = "rootDirs", skip_serializing_if = "Vec::is_empty")]
    pub root_dirs: Vec<String>,
    #[serde(rename = "skipLibCheck", skip_serializing_if = "Tristate::is_unknown")]
    pub skip_lib_check: Tristate,
    #[serde(
        rename = "stableTypeOrdering",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub stable_type_ordering: Tristate,
    #[serde(rename = "strict", skip_serializing_if = "Tristate::is_unknown")]
    pub strict: Tristate,
    #[serde(
        rename = "strictBindCallApply",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub strict_bind_call_apply: Tristate,
    #[serde(
        rename = "strictBuiltinIteratorReturn",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub strict_builtin_iterator_return: Tristate,
    #[serde(
        rename = "strictFunctionTypes",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub strict_function_types: Tristate,
    #[serde(
        rename = "strictNullChecks",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub strict_null_checks: Tristate,
    #[serde(
        rename = "strictPropertyInitialization",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub strict_property_initialization: Tristate,
    #[serde(rename = "stripInternal", skip_serializing_if = "Tristate::is_unknown")]
    pub strip_internal: Tristate,
    #[serde(
        rename = "skipDefaultLibCheck",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub skip_default_lib_check: Tristate,
    #[serde(rename = "sourceMap", skip_serializing_if = "Tristate::is_unknown")]
    pub source_map: Tristate,
    #[serde(rename = "sourceRoot", skip_serializing_if = "String::is_empty")]
    pub source_root: String,
    #[serde(
        rename = "suppressOutputPathCheck",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub suppress_output_path_check: Tristate,
    #[serde(rename = "target", skip_serializing_if = "is_default")]
    pub target: ScriptTarget,
    #[serde(
        rename = "traceResolution",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub trace_resolution: Tristate,
    #[serde(rename = "tsBuildInfoFile", skip_serializing_if = "String::is_empty")]
    pub ts_build_info_file: String,
    #[serde(rename = "typeRoots", skip_serializing_if = "Vec::is_empty")]
    pub type_roots: Vec<String>,
    #[serde(skip)]
    pub type_roots_configured: bool,
    #[serde(rename = "types", skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    #[serde(
        rename = "useDefineForClassFields",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub use_define_for_class_fields: Tristate,
    #[serde(
        rename = "useUnknownInCatchVariables",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub use_unknown_in_catch_variables: Tristate,
    #[serde(
        rename = "verbatimModuleSyntax",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub verbatim_module_syntax: Tristate,
    #[serde(
        rename = "maxNodeModuleJsDepth",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_node_module_js_depth: Option<usize>,

    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(
        rename = "allowSyntheticDefaultImports",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub allow_synthetic_default_imports: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(rename = "alwaysStrict", skip_serializing_if = "Tristate::is_unknown")]
    pub always_strict: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(rename = "baseUrl", skip_serializing_if = "String::is_empty")]
    pub base_url: String,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(
        rename = "downlevelIteration",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub downlevel_iteration: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(
        rename = "esModuleInterop",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub es_module_interop: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(rename = "outFile", skip_serializing_if = "String::is_empty")]
    pub out_file: String,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(rename = "charset", skip_serializing_if = "String::is_empty")]
    pub charset: String,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(
        rename = "keyofStringsOnly",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub keyof_strings_only: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(
        rename = "noImplicitUseStrict",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_implicit_use_strict: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(
        rename = "noStrictGenericChecks",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_strict_generic_checks: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(rename = "out", skip_serializing_if = "String::is_empty")]
    pub out: String,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(
        rename = "suppressExcessPropertyErrors",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub suppress_excess_property_errors: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(
        rename = "suppressImplicitAnyIndexErrors",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub suppress_implicit_any_index_errors: Tristate,
    // Deprecated: Do not use outside of options parsing and validation.
    #[serde(skip)]
    pub target_is_es3: bool,

    // Internal fields
    #[serde(rename = "configFilePath", skip_serializing_if = "String::is_empty")]
    pub config_file_path: String,
    #[serde(
        rename = "noDtsResolution",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_dts_resolution: Tristate,
    #[serde(rename = "pathsBasePath", skip_serializing_if = "String::is_empty")]
    pub paths_base_path: String,
    #[serde(rename = "diagnostics", skip_serializing_if = "Tristate::is_unknown")]
    pub diagnostics: Tristate,
    #[serde(
        rename = "extendedDiagnostics",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub extended_diagnostics: Tristate,
    #[serde(
        rename = "generateCpuProfile",
        skip_serializing_if = "String::is_empty"
    )]
    pub generate_cpu_profile: String,
    #[serde(rename = "generateTrace", skip_serializing_if = "String::is_empty")]
    pub generate_trace: String,
    #[serde(
        rename = "listEmittedFiles",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub list_emitted_files: Tristate,
    #[serde(rename = "listFiles", skip_serializing_if = "Tristate::is_unknown")]
    pub list_files: Tristate,
    #[serde(rename = "explainFiles", skip_serializing_if = "Tristate::is_unknown")]
    pub explain_files: Tristate,
    #[serde(rename = "listFilesOnly", skip_serializing_if = "Tristate::is_unknown")]
    pub list_files_only: Tristate,
    #[serde(
        rename = "noEmitForJsFiles",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub no_emit_for_js_files: Tristate,
    #[serde(
        rename = "preserveWatchOutput",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub preserve_watch_output: Tristate,
    #[serde(rename = "pretty", skip_serializing_if = "Tristate::is_unknown")]
    pub pretty: Tristate,
    #[serde(rename = "version", skip_serializing_if = "Tristate::is_unknown")]
    pub version: Tristate,
    #[serde(rename = "watch", skip_serializing_if = "Tristate::is_unknown")]
    pub watch: Tristate,
    #[serde(rename = "showConfig", skip_serializing_if = "Tristate::is_unknown")]
    pub show_config: Tristate,
    #[serde(rename = "build", skip_serializing_if = "Tristate::is_unknown")]
    pub build: Tristate,
    #[serde(rename = "help", skip_serializing_if = "Tristate::is_unknown")]
    pub help: Tristate,
    #[serde(rename = "all", skip_serializing_if = "Tristate::is_unknown")]
    pub all: Tristate,

    #[serde(rename = "pprofDir", skip_serializing_if = "String::is_empty")]
    pub pprof_dir: String,
    #[serde(
        rename = "singleThreaded",
        skip_serializing_if = "Tristate::is_unknown"
    )]
    pub single_threaded: Tristate,
    #[serde(rename = "quiet", skip_serializing_if = "Tristate::is_unknown")]
    pub quiet: Tristate,
    #[serde(rename = "checkers", skip_serializing_if = "Option::is_none")]
    pub checkers: Option<usize>,
}

fn is_default<T>(value: &T) -> bool
where
    T: Default + PartialEq,
{
    value == &T::default()
}

fn is_ordered_map_empty<K, V>(value: &OrderedMap<K, V>) -> bool
where
    K: Eq + Hash + Clone,
{
    value.size() == 0
}

// noCopy may be embedded into structs which must not be copied
// after the first use.
//
// See https://golang.org/issues/8005#issuecomment-190753527
// for details.
struct NoCopy;

// Lock is a no-op used by -copylocks checker from `go vet`.
impl NoCopy {
    fn lock(&self) {}
    fn unlock(&self) {}
}

pub fn empty_compiler_options() -> CompilerOptions {
    CompilerOptions::default()
}

impl CompilerOptions {
    // Clone creates a shallow copy of the CompilerOptions.
    fn clone_options(&self) -> CompilerOptions {
        // TODO: this could be generated code instead of reflection.
        <CompilerOptions as Clone>::clone(self)
    }

    pub fn get_emit_script_target(&self) -> ScriptTarget {
        if self.target != ScriptTarget::None {
            return self.target;
        }
        SCRIPT_TARGET_LATEST_STANDARD
    }

    pub fn get_emit_module_kind(&self) -> ModuleKind {
        if self.module != ModuleKind::None {
            return self.module;
        }

        let target = self.get_emit_script_target();
        if target == ScriptTarget::ESNext {
            return ModuleKind::ESNext;
        }
        if target >= ScriptTarget::ES2022 {
            return ModuleKind::ES2022;
        }
        if target >= ScriptTarget::ES2020 {
            return ModuleKind::ES2020;
        }
        if target >= ScriptTarget::ES2015 {
            return ModuleKind::ES2015;
        }
        ModuleKind::CommonJS
    }

    pub fn get_module_resolution_kind(&self) -> ModuleResolutionKind {
        match self.module_resolution {
            ModuleResolutionKind::Unknown
            | ModuleResolutionKind::Classic
            | ModuleResolutionKind::Node10 => match self.get_emit_module_kind() {
                ModuleKind::Node16 | ModuleKind::Node18 | ModuleKind::Node20 => {
                    ModuleResolutionKind::Node16
                }
                ModuleKind::NodeNext => ModuleResolutionKind::NodeNext,
                _ => ModuleResolutionKind::Bundler,
            },
            _ => self.module_resolution,
        }
    }

    pub fn get_emit_module_detection_kind(&self) -> ModuleDetectionKind {
        if self.module_detection != ModuleDetectionKind::None {
            return self.module_detection;
        }
        let module_kind = self.get_emit_module_kind();
        if ModuleKind::Node16 <= module_kind && module_kind <= ModuleKind::NodeNext {
            return ModuleDetectionKind::Force;
        }
        ModuleDetectionKind::Auto
    }

    pub fn get_resolve_package_json_exports(&self) -> bool {
        self.resolve_package_json_exports.is_true_or_unknown()
    }

    pub fn get_resolve_package_json_imports(&self) -> bool {
        self.resolve_package_json_imports.is_true_or_unknown()
    }

    pub fn get_allow_importing_ts_extensions(&self) -> bool {
        self.allow_importing_ts_extensions.is_true()
            || self.rewrite_relative_import_extensions.is_true()
    }

    pub fn allow_importing_ts_extensions_from(&self, file_name: &str) -> bool {
        self.get_allow_importing_ts_extensions() || tspath::is_declaration_file_name(file_name)
    }

    pub fn get_resolve_json_module(&self) -> bool {
        if self.resolve_json_module != TS_UNKNOWN {
            return self.resolve_json_module == TS_TRUE;
        }
        match self.get_emit_module_kind() {
            // TODO in 6.0: add Node16/Node18
            ModuleKind::Node20 | ModuleKind::NodeNext => return true,
            _ => {}
        }
        self.get_module_resolution_kind() == ModuleResolutionKind::Bundler
    }

    pub fn should_preserve_const_enums(&self) -> bool {
        self.preserve_const_enums == TS_TRUE || self.get_isolated_modules()
    }

    pub fn get_allow_js(&self) -> bool {
        if self.allow_js != TS_UNKNOWN {
            return self.allow_js == TS_TRUE;
        }
        self.check_js == TS_TRUE
    }

    pub fn get_jsxtransform_enabled(&self) -> bool {
        let jsx = self.jsx;
        jsx == JsxEmit::React || jsx == JsxEmit::ReactJSX || jsx == JsxEmit::ReactJSXDev
    }

    pub fn get_jsx_transform_enabled(&self) -> bool {
        self.get_jsxtransform_enabled()
    }

    pub fn get_strict_option_value(&self, value: Tristate) -> bool {
        if value != TS_UNKNOWN {
            return value == TS_TRUE;
        }
        self.strict != TS_FALSE
    }

    pub fn get_effective_type_roots(&self, current_directory: &str) -> (Vec<String>, bool) {
        if self.type_roots_configured {
            return (self.type_roots.clone(), true);
        }
        let base_dir = if !self.config_file_path.is_empty() {
            tspath::get_directory_path(&self.config_file_path)
        } else {
            if current_directory.is_empty() {
                // This was accounted for in the TS codebase, but only for third-party API usage
                // where the module resolution host does not provide a getCurrentDirectory().
                panic!(
                    "cannot get effective type roots without a config file path or current directory"
                );
            }
            current_directory.to_string()
        };

        let mut type_roots = Vec::with_capacity(base_dir.matches('/').count());
        tspath::for_each_ancestor_directory(base_dir, |dir| {
            type_roots.push(tspath::combine_paths(
                &tspath::combine_paths(dir, &["node_modules"]),
                &["@types"],
            ));
            None::<()>
        });
        (type_roots, false)
    }

    // UsesWildcardTypes returns true if this option's types array includes "*"
    pub fn uses_wildcard_types(&self) -> bool {
        self.types.iter().any(|value| value == "*")
    }

    pub fn get_isolated_modules(&self) -> bool {
        self.isolated_modules == TS_TRUE || self.verbatim_module_syntax == TS_TRUE
    }

    pub fn is_incremental(&self) -> bool {
        self.incremental.is_true() || self.composite.is_true()
    }

    pub fn get_emit_standard_class_fields(&self) -> bool {
        self.use_define_for_class_fields != TS_FALSE
            && self.get_emit_script_target() >= ScriptTarget::ES2022
    }

    pub fn get_use_define_for_class_fields(&self) -> bool {
        if self.use_define_for_class_fields == TS_UNKNOWN {
            return self.get_emit_script_target() >= ScriptTarget::ES2022;
        }
        self.use_define_for_class_fields == TS_TRUE
    }

    pub fn get_emit_declarations(&self) -> bool {
        self.declaration.is_true() || self.composite.is_true()
    }

    pub fn get_are_declaration_maps_enabled(&self) -> bool {
        self.declaration_map == TS_TRUE && self.get_emit_declarations()
    }

    pub fn has_json_module_emit_enabled(&self) -> bool {
        !matches!(
            self.get_emit_module_kind(),
            ModuleKind::System | ModuleKind::UMD
        )
    }

    pub fn get_paths_base_path(&self, current_directory: &str) -> String {
        if self.paths.size() == 0 {
            return String::new();
        }
        if !self.paths_base_path.is_empty() {
            return self.paths_base_path.clone();
        }
        current_directory.to_string()
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct ModuleDetectionKind(pub i32);

impl ModuleDetectionKind {
    #[allow(non_upper_case_globals)]
    pub const None: ModuleDetectionKind = ModuleDetectionKind(0);
    #[allow(non_upper_case_globals)]
    pub const Auto: ModuleDetectionKind = ModuleDetectionKind(1);
    #[allow(non_upper_case_globals)]
    pub const Legacy: ModuleDetectionKind = ModuleDetectionKind(2);
    #[allow(non_upper_case_globals)]
    pub const Force: ModuleDetectionKind = ModuleDetectionKind(3);
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct ModuleResolutionKind(pub i32);

impl ModuleResolutionKind {
    #[allow(non_upper_case_globals)]
    pub const Unknown: ModuleResolutionKind = ModuleResolutionKind(0);
    // Deprecated: Do not use outside of options parsing and validation.
    #[allow(non_upper_case_globals)]
    pub const Classic: ModuleResolutionKind = ModuleResolutionKind(1);
    // Deprecated: Do not use outside of options parsing and validation.
    #[allow(non_upper_case_globals)]
    pub const Node10: ModuleResolutionKind = ModuleResolutionKind(2);
    // Starting with node16, node's module resolver has significant departures from traditional cjs resolution
    // to better support ECMAScript modules and their use within node - however more features are still being added.
    // TypeScript's Node ESM support was introduced after Node 12 went end-of-life, and Node 14 is the earliest stable
    // version that supports both pattern trailers - *but*, Node 16 is the first version that also supports ECMAScript 2022.
    // In turn, we offer both a `NodeNext` moving resolution target, and a `Node16` version-anchored resolution target
    #[allow(non_upper_case_globals)]
    pub const Node16: ModuleResolutionKind = ModuleResolutionKind(3);
    #[allow(non_upper_case_globals)]
    pub const NodeNext: ModuleResolutionKind = ModuleResolutionKind(99); // Not simply `Node16` so that compiled code linked against TS can use the `Next` value reliably (same as with `ModuleKind`)
    #[allow(non_upper_case_globals)]
    pub const Bundler: ModuleResolutionKind = ModuleResolutionKind(100);
}

pub fn module_kind_to_module_resolution_kind() -> HashMap<ModuleKind, ModuleResolutionKind> {
    HashMap::from([
        (ModuleKind::Node16, ModuleResolutionKind::Node16),
        (ModuleKind::NodeNext, ModuleResolutionKind::NodeNext),
    ])
}

// We don't use stringer on this for now, because these values
// are user-facing in --traceResolution, and stringer currently
// lacks the ability to remove the "ModuleResolutionKind" prefix
// when generating code for multiple types into the same output
// file. Additionally, since there's no TS equivalent of
// `ModuleResolutionKindUnknown`, we want to panic on that case,
// as it probably represents a mistake when porting TS to Go.
impl ModuleResolutionKind {
    pub fn string(self) -> &'static str {
        match self {
            ModuleResolutionKind::Unknown => {
                panic!("should not use zero value of ModuleResolutionKind")
            }
            ModuleResolutionKind::Classic => "Classic",
            ModuleResolutionKind::Node10 => "Node10",
            ModuleResolutionKind::Node16 => "Node16",
            ModuleResolutionKind::NodeNext => "NodeNext",
            ModuleResolutionKind::Bundler => "Bundler",
            _ => panic!("unhandled case in ModuleResolutionKind.String"),
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct NewLineKind(pub i32);

impl NewLineKind {
    #[allow(non_upper_case_globals)]
    pub const None: NewLineKind = NewLineKind(0);
    #[allow(non_upper_case_globals)]
    pub const CRLF: NewLineKind = NewLineKind(1);
    #[allow(non_upper_case_globals)]
    pub const LF: NewLineKind = NewLineKind(2);
}

pub fn get_new_line_kind(s: &str) -> NewLineKind {
    match s {
        "\r\n" => NewLineKind::CRLF,
        "\n" => NewLineKind::LF,
        _ => NewLineKind::None,
    }
}

impl NewLineKind {
    pub fn get_new_line_character(self) -> &'static str {
        if self == NewLineKind::CRLF {
            "\r\n"
        } else {
            "\n"
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct JsxEmit(pub i32);

impl JsxEmit {
    #[allow(non_upper_case_globals)]
    pub const None: JsxEmit = JsxEmit(0);
    #[allow(non_upper_case_globals)]
    pub const Preserve: JsxEmit = JsxEmit(1);
    #[allow(non_upper_case_globals)]
    pub const ReactNative: JsxEmit = JsxEmit(2);
    #[allow(non_upper_case_globals)]
    pub const React: JsxEmit = JsxEmit(3);
    #[allow(non_upper_case_globals)]
    pub const ReactJSX: JsxEmit = JsxEmit(4);
    #[allow(non_upper_case_globals)]
    pub const ReactJSXDev: JsxEmit = JsxEmit(5);
}

impl JsxEmit {
    pub fn string(self) -> &'static str {
        match self {
            JsxEmit::None => panic!("should not use zero value of JsxEmit"),
            JsxEmit::Preserve => "preserve",
            JsxEmit::ReactNative => "react-native",
            JsxEmit::React => "react",
            JsxEmit::ReactJSX => "react-jsx",
            JsxEmit::ReactJSXDev => "react-jsxdev",
            _ => panic!("unhandled case in JsxEmit.String"),
        }
    }
}
