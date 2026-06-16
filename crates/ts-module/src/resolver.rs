use std::collections::HashSet;

use ts_ast as ast;
use ts_collections::OrderedMap;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_packagejson as packagejson;
use ts_stringutil as stringutil;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::{
    Caches, EXTENSIONS_DECLARATION, EXTENSIONS_JAVASCRIPT, EXTENSIONS_JSON, EXTENSIONS_TYPESCRIPT,
    Extensions, NODE_RESOLUTION_FEATURES_ALL, NODE_RESOLUTION_FEATURES_EXPORTS,
    NODE_RESOLUTION_FEATURES_EXPORTS_PATTERN_TRAILERS, NODE_RESOLUTION_FEATURES_IMPORTS,
    NODE_RESOLUTION_FEATURES_IMPORTS_PATTERN_ROOT, NODE_RESOLUTION_FEATURES_NODE_NEXT_DEFAULT,
    NODE_RESOLUTION_FEATURES_NODE16_DEFAULT, NODE_RESOLUTION_FEATURES_NONE,
    NODE_RESOLUTION_FEATURES_SELF_NAME, NodeResolutionFeatures, ResolvedModule,
    ResolvedProjectReference, ResolvedTypeReferenceDirective, new_caches,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageId {
    pub name: String,
    pub sub_module_name: String,
    pub version: String,
    pub peer_dependencies: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Resolved {
    pub path: String,
    pub extension: String,
    pub package_id: PackageId,
    pub original_path: String,
    pub resolved_using_ts_extension: bool,
}

impl Resolved {
    pub fn should_continue_searching(&self) -> bool {
        false
    }

    pub fn is_resolved(&self) -> bool {
        !self.path.is_empty()
    }
}

pub fn continue_searching() -> Option<Resolved> {
    None
}

pub fn unresolved() -> Resolved {
    Resolved::default()
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagAndArgs {
    pub message: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tracer {
    pub traces: Vec<DiagAndArgs>,
}

impl Tracer {
    pub fn write(&mut self, diag: &str, args: Vec<String>) {
        self.traces.push(DiagAndArgs {
            message: diag.to_string(),
            args,
        });
    }

    pub fn write_message(&mut self, message: &'static diagnostics::Message, args: Vec<String>) {
        self.write(&diagnostics::format(&message.string(), args), Vec::new());
    }

    pub fn get_traces(&self) -> Vec<DiagAndArgs> {
        self.traces.clone()
    }
}

pub trait ResolutionHost: Send + Sync {
    fn get_current_directory(&self) -> String;
    fn fs(&self) -> &dyn vfs::Fs;

    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs().use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.fs().file_exists(path)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.fs().directory_exists(path)
    }

    fn realpath(&self, path: &str) -> String {
        self.fs().realpath(path)
    }
}

pub type ResolutionHostBox = Box<dyn ResolutionHost + Send + Sync>;

impl<T: ResolutionHost + ?Sized> ResolutionHost for Box<T> {
    fn get_current_directory(&self) -> String {
        (**self).get_current_directory()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        (**self).fs()
    }
}

pub type CompilerOptions = core::CompilerOptions;

pub struct ResolutionState<'a, H = ResolutionHostBox> {
    pub resolver: &'a Resolver<H>,
    pub tracer: Option<Tracer>,

    // request fields
    pub name: String,
    pub containing_directory: String,
    pub is_config_lookup: bool,
    pub features: NodeResolutionFeatures,
    pub esm_mode: bool,
    pub conditions: Vec<String>,
    pub extensions: Extensions,
    pub compiler_options: CompilerOptions,
    pub resolve_package_directory_only: bool,

    // state fields
    // candidateEndingIsFromConfig is set when the candidate file extension originated from
    // configuration (package.json fields, tsconfig.json paths entries, or wildcard substitutions)
    // rather than from the module specifier written in source code. When true, resolvedUsingTsExtension
    // is suppressed so the checker does not attempt to extract a TS extension from the original specifier.
    pub candidate_ending_is_from_config: bool,
    pub resolved_package_directory: bool,
    pub diagnostics: Vec<ast::Diagnostic>,
}

pub struct NewResolutionStateParams<'a, H: ResolutionHost> {
    pub name: &'a str,
    pub containing_directory: &'a str,
    pub is_type_reference_directive: bool,
    pub resolution_mode: core::ResolutionMode,
    pub compiler_options: &'a CompilerOptions,
    pub redirected_reference: Option<&'a dyn ResolvedProjectReference>,
    pub resolver: &'a Resolver<H>,
    pub trace_builder: Option<Tracer>,
}

pub fn new_resolution_state<'a, H: ResolutionHost>(
    params: NewResolutionStateParams<'a, H>,
) -> ResolutionState<'a, H> {
    let NewResolutionStateParams {
        name,
        containing_directory,
        is_type_reference_directive,
        resolution_mode,
        compiler_options,
        redirected_reference,
        resolver,
        trace_builder,
    } = params;

    let compiler_options =
        get_compiler_options_with_redirect(compiler_options, redirected_reference);
    let extensions = if is_type_reference_directive {
        EXTENSIONS_DECLARATION
    } else if compiler_options.no_dts_resolution == core::TS_TRUE {
        crate::EXTENSIONS_IMPLEMENTATION_FILES
    } else {
        EXTENSIONS_TYPESCRIPT | EXTENSIONS_JAVASCRIPT | EXTENSIONS_DECLARATION
    };
    let mut state = ResolutionState {
        resolver,
        tracer: trace_builder,
        name: name.to_string(),
        containing_directory: containing_directory.to_string(),
        compiler_options,
        extensions,
        ..ResolutionState::default_for(resolver)
    };
    if !is_type_reference_directive && state.compiler_options.get_resolve_json_module() {
        state.extensions |= EXTENSIONS_JSON;
    }
    match state.compiler_options.get_module_resolution_kind() {
        core::ModuleResolutionKind::Node16 => {
            state.features = NODE_RESOLUTION_FEATURES_NODE16_DEFAULT;
            state.esm_mode = resolution_mode == core::ModuleKind::ESNext;
            state.conditions = get_conditions(&state.compiler_options, resolution_mode);
        }
        core::ModuleResolutionKind::NodeNext => {
            state.features = NODE_RESOLUTION_FEATURES_NODE_NEXT_DEFAULT;
            state.esm_mode = resolution_mode == core::ModuleKind::ESNext;
            state.conditions = get_conditions(&state.compiler_options, resolution_mode);
        }
        core::ModuleResolutionKind::Bundler => {
            state.features = get_node_resolution_features(&state.compiler_options);
            state.conditions = get_conditions(&state.compiler_options, resolution_mode);
        }
        _ => {}
    }
    state
}

impl<'a, H> ResolutionState<'a, H> {
    fn default_for(resolver: &'a Resolver<H>) -> Self {
        Self {
            resolver,
            tracer: None,
            name: String::new(),
            containing_directory: String::new(),
            is_config_lookup: false,
            features: NODE_RESOLUTION_FEATURES_NONE,
            esm_mode: false,
            conditions: Vec::new(),
            extensions: 0,
            compiler_options: CompilerOptions::default(),
            resolve_package_directory_only: false,
            candidate_ending_is_from_config: false,
            resolved_package_directory: false,
            diagnostics: Vec::new(),
        }
    }
}

impl<'a, H: ResolutionHost> ResolutionState<'a, H> {
    fn trace(&mut self, message: &'static diagnostics::Message, args: Vec<String>) {
        if let Some(tracer) = &mut self.tracer {
            tracer.write_message(message, args);
        }
    }

    fn mark_file_lookup_for_trace(&mut self, file_name: &str, exists: bool) -> Option<bool> {
        if self.tracer.is_none() {
            return None;
        }
        self.resolver
            .caches
            .file_lookup_cache
            .mark(file_name, exists)
    }

    fn resolve_node_like(&mut self) -> ResolvedModule {
        let mode = if self.esm_mode { "ESM" } else { "CJS" };
        let conditions = self
            .conditions
            .iter()
            .map(|condition| format!("'{condition}'"))
            .collect::<Vec<_>>()
            .join(", ");
        self.trace(
            &diagnostics::Resolving_in_0_mode_with_conditions_1,
            vec![mode.to_string(), conditions],
        );
        let result = self.resolve_node_like_worker();
        if self.resolved_package_directory
            && !self.is_config_lookup
            && self.features & NODE_RESOLUTION_FEATURES_EXPORTS != 0
            && self.extensions & (EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION) != 0
            && !tspath::is_external_module_name_relative(&self.name)
            && result.as_ref().is_some_and(Resolved::is_resolved)
            && result
                .as_ref()
                .is_some_and(|resolved| resolved.path.contains("/node_modules/"))
            && result.as_ref().is_some_and(|resolved| {
                !crate::extension_is_ok(
                    EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION,
                    &resolved.extension,
                )
            })
            && self
                .conditions
                .iter()
                .any(|condition| condition == "import")
        {
            let mut resolved_module = self.create_resolved_module_handling_symlink(result);
            self.trace(
                &diagnostics::Resolution_of_non_relative_name_failed_trying_with_modern_Node_resolution_features_disabled_to_see_if_npm_library_needs_configuration_update,
                Vec::new(),
            );
            self.features &= !NODE_RESOLUTION_FEATURES_EXPORTS;
            self.extensions &= EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION;
            let diagnostics_count = self.diagnostics.len();
            if let Some(diagnostic_result) = self.resolve_node_like_worker() {
                let diagnostic_result =
                    self.create_resolved_module_handling_symlink(Some(diagnostic_result));
                if diagnostic_result.is_resolved() && diagnostic_result.is_external_library_import {
                    resolved_module.alternate_result = diagnostic_result.resolved_file_name;
                }
            }
            self.diagnostics.truncate(diagnostics_count);
            return resolved_module;
        }
        self.create_resolved_module_handling_symlink(result)
    }

    fn resolve_node_like_worker(&mut self) -> Option<Resolved> {
        if let Some(resolved) = self.try_load_module_using_optional_resolution_settings() {
            return Some(resolved);
        }

        if !tspath::is_external_module_name_relative(&self.name) {
            if self.features & NODE_RESOLUTION_FEATURES_IMPORTS != 0
                && self.name.starts_with('#')
                && let Some(resolved) = self.load_module_from_imports()
            {
                return Some(resolved);
            }
            if self.features & NODE_RESOLUTION_FEATURES_SELF_NAME != 0
                && let Some(resolved) = self.load_module_from_self_name_reference()
            {
                return Some(resolved);
            }
            if self.name.contains(':') {
                self.trace(
                    &diagnostics::Skipping_module_0_that_looks_like_an_absolute_URI_target_file_types_Colon_1,
                    vec![
                        self.name.clone(),
                        crate::extensions_to_string(self.extensions),
                    ],
                );
                return Some(unresolved());
            }
            self.trace(
                &diagnostics::Loading_module_0_from_node_modules_folder_target_file_types_Colon_1,
                vec![
                    self.name.clone(),
                    crate::extensions_to_string(self.extensions),
                ],
            );
            if let Some(resolved) = self.load_module_from_nearest_node_modules_directory(false) {
                return Some(resolved);
            }
            if self.extensions & EXTENSIONS_DECLARATION != 0
                && let Some(resolved) = self.resolve_from_type_root()
            {
                return Some(resolved);
            }
        } else {
            let candidate =
                normalize_path_for_cjs_resolution(&self.containing_directory, &self.name);
            return self.node_load_module_by_relative_name(self.extensions, &candidate, true);
        }

        Some(unresolved())
    }

    fn resolve_type_reference_directive(
        &mut self,
        type_roots: &[String],
        from_config: bool,
        from_inferred_types_containing_file: bool,
    ) -> ResolvedTypeReferenceDirective {
        if !type_roots.is_empty() {
            self.trace(
                &diagnostics::Resolving_with_primary_search_path_0,
                vec![type_roots.join(", ")],
            );
            for type_root in type_roots {
                let candidate = self.get_candidate_from_type_root(type_root);
                if !self.resolver.host.directory_exists(type_root) {
                    self.trace(
                        &diagnostics::Directory_0_does_not_exist_skipping_all_lookups_in_it,
                        vec![type_root.clone()],
                    );
                    continue;
                }
                if from_config
                    && let Some(mut resolved) =
                        self.load_module_from_file(EXTENSIONS_DECLARATION, &candidate)
                    && !resolved.path.is_empty()
                {
                    if let Some(package_directory) =
                        crate::parse_node_module_from_path(&resolved.path, false).strip_prefix("")
                        && !package_directory.is_empty()
                    {
                        resolved.package_id =
                            self.get_package_id(&resolved.path, package_directory);
                    }
                    return self.create_resolved_type_reference_directive(Some(resolved), true);
                }
                if let Some(resolved) =
                    self.load_node_module_from_directory(EXTENSIONS_DECLARATION, &candidate, true)
                {
                    return self.create_resolved_type_reference_directive(Some(resolved), true);
                }
            }
        } else {
            self.trace(
                &diagnostics::Root_directory_cannot_be_determined_skipping_primary_search_paths,
                Vec::new(),
            );
        }

        let resolved = if !from_config || !from_inferred_types_containing_file {
            self.trace(
                &diagnostics::Looking_up_in_node_modules_folder_initial_location_0,
                vec![self.containing_directory.clone()],
            );
            if !tspath::is_external_module_name_relative(&self.name) {
                self.load_module_from_nearest_node_modules_directory(false)
            } else {
                let candidate =
                    normalize_path_for_cjs_resolution(&self.containing_directory, &self.name);
                self.node_load_module_by_relative_name(EXTENSIONS_DECLARATION, &candidate, true)
            }
        } else {
            self.trace(
                &diagnostics::Resolving_type_reference_directive_for_program_that_specifies_custom_typeRoots_skipping_lookup_in_node_modules_folder,
                Vec::new(),
            );
            None
        };
        self.create_resolved_type_reference_directive(resolved, false)
    }

    fn get_candidate_from_type_root(&mut self, type_root: &str) -> String {
        let name_for_lookup = if type_root.ends_with("/node_modules/@types")
            || type_root.ends_with("/node_modules/@types/")
        {
            self.mangle_scoped_package_name(&self.name.clone())
        } else {
            self.name.clone()
        };
        tspath::combine_paths(type_root, &[&name_for_lookup])
    }

    fn mangle_scoped_package_name(&mut self, name: &str) -> String {
        let mangled = crate::mangle_scoped_package_name(name);
        if mangled != name {
            self.trace(
                &diagnostics::Scoped_package_detected_looking_in_0,
                vec![mangled.clone()],
            );
        }
        mangled
    }

    fn get_package_scope_for_path(
        &mut self,
        directory: &str,
    ) -> Option<packagejson::InfoCacheEntry> {
        tspath::for_each_ancestor_directory_stopping_at_global_cache(
            &self.resolver.typings_location,
            directory.to_string(),
            |ancestor| match self.get_package_json_info(ancestor) {
                Some(info) => (Some(info), true),
                None => (None, false),
            },
        )
        .flatten()
    }

    fn load_module_from_imports(&mut self) -> Option<Resolved> {
        if self.name == "#"
            || (self.name.starts_with("#/")
                && (self.features & NODE_RESOLUTION_FEATURES_IMPORTS_PATTERN_ROOT) == 0)
        {
            self.trace(
                &diagnostics::Invalid_import_specifier_0_has_no_possible_resolutions,
                vec![self.name.clone()],
            );
            return None;
        }
        let directory_path = tspath::get_normalized_absolute_path(
            &self.containing_directory,
            &self.resolver.host.get_current_directory(),
        );
        let Some(scope) = self.get_package_scope_for_path(&directory_path) else {
            self.trace(
                &diagnostics::Directory_0_has_no_containing_package_json_scope_Imports_will_not_resolve,
                vec![directory_path],
            );
            return None;
        };
        let Some(contents) = scope.get_contents() else {
            return None;
        };
        if contents.fields.path_fields.imports.json_value.type_
            != packagejson::JsonValueType::Object
        {
            // !!! Old compiler only checks for undefined, but then assumes `imports` is an object if present.
            // Maybe should have a new diagnostic for imports of an invalid type. Also, array should be handled?
            self.trace(
                &diagnostics::X_package_json_scope_0_has_no_imports_defined,
                vec![scope.package_directory.clone()],
            );
            return None;
        }

        if let Some(result) = self.load_module_from_exports_or_imports(
            self.extensions,
            &self.name.clone(),
            contents.fields.path_fields.imports.as_object().clone(),
            &scope,
            true,
        ) {
            return Some(result);
        }

        self.trace(
            &diagnostics::Import_specifier_0_does_not_exist_in_package_json_scope_at_path_1,
            vec![self.name.clone(), scope.package_directory],
        );
        None
    }

    fn load_module_from_self_name_reference(&mut self) -> Option<Resolved> {
        let directory_path = tspath::get_normalized_absolute_path(
            &self.containing_directory,
            &self.resolver.host.get_current_directory(),
        );
        let scope = self.get_package_scope_for_path(&directory_path)?;
        let contents = scope.get_contents()?;
        if contents.fields.path_fields.exports.json_value.is_falsy() {
            return None;
        }
        let (package_name, has_package_name) = contents.fields.header_fields.name.get_value();
        if !has_package_name {
            return None;
        }

        let parts = tspath::get_path_components(&self.name, "");
        let name_parts = tspath::get_path_components(&package_name, "");
        if parts.len() < name_parts.len() || parts[..name_parts.len()] != name_parts {
            return None;
        }

        let trailing_parts = &parts[name_parts.len()..];
        let subpath = if trailing_parts.is_empty() {
            ".".to_string()
        } else {
            let trailing_refs = trailing_parts
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            tspath::combine_paths(".", &trailing_refs)
        };

        if self.compiler_options.get_allow_js()
            && !self.containing_directory.contains("/node_modules/")
        {
            return self.load_module_from_exports(&scope, self.extensions, &subpath);
        }

        let priority_extensions =
            self.extensions & (EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION);
        let secondary_extensions =
            self.extensions & !(EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION);
        if let Some(resolved) = self.load_module_from_exports(&scope, priority_extensions, &subpath)
        {
            return Some(resolved);
        }
        self.load_module_from_exports(&scope, secondary_extensions, &subpath)
    }

    fn resolve_from_type_root(&mut self) -> Option<Resolved> {
        if !self.compiler_options.type_roots_configured {
            return None;
        }
        for type_root in self.compiler_options.type_roots.clone() {
            let candidate = self.get_candidate_from_type_root(&type_root);
            if !self.resolver.host.directory_exists(&type_root) {
                continue;
            }
            if let Some(mut resolved) =
                self.load_module_from_file(EXTENSIONS_DECLARATION, &candidate)
                && !resolved.path.is_empty()
            {
                let package_directory = crate::parse_node_module_from_path(&resolved.path, false);
                if !package_directory.is_empty() {
                    resolved.package_id = self.get_package_id(&resolved.path, &package_directory);
                }
                return Some(resolved);
            }
            if let Some(resolved) =
                self.load_node_module_from_directory(EXTENSIONS_DECLARATION, &candidate, true)
            {
                return Some(resolved);
            }
        }
        None
    }

    fn try_load_module_using_optional_resolution_settings(&mut self) -> Option<Resolved> {
        if let Some(resolved) = self.try_load_module_using_paths_if_eligible() {
            return Some(resolved);
        }
        if tspath::is_external_module_name_relative(&self.name) {
            return self.try_load_module_using_root_dirs();
        }
        None
    }

    fn try_load_module_using_paths_if_eligible(&mut self) -> Option<Resolved> {
        if self.compiler_options.paths.is_empty() || tspath::path_is_relative(&self.name) {
            return None;
        }
        self.trace(
            &diagnostics::X_paths_option_is_specified_looking_for_a_pattern_to_match_module_name_0,
            vec![self.name.clone()],
        );
        let base_directory = self
            .compiler_options
            .get_paths_base_path(&self.resolver.host.get_current_directory());
        let path_patterns = parse_pattern_keys(self.compiler_options.paths.keys().cloned());
        let paths = self.compiler_options.paths.clone();
        self.try_load_module_using_paths(
            self.extensions,
            &self.name.clone(),
            &base_directory,
            &paths,
            path_patterns.as_ref(),
        )
    }

    fn try_load_module_using_paths(
        &mut self,
        extensions: Extensions,
        module_name: &str,
        containing_directory: &str,
        paths: &OrderedMap<String, Vec<String>>,
        path_patterns: Option<&ParsedPatterns>,
    ) -> Option<Resolved> {
        self.try_load_module_using_paths_with_loader(
            extensions,
            module_name,
            containing_directory,
            paths,
            path_patterns,
            |state, extensions, candidate| {
                state.node_load_module_by_relative_name(extensions, candidate, true)
            },
        )
    }

    fn try_load_module_using_paths_with_loader(
        &mut self,
        extensions: Extensions,
        module_name: &str,
        containing_directory: &str,
        paths: &OrderedMap<String, Vec<String>>,
        path_patterns: Option<&ParsedPatterns>,
        mut loader: impl FnMut(&mut Self, Extensions, &str) -> Option<Resolved>,
    ) -> Option<Resolved> {
        let matched_pattern =
            path_patterns.and_then(|patterns| match_pattern_or_exact(patterns, module_name));
        let matched_pattern = matched_pattern?;
        self.trace(
            &diagnostics::Module_name_0_matched_pattern_1,
            vec![module_name.to_owned(), matched_pattern.clone()],
        );
        let matched_star = match core::try_parse_pattern(&matched_pattern) {
            pattern if pattern.is_valid() && pattern.star_index >= 0 => {
                let star_index = pattern.star_index as usize;
                let suffix_len = pattern.text.len() - star_index - 1;
                module_name[star_index..module_name.len() - suffix_len].to_string()
            }
            _ => String::new(),
        };
        for subst in paths.get(&matched_pattern).into_iter().flatten() {
            let path = subst.replacen('*', &matched_star, 1);
            let candidate =
                tspath::normalize_path(&tspath::combine_paths(containing_directory, &[&path]));
            self.trace(
                &diagnostics::Trying_substitution_0_candidate_module_location_Colon_1,
                vec![subst.clone(), path],
            );
            let extension_from_subst = tspath::try_get_extension_from_path(subst);
            if !extension_from_subst.is_empty()
                && let Some(path) = self.try_file(&candidate)
            {
                return Some(Resolved {
                    path,
                    extension: extension_from_subst.to_string(),
                    ..Default::default()
                });
            }
            let save_candidate_ending_is_from_config = self.candidate_ending_is_from_config;
            if !extension_from_subst.is_empty() {
                self.candidate_ending_is_from_config = true;
            }
            let resolved = loader(self, extensions, &candidate);
            self.candidate_ending_is_from_config = save_candidate_ending_is_from_config;
            if resolved.is_some() {
                return resolved;
            }
        }
        None
    }

    fn try_load_module_using_root_dirs(&mut self) -> Option<Resolved> {
        if self.compiler_options.root_dirs.is_empty() {
            return None;
        }
        self.trace(
            &diagnostics::X_rootDirs_option_is_set_using_it_to_resolve_relative_module_name_0,
            vec![self.name.clone()],
        );
        let candidate = tspath::normalize_path(&tspath::combine_paths(
            &self.containing_directory,
            &[&self.name],
        ));
        let mut matched_root_dir = String::new();
        let mut matched_normalized_prefix = String::new();
        for root_dir in self.compiler_options.root_dirs.clone() {
            let mut normalized_root = tspath::normalize_path(&root_dir);
            if !normalized_root.ends_with('/') {
                normalized_root.push('/');
            }
            let is_longest_matching_prefix = candidate.starts_with(&normalized_root)
                && (matched_normalized_prefix.is_empty()
                    || matched_normalized_prefix.len() < normalized_root.len());
            self.trace(
                &diagnostics::Checking_if_0_is_the_longest_matching_prefix_for_1_2,
                vec![
                    normalized_root.clone(),
                    candidate.clone(),
                    is_longest_matching_prefix.to_string(),
                ],
            );
            if is_longest_matching_prefix {
                matched_normalized_prefix = normalized_root.clone();
                matched_root_dir = root_dir.clone();
            }
        }
        if matched_normalized_prefix.is_empty() {
            return None;
        }
        self.trace(
            &diagnostics::Longest_matching_prefix_for_0_is_1,
            vec![candidate.clone(), matched_normalized_prefix.clone()],
        );
        let suffix = candidate[matched_normalized_prefix.len()..].to_string();
        self.trace(
            &diagnostics::Loading_0_from_the_root_dir_1_candidate_location_2,
            vec![
                suffix.clone(),
                matched_normalized_prefix.clone(),
                candidate.clone(),
            ],
        );
        if let Some(resolved) =
            self.node_load_module_by_relative_name(self.extensions, &candidate, true)
        {
            return Some(resolved);
        }
        self.trace(&diagnostics::Trying_other_entries_in_rootDirs, Vec::new());
        for root_dir in self.compiler_options.root_dirs.clone() {
            if root_dir == matched_root_dir {
                continue;
            }
            let candidate = tspath::combine_paths(&tspath::normalize_path(&root_dir), &[&suffix]);
            self.trace(
                &diagnostics::Loading_0_from_the_root_dir_1_candidate_location_2,
                vec![suffix.clone(), root_dir, candidate.clone()],
            );
            if let Some(resolved) =
                self.node_load_module_by_relative_name(self.extensions, &candidate, true)
            {
                return Some(resolved);
            }
        }
        self.trace(
            &diagnostics::Module_resolution_using_rootDirs_has_failed,
            Vec::new(),
        );
        None
    }

    fn load_module_from_nearest_node_modules_directory(
        &mut self,
        types_scope_only: bool,
    ) -> Option<Resolved> {
        let priority_extensions =
            self.extensions & (EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION);
        let secondary_extensions =
            self.extensions & !(EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION);
        if priority_extensions != 0
            && {
                self.trace(
                    &diagnostics::Searching_all_ancestor_node_modules_directories_for_preferred_extensions_Colon_0,
                    vec![crate::extensions_to_string(priority_extensions)],
                );
                true
            }
            && let Some(result) = self.load_module_from_nearest_node_modules_directory_worker(
                priority_extensions,
                types_scope_only,
            )
        {
            return Some(result);
        }
        if secondary_extensions != 0 && !types_scope_only {
            self.trace(
                &diagnostics::Searching_all_ancestor_node_modules_directories_for_fallback_extensions_Colon_0,
                vec![crate::extensions_to_string(secondary_extensions)],
            );
            return self.load_module_from_nearest_node_modules_directory_worker(
                secondary_extensions,
                types_scope_only,
            );
        }
        None
    }

    fn load_module_from_nearest_node_modules_directory_worker(
        &mut self,
        extensions: Extensions,
        types_scope_only: bool,
    ) -> Option<Resolved> {
        tspath::for_each_ancestor_directory(self.containing_directory.clone(), |directory| {
            if tspath::get_base_file_name(directory) == "node_modules" {
                return None;
            }
            self.load_module_from_immediate_node_modules_directory(
                extensions,
                directory,
                types_scope_only,
            )
        })
    }

    fn load_module_from_immediate_node_modules_directory(
        &mut self,
        extensions: Extensions,
        directory: &str,
        types_scope_only: bool,
    ) -> Option<Resolved> {
        let node_modules_folder = tspath::combine_paths(directory, &["node_modules"]);
        if !self.resolver.host.directory_exists(&node_modules_folder) {
            self.trace(
                &diagnostics::Directory_0_does_not_exist_skipping_all_lookups_in_it,
                vec![node_modules_folder],
            );
            return None;
        }
        if !types_scope_only
            && let Some(package_result) = self.load_module_from_specific_node_modules_directory(
                extensions,
                &self.name.clone(),
                &node_modules_folder,
            )
        {
            return Some(package_result);
        }
        if extensions & EXTENSIONS_DECLARATION != 0 {
            let node_modules_at_types = tspath::combine_paths(&node_modules_folder, &["@types"]);
            if !self.resolver.host.directory_exists(&node_modules_at_types) {
                self.trace(
                    &diagnostics::Directory_0_does_not_exist_skipping_all_lookups_in_it,
                    vec![node_modules_at_types],
                );
                return None;
            }
            let mangled = self.mangle_scoped_package_name(&self.name.clone());
            return self.load_module_from_specific_node_modules_directory(
                EXTENSIONS_DECLARATION,
                &mangled,
                &node_modules_at_types,
            );
        }
        None
    }

    fn load_module_from_specific_node_modules_directory(
        &mut self,
        extensions: Extensions,
        module_name: &str,
        node_modules_directory: &str,
    ) -> Option<Resolved> {
        let candidate = tspath::remove_trailing_directory_separator(&tspath::normalize_path(
            &tspath::combine_paths(node_modules_directory, &[module_name]),
        ))
        .to_string();
        let (package_name, rest) = crate::parse_package_name(module_name);
        let package_directory = if package_name.is_empty() {
            candidate.clone()
        } else {
            tspath::combine_paths(node_modules_directory, &[&package_name])
        };
        if self.resolve_package_directory_only {
            return self
                .resolver
                .host
                .directory_exists(&package_directory)
                .then_some(Resolved {
                    path: package_directory,
                    ..Default::default()
                });
        }
        let mut root_package_info = None;
        let mut package_info = self.get_package_json_info(&candidate);
        if !rest.is_empty() && package_info.is_some() {
            if self.features & NODE_RESOLUTION_FEATURES_EXPORTS != 0 {
                root_package_info = self.get_package_json_info(&package_directory);
            }
            let root_has_exports = root_package_info.as_ref().is_some_and(|info| {
                info.get_contents().is_some_and(|contents| {
                    !contents.fields.path_fields.exports.json_value.is_falsy()
                })
            });
            if !root_has_exports {
                if let Some(from_file) = self.load_module_from_file(extensions, &candidate) {
                    return Some(from_file);
                }
                if let Some(mut from_directory) = self.load_node_module_from_directory_worker(
                    extensions,
                    &candidate,
                    package_info.as_ref(),
                ) {
                    if let Some(package_info) = package_info.as_ref() {
                        from_directory.package_id =
                            self.get_package_id_from_info(&from_directory.path, package_info);
                    }
                    return Some(from_directory);
                }
            }
        }

        if !rest.is_empty() {
            package_info =
                root_package_info.or_else(|| self.get_package_json_info(&package_directory));
        }

        if let Some(package_info) = package_info.clone() {
            self.resolved_package_directory = true;
            if self.features & NODE_RESOLUTION_FEATURES_EXPORTS != 0
                && package_info.exists()
                && package_info.get_contents().is_some_and(|contents| {
                    !contents.fields.path_fields.exports.json_value.is_falsy()
                })
            {
                return self.load_module_from_exports(
                    &package_info,
                    extensions,
                    &if rest.is_empty() {
                        ".".to_string()
                    } else {
                        tspath::combine_paths(".", &[&rest])
                    },
                );
            }
            if !rest.is_empty()
                && let Some(version_paths) = self.get_version_paths(&package_info)
                && let Some(paths) = version_paths.get_paths()
            {
                self.trace(
                    &diagnostics::X_package_json_has_a_typesVersions_entry_0_that_matches_compiler_version_1_looking_for_a_pattern_to_match_module_name_2,
                    vec![
                        version_paths.version.clone(),
                        core::version().to_string(),
                        rest.clone(),
                    ],
                );
                let path_patterns = try_parse_patterns(paths);
                if let Some(from_paths) = self.try_load_module_using_paths_with_loader(
                    extensions,
                    &rest,
                    &package_directory,
                    paths,
                    path_patterns.as_ref(),
                    |state, extensions, candidate| {
                        if (!rest.is_empty() || !state.esm_mode)
                            && let Some(mut from_file) =
                                state.load_module_from_file(extensions, candidate)
                        {
                            from_file.package_id =
                                state.get_package_id_from_info(&from_file.path, &package_info);
                            return Some(from_file);
                        }
                        if let Some(mut from_directory) = state
                            .load_node_module_from_directory_worker(
                                extensions,
                                candidate,
                                Some(&package_info),
                            )
                        {
                            from_directory.package_id =
                                state.get_package_id_from_info(&from_directory.path, &package_info);
                            return Some(from_directory);
                        }
                        if rest.is_empty()
                            && package_info.exists()
                            && package_info.get_contents().is_some_and(|contents| {
                                matches!(
                                    contents.fields.path_fields.exports.json_value.type_,
                                    packagejson::JsonValueType::NotPresent
                                        | packagejson::JsonValueType::Null
                                )
                            })
                            && state.esm_mode
                        {
                            let index_path = tspath::combine_paths(candidate, &["index.js"]);
                            if let Some(mut index_result) =
                                state.load_module_from_file(extensions, &index_path)
                            {
                                index_result.package_id = state
                                    .get_package_id_from_info(&index_result.path, &package_info);
                                return Some(index_result);
                            }
                        }
                        None
                    },
                ) {
                    return Some(from_paths);
                }
            }
        }
        if let Some(mut from_file) = self.load_module_from_file(extensions, &candidate) {
            if let Some(package_info) = package_info.as_ref() {
                from_file.package_id = self.get_package_id_from_info(&from_file.path, package_info);
            }
            return Some(from_file);
        }
        let mut resolved = self.load_node_module_from_directory_worker(
            extensions,
            &candidate,
            package_info.as_ref(),
        );
        if resolved.is_none()
            && rest.is_empty()
            && self.esm_mode
            && package_info.as_ref().is_some_and(|package_info| {
                package_info.exists()
                    && package_info.get_contents().is_some_and(|contents| {
                        matches!(
                            contents.fields.path_fields.exports.json_value.type_,
                            packagejson::JsonValueType::NotPresent
                                | packagejson::JsonValueType::Null
                        )
                    })
            })
        {
            resolved = self.load_module_from_file(
                extensions,
                &tspath::combine_paths(&candidate, &["index.js"]),
            );
        }
        if let Some(mut resolved) = resolved {
            if let Some(package_info) = package_info.as_ref() {
                resolved.package_id = self.get_package_id_from_info(&resolved.path, package_info);
            }
            return Some(resolved);
        }
        None
    }

    fn load_module_from_exports(
        &mut self,
        package_info: &packagejson::InfoCacheEntry,
        extensions: Extensions,
        subpath: &str,
    ) -> Option<Resolved> {
        let contents = package_info.get_contents()?;
        let mut exports = contents.fields.path_fields.exports.clone();
        if exports.json_value.is_falsy() {
            return None;
        }

        if subpath == "." {
            let mut main_export = None;
            match exports.json_value.type_ {
                packagejson::JsonValueType::String | packagejson::JsonValueType::Array => {
                    main_export = Some(exports);
                }
                packagejson::JsonValueType::Object => {
                    if exports.is_conditions() {
                        main_export = Some(exports);
                    } else if let Some(dot) = exports.as_object().get(".") {
                        main_export =
                            Some(packagejson::ExportsOrImports::from_json_value(dot.clone()));
                    }
                }
                _ => {}
            }
            if let Some(main_export) = main_export {
                return self.load_module_from_target_export_or_import(
                    extensions,
                    subpath,
                    package_info,
                    false,
                    main_export,
                    "",
                    false,
                    ".",
                );
            }
        } else if exports.json_value.type_ == packagejson::JsonValueType::Object
            && exports.is_subpaths()
            && let Some(result) = self.load_module_from_exports_or_imports(
                extensions,
                subpath,
                exports.as_object().clone(),
                package_info,
                false,
            )
        {
            return Some(result);
        }

        self.trace(
            &diagnostics::Export_specifier_0_does_not_exist_in_package_json_scope_at_path_1,
            vec![subpath.to_string(), package_info.package_directory.clone()],
        );
        None
    }

    fn load_module_from_exports_or_imports(
        &mut self,
        extensions: Extensions,
        module_name: &str,
        lookup_table: serde_json::Map<String, serde_json::Value>,
        scope: &packagejson::InfoCacheEntry,
        is_imports: bool,
    ) -> Option<Resolved> {
        if !module_name.ends_with('/')
            && !module_name.contains('*')
            && let Some(target) = lookup_table.get(module_name)
        {
            return self.load_module_from_target_export_or_import(
                extensions,
                module_name,
                scope,
                is_imports,
                packagejson::ExportsOrImports::from_json_value(target.clone()),
                "",
                false,
                module_name,
            );
        }

        let mut expanding_keys = lookup_table
            .keys()
            .filter(|key| key.matches('*').count() == 1 || key.ends_with('/'))
            .cloned()
            .collect::<Vec<_>>();
        expanding_keys.sort_by(|a, b| compare_pattern_keys(a, b));

        for potential_target in expanding_keys {
            if self.features & NODE_RESOLUTION_FEATURES_EXPORTS_PATTERN_TRAILERS != 0
                && matches_pattern_with_trailer(&potential_target, module_name)
            {
                let Some(target) = lookup_table.get(&potential_target) else {
                    continue;
                };
                let star_pos = potential_target
                    .find('*')
                    .expect("pattern trailer target should contain '*'");
                let suffix_len = potential_target.len() - 1 - star_pos;
                let subpath = &module_name[star_pos..module_name.len() - suffix_len];
                return self.load_module_from_target_export_or_import(
                    extensions,
                    module_name,
                    scope,
                    is_imports,
                    packagejson::ExportsOrImports::from_json_value(target.clone()),
                    subpath,
                    true,
                    &potential_target,
                );
            } else if potential_target.ends_with('*')
                && module_name.starts_with(&potential_target[..potential_target.len() - 1])
            {
                let Some(target) = lookup_table.get(&potential_target) else {
                    continue;
                };
                let subpath = &module_name[potential_target.len() - 1..];
                return self.load_module_from_target_export_or_import(
                    extensions,
                    module_name,
                    scope,
                    is_imports,
                    packagejson::ExportsOrImports::from_json_value(target.clone()),
                    subpath,
                    true,
                    &potential_target,
                );
            }
            if module_name.starts_with(&potential_target) {
                let Some(target) = lookup_table.get(&potential_target) else {
                    continue;
                };
                let subpath = &module_name[potential_target.len()..];
                return self.load_module_from_target_export_or_import(
                    extensions,
                    module_name,
                    scope,
                    is_imports,
                    packagejson::ExportsOrImports::from_json_value(target.clone()),
                    subpath,
                    false,
                    &potential_target,
                );
            }
        }

        None
    }

    #[allow(clippy::too_many_arguments)]
    fn load_module_from_target_export_or_import(
        &mut self,
        extensions: Extensions,
        module_name: &str,
        scope: &packagejson::InfoCacheEntry,
        is_imports: bool,
        target: packagejson::ExportsOrImports,
        subpath: &str,
        is_pattern: bool,
        key: &str,
    ) -> Option<Resolved> {
        match target.json_value.type_ {
            packagejson::JsonValueType::String => {
                let target_string = target.json_value.as_string().to_string();
                if !is_pattern && !subpath.is_empty() && !target_string.ends_with('/') {
                    self.trace_invalid_package_target(scope, module_name);
                    return None;
                }
                if !target_string.starts_with("./") {
                    if is_imports
                        && !target_string.starts_with("../")
                        && !target_string.starts_with('/')
                        && !tspath::is_rooted_disk_path(&target_string)
                    {
                        let combined_lookup = if is_pattern {
                            target_string.replace('*', subpath)
                        } else {
                            format!("{target_string}{subpath}")
                        };
                        let scope_containing_directory =
                            tspath::ensure_trailing_directory_separator(&scope.package_directory);
                        self.trace(
                            &diagnostics::Using_0_subpath_1_with_target_2,
                            vec![
                                "imports".to_string(),
                                key.to_string(),
                                combined_lookup.clone(),
                            ],
                        );
                        self.trace(
                            &diagnostics::Resolving_module_0_from_1,
                            vec![combined_lookup.clone(), scope_containing_directory.clone()],
                        );
                        let old_name = std::mem::replace(&mut self.name, combined_lookup);
                        let old_containing_directory = std::mem::replace(
                            &mut self.containing_directory,
                            scope_containing_directory,
                        );
                        let result = self.resolve_node_like();
                        self.name = old_name;
                        self.containing_directory = old_containing_directory;
                        return result.is_resolved().then(|| Resolved {
                            path: result.resolved_file_name,
                            extension: result.extension,
                            package_id: result.package_id,
                            original_path: result.original_path,
                            resolved_using_ts_extension: result.resolved_using_ts_extension,
                        });
                    }
                    self.trace_invalid_package_target(scope, module_name);
                    return None;
                }

                let parts = if tspath::path_is_relative(&target_string) {
                    tspath::get_path_components(&target_string, "")[1..].to_vec()
                } else {
                    tspath::get_path_components(&target_string, "")
                };
                if parts
                    .iter()
                    .skip(1)
                    .any(|part| matches!(part.as_str(), ".." | "." | "node_modules"))
                {
                    self.trace_invalid_package_target(scope, module_name);
                    return None;
                }
                if tspath::get_path_components(subpath, "")
                    .iter()
                    .any(|part| matches!(part.as_str(), ".." | "." | "node_modules"))
                {
                    self.trace_invalid_package_target(scope, module_name);
                    return None;
                }

                let resolved_target =
                    tspath::combine_paths(&scope.package_directory, &[&target_string]);
                let message_target = if is_pattern {
                    target_string.replace('*', subpath)
                } else {
                    format!("{target_string}{subpath}")
                };
                self.trace(
                    &diagnostics::Using_0_subpath_1_with_target_2,
                    vec![
                        if is_imports { "imports" } else { "exports" }.to_string(),
                        key.to_string(),
                        message_target,
                    ],
                );
                let final_path = if is_pattern {
                    tspath::get_normalized_absolute_path(
                        &resolved_target.replace('*', subpath),
                        &self.resolver.host.get_current_directory(),
                    )
                } else {
                    tspath::get_normalized_absolute_path(
                        &format!("{resolved_target}{subpath}"),
                        &self.resolver.host.get_current_directory(),
                    )
                };
                if let Some(mut input_link) = self.try_load_input_file_for_path(
                    &final_path,
                    subpath,
                    &tspath::combine_paths(&scope.package_directory, &["package.json"]),
                    is_imports,
                ) {
                    if input_link.is_resolved() {
                        input_link.package_id =
                            self.get_package_id_from_info(&input_link.path, scope);
                    }
                    return Some(input_link);
                }
                let mut result = self.load_file_name_from_package_json_field(
                    extensions,
                    &final_path,
                    &target_string,
                )?;
                result.package_id = self.get_package_id_from_info(&result.path, scope);
                Some(result)
            }
            packagejson::JsonValueType::Object => {
                self.trace(&diagnostics::Entering_conditional_exports, Vec::new());
                for (condition, sub_target) in target.as_object() {
                    if self.condition_matches(condition) {
                        self.trace(
                            &diagnostics::Matched_0_condition_1,
                            vec![
                                if is_imports { "imports" } else { "exports" }.to_string(),
                                condition.clone(),
                            ],
                        );
                        if let Some(result) = self.load_module_from_target_export_or_import(
                            extensions,
                            module_name,
                            scope,
                            is_imports,
                            packagejson::ExportsOrImports::from_json_value(sub_target.clone()),
                            subpath,
                            is_pattern,
                            key,
                        ) {
                            if result.is_resolved() {
                                self.trace(
                                    &diagnostics::Resolved_under_condition_0,
                                    vec![condition.clone()],
                                );
                            }
                            self.trace(&diagnostics::Exiting_conditional_exports, Vec::new());
                            return Some(result);
                        }
                        self.trace(
                            &diagnostics::Failed_to_resolve_under_condition_0,
                            vec![condition.clone()],
                        );
                    } else {
                        self.trace(
                            &diagnostics::Saw_non_matching_condition_0,
                            vec![condition.clone()],
                        );
                    }
                }
                self.trace(&diagnostics::Exiting_conditional_exports, Vec::new());
                None
            }
            packagejson::JsonValueType::Array => {
                if target.as_array().is_empty() {
                    self.trace_invalid_package_target(scope, module_name);
                    return None;
                }
                for element in target.as_array() {
                    if let Some(result) = self.load_module_from_target_export_or_import(
                        extensions,
                        module_name,
                        scope,
                        is_imports,
                        packagejson::ExportsOrImports::from_json_value(element.clone()),
                        subpath,
                        is_pattern,
                        key,
                    ) {
                        return Some(result);
                    }
                }
                None
            }
            packagejson::JsonValueType::Null => {
                self.trace(
                    &diagnostics::X_package_json_scope_0_explicitly_maps_specifier_1_to_null,
                    vec![scope.package_directory.clone(), module_name.to_string()],
                );
                Some(unresolved())
            }
            _ => {
                self.trace_invalid_package_target(scope, module_name);
                None
            }
        }
    }

    fn try_load_input_file_for_path(
        &mut self,
        final_path: &str,
        entry: &str,
        package_path: &str,
        is_imports: bool,
    ) -> Option<Resolved> {
        // Replace any references to outputs for files in the program with the input files
        // to support package self-names used with outDir.
        if !self.is_config_lookup
            && (!self.compiler_options.declaration_dir.is_empty()
                || !self.compiler_options.out_dir.is_empty())
            && !final_path.contains("/node_modules/")
            && (self.compiler_options.config_file_path.is_empty()
                || tspath::contains_path(
                    &tspath::get_directory_path(package_path),
                    &self.compiler_options.config_file_path,
                    &tspath::ComparePathsOptions {
                        use_case_sensitive_file_names: self
                            .resolver
                            .host
                            .use_case_sensitive_file_names(),
                        current_directory: self.resolver.host.get_current_directory(),
                    },
                ))
        {
            // Note: this differs from Strada's tryLoadInputFileForPath in that it
            // does not attempt to perform "guesses", instead requring a clear root indicator.
            let root_dir = if !self.compiler_options.root_dir.is_empty() {
                // A `rootDir` compiler option strongly indicates the root location
                self.compiler_options.root_dir.clone()
            } else if !self.compiler_options.config_file_path.is_empty() {
                // When no explicit rootDir is set, treat the config file's directory as the project root, which establishes the common source directory, so no other locations need to be checked.
                tspath::get_directory_path(&self.compiler_options.config_file_path)
            } else {
                let message = if is_imports {
                    &diagnostics::The_project_root_is_ambiguous_but_is_required_to_resolve_import_map_entry_0_in_file_1_Supply_the_rootDir_compiler_option_to_disambiguate
                } else {
                    &diagnostics::The_project_root_is_ambiguous_but_is_required_to_resolve_export_map_entry_0_in_file_1_Supply_the_rootDir_compiler_option_to_disambiguate
                };
                self.diagnostics.push(ast::new_diagnostic(
                    None,
                    core::TextRange::default(),
                    message,
                    &[
                        if entry.is_empty() { "." } else { entry }
                            .to_string()
                            .into(),
                        package_path.to_string().into(),
                    ],
                ));
                return Some(unresolved());
            };

            let candidate_directories = self.get_output_directories_for_base_directory(&root_dir);
            for candidate_dir in candidate_directories {
                if tspath::contains_path(
                    &candidate_dir,
                    final_path,
                    &tspath::ComparePathsOptions {
                        use_case_sensitive_file_names: self
                            .resolver
                            .host
                            .use_case_sensitive_file_names(),
                        current_directory: self.resolver.host.get_current_directory(),
                    },
                ) {
                    // The matched export is looking up something in either the out declaration or js dir, now map the written path back into the source dir and source extension
                    let path_fragment = &final_path[candidate_dir.len() + 1..];
                    let possible_input_base = tspath::combine_paths(&root_dir, &[path_fragment]);
                    let js_and_dts_extensions = [
                        tspath::EXTENSION_MJS,
                        tspath::EXTENSION_CJS,
                        tspath::EXTENSION_JS,
                        tspath::EXTENSION_JSON,
                        tspath::EXTENSION_DMTS,
                        tspath::EXTENSION_DCTS,
                        tspath::EXTENSION_DTS,
                    ];
                    for ext in js_and_dts_extensions {
                        if tspath::file_extension_is(&possible_input_base, ext) {
                            for possible_ext in
                                tspath::get_possible_original_input_extension_for_extension(
                                    &possible_input_base,
                                )
                            {
                                if !extension_is_ok(self.extensions, &possible_ext) {
                                    continue;
                                }
                                let possible_input_with_input_extension =
                                    tspath::change_extension(&possible_input_base, &possible_ext);
                                if self
                                    .resolver
                                    .host
                                    .file_exists(&possible_input_with_input_extension)
                                    && let Some(resolved) = self
                                        .load_file_name_from_package_json_field(
                                            self.extensions,
                                            &possible_input_with_input_extension,
                                            "",
                                        )
                                {
                                    return Some(resolved);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn get_output_directories_for_base_directory(
        &self,
        common_source_dir_guess: &str,
    ) -> Vec<String> {
        let current_dir = if !self.compiler_options.config_file_path.is_empty() {
            self.resolver.host.get_current_directory()
        } else {
            common_source_dir_guess.to_string()
        };
        let mut candidate_directories = Vec::new();
        if !self.compiler_options.declaration_dir.is_empty() {
            candidate_directories.push(tspath::get_normalized_absolute_path(
                &tspath::combine_paths(&current_dir, &[&self.compiler_options.declaration_dir]),
                &self.resolver.host.get_current_directory(),
            ));
        }
        if !self.compiler_options.out_dir.is_empty()
            && self.compiler_options.out_dir != self.compiler_options.declaration_dir
        {
            candidate_directories.push(tspath::get_normalized_absolute_path(
                &tspath::combine_paths(&current_dir, &[&self.compiler_options.out_dir]),
                &self.resolver.host.get_current_directory(),
            ));
        }
        candidate_directories
    }

    fn condition_matches(&self, condition: &str) -> bool {
        if condition == "default" || self.conditions.iter().any(|current| current == condition) {
            return true;
        }
        self.conditions.iter().any(|current| current == "types")
            && crate::is_applicable_versioned_types_key(condition)
    }

    fn trace_invalid_package_target(
        &mut self,
        scope: &packagejson::InfoCacheEntry,
        module_name: &str,
    ) {
        self.trace(
            &diagnostics::X_package_json_scope_0_has_invalid_type_for_target_of_specifier_1,
            vec![scope.package_directory.clone(), module_name.to_string()],
        );
    }

    fn trace_type_reference_directive_result(
        &mut self,
        type_reference_directive_name: &str,
        result: &ResolvedTypeReferenceDirective,
    ) {
        if !result.is_resolved() {
            self.trace(
                &diagnostics::Type_reference_directive_0_was_not_resolved,
                vec![type_reference_directive_name.to_string()],
            );
            return;
        }

        if !result.package_id.name.is_empty() {
            self.trace(
                &diagnostics::Type_reference_directive_0_was_successfully_resolved_to_1_with_Package_ID_2_primary_Colon_3,
                vec![
                    type_reference_directive_name.to_string(),
                    result.resolved_file_name.clone(),
                    result.package_id.to_string(),
                    result.primary.to_string(),
                ],
            );
        } else {
            self.trace(
                &diagnostics::Type_reference_directive_0_was_successfully_resolved_to_1_primary_Colon_2,
                vec![
                    type_reference_directive_name.to_string(),
                    result.resolved_file_name.clone(),
                    result.primary.to_string(),
                ],
            );
        }
    }

    fn get_version_paths(
        &mut self,
        package_info: &packagejson::InfoCacheEntry,
    ) -> Option<packagejson::VersionPaths> {
        let contents = package_info.get_contents()?;
        let mut traces = Vec::new();
        let version_paths = contents.get_version_paths(Some(
            |message: &'static diagnostics::Message, args: &[String]| {
                traces.push((message, args.to_vec()));
            },
        ));
        for (message, args) in traces {
            self.trace(message, args);
        }
        version_paths.exists().then_some(version_paths)
    }

    fn node_load_module_by_relative_name(
        &mut self,
        extensions: Extensions,
        candidate: &str,
        consider_package_json: bool,
    ) -> Option<Resolved> {
        self.trace(
            &diagnostics::Loading_module_as_file_Slash_folder_candidate_module_location_0_target_file_types_Colon_1,
            vec![candidate.to_string(), crate::extensions_to_string(extensions)],
        );
        if !tspath::has_trailing_directory_separator(candidate) {
            let parent_of_candidate = tspath::get_directory_path(candidate);
            if !self.directory_exists_or_root(&parent_of_candidate) {
                self.trace(
                    &diagnostics::Directory_0_does_not_exist_skipping_all_lookups_in_it,
                    vec![parent_of_candidate],
                );
                return None;
            }
            if let Some(mut resolved_from_file) = self.load_module_from_file(extensions, candidate)
            {
                if consider_package_json {
                    let package_directory =
                        crate::parse_node_module_from_path(&resolved_from_file.path, false);
                    if !package_directory.is_empty() {
                        resolved_from_file.package_id =
                            self.get_package_id(&resolved_from_file.path, &package_directory);
                    }
                }
                return Some(resolved_from_file);
            }
        }
        if !self.directory_exists_or_root(candidate) {
            self.trace(
                &diagnostics::Directory_0_does_not_exist_skipping_all_lookups_in_it,
                vec![candidate.to_string()],
            );
            return None;
        }
        if !self.esm_mode {
            return self.load_node_module_from_directory(
                extensions,
                candidate,
                consider_package_json,
            );
        }
        None
    }

    fn load_module_from_file(
        &mut self,
        extensions: Extensions,
        candidate: &str,
    ) -> Option<Resolved> {
        if let Some(resolved) =
            self.load_module_from_file_no_implicit_extensions(extensions, candidate)
        {
            return Some(resolved);
        }
        if !self.esm_mode {
            return self.try_adding_extensions(candidate, extensions, "");
        }
        None
    }

    fn load_module_from_file_no_implicit_extensions(
        &mut self,
        extensions: Extensions,
        candidate: &str,
    ) -> Option<Resolved> {
        let base = tspath::get_base_file_name(candidate);
        if !base.contains('.') {
            return None;
        }
        let mut extensionless = tspath::remove_file_extension(candidate);
        if extensionless == candidate
            && let Some(index) = candidate.rfind('.')
        {
            extensionless = candidate[..index].to_string();
        }
        let extension = &candidate[extensionless.len()..];
        self.trace(
            &diagnostics::File_name_0_has_a_1_extension_stripping_it,
            vec![candidate.to_string(), extension.to_string()],
        );
        self.try_adding_extensions(&extensionless, extensions, extension)
    }

    fn try_adding_extensions(
        &mut self,
        extensionless: &str,
        extensions: Extensions,
        original_extension: &str,
    ) -> Option<Resolved> {
        let directory = tspath::get_directory_path(extensionless);
        if !directory.is_empty() && !self.directory_exists_or_root(&directory) {
            return None;
        }
        match original_extension {
            tspath::EXTENSION_MJS | tspath::EXTENSION_MTS | tspath::EXTENSION_DMTS => self
                .try_extension_group(
                    extensionless,
                    original_extension,
                    &[
                        (
                            EXTENSIONS_TYPESCRIPT,
                            tspath::EXTENSION_MTS,
                            original_extension == tspath::EXTENSION_MTS
                                || original_extension == tspath::EXTENSION_DMTS,
                        ),
                        (
                            EXTENSIONS_DECLARATION,
                            tspath::EXTENSION_DMTS,
                            original_extension == tspath::EXTENSION_MTS
                                || original_extension == tspath::EXTENSION_DMTS,
                        ),
                        (EXTENSIONS_JAVASCRIPT, tspath::EXTENSION_MJS, false),
                    ],
                    extensions,
                ),
            tspath::EXTENSION_CJS | tspath::EXTENSION_CTS | tspath::EXTENSION_DCTS => self
                .try_extension_group(
                    extensionless,
                    original_extension,
                    &[
                        (
                            EXTENSIONS_TYPESCRIPT,
                            tspath::EXTENSION_CTS,
                            original_extension == tspath::EXTENSION_CTS
                                || original_extension == tspath::EXTENSION_DCTS,
                        ),
                        (
                            EXTENSIONS_DECLARATION,
                            tspath::EXTENSION_DCTS,
                            original_extension == tspath::EXTENSION_CTS
                                || original_extension == tspath::EXTENSION_DCTS,
                        ),
                        (EXTENSIONS_JAVASCRIPT, tspath::EXTENSION_CJS, false),
                    ],
                    extensions,
                ),
            tspath::EXTENSION_JSON => {
                if extensions & EXTENSIONS_DECLARATION != 0
                    && let Some(resolved) = self.try_extension(".d.json.ts", extensionless, false)
                {
                    return Some(resolved);
                }
                if extensions & EXTENSIONS_JSON != 0 {
                    return self.try_extension(tspath::EXTENSION_JSON, extensionless, false);
                }
                None
            }
            tspath::EXTENSION_TSX | tspath::EXTENSION_JSX => self.try_extension_group(
                extensionless,
                original_extension,
                &[
                    (
                        EXTENSIONS_TYPESCRIPT,
                        tspath::EXTENSION_TSX,
                        original_extension == tspath::EXTENSION_TSX,
                    ),
                    (
                        EXTENSIONS_TYPESCRIPT,
                        tspath::EXTENSION_TS,
                        original_extension == tspath::EXTENSION_TSX,
                    ),
                    (
                        EXTENSIONS_DECLARATION,
                        tspath::EXTENSION_DTS,
                        original_extension == tspath::EXTENSION_TSX,
                    ),
                    (EXTENSIONS_JAVASCRIPT, tspath::EXTENSION_JSX, false),
                    (EXTENSIONS_JAVASCRIPT, tspath::EXTENSION_JS, false),
                ],
                extensions,
            ),
            tspath::EXTENSION_TS | tspath::EXTENSION_DTS | tspath::EXTENSION_JS | "" => {
                let resolved = self.try_extension_group(
                    extensionless,
                    original_extension,
                    &[
                        (
                            EXTENSIONS_TYPESCRIPT,
                            tspath::EXTENSION_TS,
                            original_extension == tspath::EXTENSION_TS
                                || original_extension == tspath::EXTENSION_DTS,
                        ),
                        (
                            EXTENSIONS_TYPESCRIPT,
                            tspath::EXTENSION_TSX,
                            original_extension == tspath::EXTENSION_TS
                                || original_extension == tspath::EXTENSION_DTS,
                        ),
                        (
                            EXTENSIONS_DECLARATION,
                            tspath::EXTENSION_DTS,
                            original_extension == tspath::EXTENSION_TS
                                || original_extension == tspath::EXTENSION_DTS,
                        ),
                        (EXTENSIONS_JAVASCRIPT, tspath::EXTENSION_JS, false),
                        (EXTENSIONS_JAVASCRIPT, tspath::EXTENSION_JSX, false),
                    ],
                    extensions,
                );
                if resolved.is_some() {
                    return resolved;
                }
                if self.is_config_lookup {
                    return self.try_extension(tspath::EXTENSION_JSON, extensionless, false);
                }
                None
            }
            _ => {
                if extensions & EXTENSIONS_DECLARATION != 0
                    && !tspath::is_declaration_file_name(&format!(
                        "{extensionless}{original_extension}"
                    ))
                {
                    return self.try_extension(
                        &format!(".d{original_extension}.ts"),
                        extensionless,
                        false,
                    );
                }
                None
            }
        }
    }

    fn try_extension_group(
        &mut self,
        extensionless: &str,
        _original_extension: &str,
        entries: &[(Extensions, &str, bool)],
        extensions: Extensions,
    ) -> Option<Resolved> {
        for (mask, extension, resolved_using_ts_extension) in entries {
            if extensions & *mask != 0
                && let Some(resolved) =
                    self.try_extension(extension, extensionless, *resolved_using_ts_extension)
            {
                return Some(resolved);
            }
        }
        None
    }

    fn try_extension(
        &mut self,
        extension: &str,
        extensionless: &str,
        resolved_using_ts_extension: bool,
    ) -> Option<Resolved> {
        let file_name = format!("{extensionless}{extension}");
        self.try_file(&file_name).map(|path| Resolved {
            path,
            extension: extension.to_string(),
            resolved_using_ts_extension: !self.candidate_ending_is_from_config
                && resolved_using_ts_extension,
            ..Default::default()
        })
    }

    fn try_file(&mut self, file_name: &str) -> Option<String> {
        if self.compiler_options.module_suffixes.is_empty() {
            let exists = self.resolver.host.file_exists(file_name);
            match (exists, self.mark_file_lookup_for_trace(file_name, exists)) {
                (true, _) => self.trace(
                    &diagnostics::File_0_exists_use_it_as_a_name_resolution_result,
                    vec![file_name.to_string()],
                ),
                (false, Some(_)) => self.trace(
                    &diagnostics::File_0_does_not_exist_according_to_earlier_cached_lookups,
                    vec![file_name.to_string()],
                ),
                (false, None) => self.trace(
                    &diagnostics::File_0_does_not_exist,
                    vec![file_name.to_string()],
                ),
            }
            if exists {
                return Some(file_name.to_string());
            }
            return None;
        }
        let ext = tspath::try_get_extension_from_path(file_name);
        let file_name_no_extension = tspath::remove_extension(file_name, ext);
        for suffix in self.compiler_options.module_suffixes.clone() {
            let path = format!("{file_name_no_extension}{suffix}{ext}");
            let exists = self.resolver.host.file_exists(&path);
            match (exists, self.mark_file_lookup_for_trace(&path, exists)) {
                (true, _) => self.trace(
                    &diagnostics::File_0_exists_use_it_as_a_name_resolution_result,
                    vec![path.clone()],
                ),
                (false, Some(_)) => self.trace(
                    &diagnostics::File_0_does_not_exist_according_to_earlier_cached_lookups,
                    vec![path.clone()],
                ),
                (false, None) => {
                    self.trace(&diagnostics::File_0_does_not_exist, vec![path.clone()])
                }
            }
            if exists {
                return Some(path);
            }
        }
        None
    }

    fn directory_exists_or_root(&self, path: &str) -> bool {
        tspath::is_disk_path_root(path) || self.resolver.host.directory_exists(path)
    }

    fn load_node_module_from_directory(
        &mut self,
        extensions: Extensions,
        candidate: &str,
        consider_package_json: bool,
    ) -> Option<Resolved> {
        let package_info = consider_package_json
            .then(|| self.get_package_json_info(candidate))
            .flatten();
        self.load_node_module_from_directory_worker(extensions, candidate, package_info.as_ref())
    }

    fn load_node_module_from_directory_worker(
        &mut self,
        extensions: Extensions,
        candidate: &str,
        package_info: Option<&packagejson::InfoCacheEntry>,
    ) -> Option<Resolved> {
        let mut package_file = String::new();
        let mut version_paths = None;
        if let Some(package_info) = package_info.filter(|info| info.exists()) {
            version_paths = self.get_version_paths(package_info);
            if tspath::compare_paths(
                candidate,
                &package_info.package_directory,
                &tspath::ComparePathsOptions {
                    use_case_sensitive_file_names: self
                        .resolver
                        .host
                        .use_case_sensitive_file_names(),
                    current_directory: self.resolver.host.get_current_directory(),
                },
            )
            .is_eq()
            {
                package_file = self
                    .get_package_file(extensions, package_info)
                    .unwrap_or_default();
            }
        }

        let index_path = if self.is_config_lookup {
            tspath::combine_paths(candidate, &["tsconfig"])
        } else {
            tspath::combine_paths(candidate, &["index"])
        };

        if let Some(version_paths) = &version_paths
            && let Some(paths) = version_paths.get_paths()
            && (package_file.is_empty()
                || tspath::contains_path(
                    candidate,
                    &package_file,
                    &tspath::ComparePathsOptions::default(),
                ))
        {
            let module_name = if package_file.is_empty() {
                tspath::get_relative_path_from_directory(
                    candidate,
                    &index_path,
                    &tspath::ComparePathsOptions::default(),
                )
            } else {
                tspath::get_relative_path_from_directory(
                    candidate,
                    &package_file,
                    &tspath::ComparePathsOptions::default(),
                )
            };
            self.trace(
                &diagnostics::X_package_json_has_a_typesVersions_entry_0_that_matches_compiler_version_1_looking_for_a_pattern_to_match_module_name_2,
                vec![
                    version_paths.version.clone(),
                    core::version().to_string(),
                    module_name.clone(),
                ],
            );
            let path_patterns = try_parse_patterns(paths);
            if let Some(result) = self.try_load_module_using_paths_with_loader(
                extensions,
                &module_name,
                candidate,
                paths,
                path_patterns.as_ref(),
                |state, extensions, candidate| {
                    state.load_package_json_candidate(
                        extensions,
                        candidate,
                        &package_file,
                        package_info,
                    )
                },
            ) {
                return Some(result);
            }
        }

        if !package_file.is_empty()
            && let Some(result) = self.load_package_json_candidate(
                extensions,
                &package_file,
                &package_file,
                package_info,
            )
        {
            return Some(result);
        }
        if !self.esm_mode {
            if !self.resolver.host.directory_exists(candidate) {
                return None;
            }
            return self.load_module_from_file(extensions, &index_path);
        }
        None
    }

    fn load_package_json_candidate(
        &mut self,
        extensions: Extensions,
        candidate: &str,
        package_json_value: &str,
        package_info: Option<&packagejson::InfoCacheEntry>,
    ) -> Option<Resolved> {
        if let Some(package_file_result) =
            self.load_file_name_from_package_json_field(extensions, candidate, package_json_value)
        {
            return Some(package_file_result);
        }

        let expanded_extensions = if extensions == EXTENSIONS_DECLARATION {
            EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION
        } else {
            extensions
        };
        let save_esm_mode = self.esm_mode;
        let save_candidate_ending_is_from_config = self.candidate_ending_is_from_config;
        self.candidate_ending_is_from_config = true;
        if package_info
            .and_then(packagejson::InfoCacheEntry::get_contents)
            .is_some_and(|contents| contents.fields.header_fields.type_.value != "module")
        {
            self.esm_mode = false;
        }
        let result = self.node_load_module_by_relative_name(expanded_extensions, candidate, false);
        self.esm_mode = save_esm_mode;
        self.candidate_ending_is_from_config = save_candidate_ending_is_from_config;
        result
    }

    fn load_file_name_from_package_json_field(
        &mut self,
        extensions: Extensions,
        candidate: &str,
        package_json_value: &str,
    ) -> Option<Resolved> {
        if (extensions & EXTENSIONS_TYPESCRIPT != 0
            && tspath::has_implementation_ts_file_extension(candidate))
            || (extensions & EXTENSIONS_DECLARATION != 0
                && tspath::is_declaration_file_name(candidate))
        {
            return self.try_file(candidate).map(|path| {
                let extension = tspath::try_extract_ts_extension(&path).to_string();
                Resolved {
                    path,
                    extension: extension.clone(),
                    resolved_using_ts_extension: package_json_value.ends_with('*')
                        && !extension.is_empty(),
                    ..Default::default()
                }
            });
        }
        if self.is_config_lookup
            && extensions & EXTENSIONS_JSON != 0
            && tspath::file_extension_is(candidate, tspath::EXTENSION_JSON)
        {
            return self.try_file(candidate).map(|path| Resolved {
                path,
                extension: tspath::EXTENSION_JSON.to_string(),
                ..Default::default()
            });
        }
        self.load_module_from_file_no_implicit_extensions(extensions, candidate)
    }

    fn get_package_file(
        &mut self,
        extensions: Extensions,
        package_info: &packagejson::InfoCacheEntry,
    ) -> Option<String> {
        let contents = package_info.get_contents()?;
        if self.is_config_lookup {
            return self.get_package_json_path_field(
                &contents.fields.path_fields.tsconfig,
                &package_info.package_directory,
                "tsconfig",
            );
        }
        if extensions & EXTENSIONS_DECLARATION != 0 {
            if let Some(package_file) = self.get_package_json_path_field(
                &contents.fields.path_fields.typings,
                &package_info.package_directory,
                "typings",
            ) {
                return Some(package_file);
            }
            if let Some(package_file) = self.get_package_json_path_field(
                &contents.fields.path_fields.types,
                &package_info.package_directory,
                "types",
            ) {
                return Some(package_file);
            }
        }
        if extensions & (crate::EXTENSIONS_IMPLEMENTATION_FILES | EXTENSIONS_DECLARATION) != 0 {
            return self.get_package_json_path_field(
                &contents.fields.path_fields.main,
                &package_info.package_directory,
                "main",
            );
        }
        None
    }

    fn get_package_json_path_field(
        &mut self,
        field: &packagejson::Expected<String>,
        directory: &str,
        field_name: &str,
    ) -> Option<String> {
        let (value, valid) = field.get_value();
        if !valid || value.is_empty() {
            self.trace(
                &diagnostics::X_package_json_does_not_have_a_0_field,
                vec![field_name.to_string()],
            );
            return None;
        }
        let path = tspath::normalize_path(&tspath::combine_paths(directory, &[&value]));
        self.trace(
            &diagnostics::X_package_json_has_0_field_1_that_references_2,
            vec![field_name.to_string(), value, path.clone()],
        );
        Some(path)
    }

    fn get_package_json_info(
        &mut self,
        package_directory: &str,
    ) -> Option<packagejson::InfoCacheEntry> {
        let package_json_path = tspath::combine_paths(package_directory, &["package.json"]);
        if let Some(existing) = self
            .resolver
            .caches
            .package_json_info_cache
            .get(&package_json_path)
        {
            if existing.contents.is_some() {
                self.trace(
                    &diagnostics::File_0_exists_according_to_earlier_cached_lookups,
                    vec![package_json_path.clone()],
                );
                if existing.package_directory == package_directory {
                    return Some(existing);
                }
                return Some(packagejson::InfoCacheEntry {
                    package_directory: package_directory.to_string(),
                    directory_exists: true,
                    contents: existing.contents,
                });
            } else {
                if existing.directory_exists {
                    self.trace(
                        &diagnostics::File_0_does_not_exist_according_to_earlier_cached_lookups,
                        vec![package_json_path],
                    );
                }
                return None;
            }
        }

        let directory_exists = self.resolver.host.directory_exists(package_directory);
        if directory_exists && self.resolver.host.file_exists(&package_json_path) {
            let (contents, _) = self.resolver.host.fs().read_file(&package_json_path);
            let (fields, parseable) = match packagejson::parse(contents.as_bytes()) {
                Ok(fields) => (fields, true),
                Err(_) => (Default::default(), false),
            };
            self.trace(
                &diagnostics::Found_package_json_at_0,
                vec![package_json_path.clone()],
            );
            let result = self.resolver.caches.package_json_info_cache.set(
                &package_json_path,
                packagejson::InfoCacheEntry {
                    package_directory: package_directory.to_string(),
                    directory_exists: true,
                    contents: Some(packagejson::PackageJson::new(fields, parseable)),
                },
            );
            return Some(result);
        }
        if directory_exists {
            self.trace(
                &diagnostics::File_0_does_not_exist,
                vec![package_json_path.clone()],
            );
            self.resolver.caches.package_json_info_cache.set(
                &package_json_path,
                packagejson::InfoCacheEntry {
                    package_directory: package_directory.to_string(),
                    directory_exists,
                    contents: None,
                },
            );
        }
        None
    }

    fn get_package_id(&mut self, resolved_file_name: &str, package_directory: &str) -> PackageId {
        let Some(package_info) = self.get_package_json_info(package_directory) else {
            return PackageId::default();
        };
        self.get_package_id_from_info(resolved_file_name, &package_info)
    }

    fn get_package_id_from_info(
        &mut self,
        resolved_file_name: &str,
        package_info: &packagejson::InfoCacheEntry,
    ) -> PackageId {
        let Some(contents) = package_info.get_contents() else {
            return PackageId::default();
        };
        let (name, name_ok) = contents.fields.header_fields.name.get_value();
        let (version, version_ok) = contents.fields.header_fields.version.get_value();
        if !name_ok || !version_ok {
            return PackageId::default();
        }
        let sub_module_name = if resolved_file_name.len() > package_info.package_directory.len() {
            resolved_file_name[package_info.package_directory.len() + 1..].to_string()
        } else {
            String::new()
        };
        PackageId {
            name,
            version,
            sub_module_name,
            peer_dependencies: self.read_package_json_peer_dependencies(package_info),
        }
    }

    fn read_package_json_peer_dependencies(
        &mut self,
        package_json_info: &packagejson::InfoCacheEntry,
    ) -> String {
        let Some(contents) = package_json_info.get_contents() else {
            return String::new();
        };
        let (peer_dependencies, ok) = contents
            .fields
            .dependency_fields
            .peer_dependencies
            .get_value();
        if !ok || peer_dependencies.is_empty() {
            self.trace(
                &diagnostics::X_package_json_does_not_have_a_0_field,
                vec!["peerDependencies".to_string()],
            );
            return String::new();
        }
        let package_directory = tspath::normalize_path(
            &self
                .resolver
                .host
                .realpath(&package_json_info.package_directory),
        );
        let Some(node_modules_index) = package_directory.rfind("/node_modules") else {
            return String::new();
        };
        let node_modules = format!(
            "{}/",
            &package_directory[..node_modules_index + "/node_modules".len()]
        );
        let mut names = peer_dependencies.keys().cloned().collect::<Vec<_>>();
        names.sort();
        let mut result = String::new();
        for name in names {
            if let Some(peer_package_json) =
                self.get_package_json_info(&format!("{node_modules}{name}"))
                && let Some(contents) = peer_package_json.get_contents()
            {
                result.push('+');
                result.push_str(&name);
                result.push('@');
                result.push_str(&contents.fields.header_fields.version.value);
            }
        }
        result
    }

    fn create_resolved_module_handling_symlink(
        &mut self,
        mut resolved: Option<Resolved>,
    ) -> ResolvedModule {
        let is_external_library_import = resolved
            .as_ref()
            .is_some_and(|resolved| resolved.path.contains("/node_modules/"));
        if self.compiler_options.preserve_symlinks != core::TS_TRUE
            && is_external_library_import
            && resolved
                .as_ref()
                .is_some_and(|resolved| resolved.original_path.is_empty())
            && !tspath::is_external_module_name_relative(&self.name)
            && let Some(resolved) = &mut resolved
        {
            let (original_path, resolved_file_name) =
                self.get_original_and_resolved_file_name(&resolved.path);
            if !original_path.is_empty() {
                resolved.path = resolved_file_name;
                resolved.original_path = original_path;
            }
        }
        self.create_resolved_module(resolved, is_external_library_import)
    }

    fn create_resolved_module(
        &mut self,
        resolved: Option<Resolved>,
        is_external_library_import: bool,
    ) -> ResolvedModule {
        if let Some(resolved) = resolved {
            ResolvedModule {
                resolution_diagnostics: self.diagnostics.clone(),
                resolved_file_name: resolved.path,
                original_path: resolved.original_path,
                extension: resolved.extension,
                resolved_using_ts_extension: resolved.resolved_using_ts_extension,
                package_id: resolved.package_id,
                is_external_library_import,
                alternate_result: String::new(),
            }
        } else {
            ResolvedModule {
                resolution_diagnostics: self.diagnostics.clone(),
                ..Default::default()
            }
        }
    }

    fn get_original_and_resolved_file_name(&mut self, file_name: &str) -> (String, String) {
        let resolved_file_name = tspath::normalize_path(&self.resolver.host.realpath(file_name));
        self.trace(
            &diagnostics::Resolving_real_path_for_0_result_1,
            vec![file_name.to_string(), resolved_file_name.clone()],
        );
        let compare_paths_options = tspath::ComparePathsOptions {
            use_case_sensitive_file_names: self.resolver.host.fs().use_case_sensitive_file_names(),
            current_directory: self.resolver.host.get_current_directory(),
        };
        if tspath::compare_paths(file_name, &resolved_file_name, &compare_paths_options).is_eq() {
            return (String::new(), file_name.to_string());
        }
        (file_name.to_string(), resolved_file_name)
    }

    fn create_resolved_type_reference_directive(
        &mut self,
        resolved: Option<Resolved>,
        _primary: bool,
    ) -> ResolvedTypeReferenceDirective {
        if let Some(resolved) = resolved.filter(Resolved::is_resolved) {
            if !tspath::extension_is_ts(&resolved.extension) {
                panic!("expected a TypeScript file extension");
            }
            let is_external_library_import = resolved.path.contains("/node_modules/");
            let mut resolved_type_reference_directive = ResolvedTypeReferenceDirective {
                resolution_diagnostics: self.diagnostics.clone(),
                primary: _primary,
                resolved_file_name: resolved.path,
                original_path: String::new(),
                package_id: resolved.package_id,
                is_external_library_import,
            };
            if self.compiler_options.preserve_symlinks != core::TS_TRUE {
                let (original_path, resolved_file_name) = self.get_original_and_resolved_file_name(
                    &resolved_type_reference_directive.resolved_file_name,
                );
                if !original_path.is_empty() {
                    resolved_type_reference_directive.resolved_file_name = resolved_file_name;
                    resolved_type_reference_directive.original_path = original_path;
                }
            }
            resolved_type_reference_directive
        } else {
            ResolvedTypeReferenceDirective::default()
        }
    }
}

pub fn get_compiler_options_with_redirect(
    compiler_options: &CompilerOptions,
    redirected_reference: Option<&dyn ResolvedProjectReference>,
) -> CompilerOptions {
    if let Some(redirected_reference) = redirected_reference {
        return redirected_reference.compiler_options();
    }
    compiler_options.clone()
}

pub struct Resolver<H = ResolutionHostBox> {
    pub caches: Caches,
    pub host: H,
    pub compiler_options: CompilerOptions,
    pub typings_location: String,
    pub project_name: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolverOptions {
    pub package_json_cache_present: bool,
}

impl<H: ResolutionHost> Resolver<H> {
    pub fn new(
        host: H,
        options: CompilerOptions,
        typings_location: String,
        project_name: String,
    ) -> Self {
        let caches = new_caches(
            &host.get_current_directory(),
            host.use_case_sensitive_file_names(),
            &options,
        );
        Self {
            host,
            caches,
            compiler_options: options,
            typings_location,
            project_name,
        }
    }

    pub fn new_with_options(
        host: H,
        compiler_options: CompilerOptions,
        typings_location: String,
        project_name: String,
        _opts: ResolverOptions,
    ) -> Self {
        Self::new(host, compiler_options, typings_location, project_name)
    }

    pub fn new_trace_builder(&self) -> Option<Tracer> {
        (self.compiler_options.trace_resolution == core::TS_TRUE).then(Tracer::default)
    }

    pub fn get_package_scope_for_path(&self, directory: &str) -> Option<String> {
        tspath::for_each_ancestor_directory_stopping_at_global_cache(
            &self.typings_location,
            directory.to_string(),
            |ancestor| {
                let package_json_path = tspath::combine_paths(ancestor, &["package.json"]);
                let found = self.host.file_exists(&package_json_path);
                (ancestor.to_string(), found)
            },
        )
    }

    pub fn resolve_type_reference_directive(
        &mut self,
        type_reference_directive_name: &str,
        containing_file: &str,
        resolution_mode: core::ResolutionMode,
        redirected_reference: Option<&dyn ResolvedProjectReference>,
        from_inferred_types_containing_file: bool,
    ) -> (ResolvedTypeReferenceDirective, Vec<DiagAndArgs>) {
        let redirect_config_name = crate::get_redirect_config_name(redirected_reference);
        let key = crate::TypeRefDirectiveResolutionCacheKey {
            containing_directory: directory_of(containing_file),
            type_reference_name: type_reference_directive_name.to_string(),
            resolution_mode,
            redirect_config_name,
            from_inferred_types_containing_file,
        };
        let trace_builder = self.new_trace_builder();
        if trace_builder.is_none()
            && let Some(cached) = self.caches.type_ref_directive_resolution_cache.get(&key)
        {
            return (cached.clone(), Vec::new());
        }
        let compiler_options =
            get_compiler_options_with_redirect(&self.compiler_options, redirected_reference);
        let (type_roots, from_config) =
            compiler_options.get_effective_type_roots(&self.host.get_current_directory());
        let containing_directory = directory_of(containing_file);
        let mut state = new_resolution_state(NewResolutionStateParams {
            name: type_reference_directive_name,
            containing_directory: &containing_directory,
            is_type_reference_directive: true,
            resolution_mode,
            compiler_options: &compiler_options,
            redirected_reference,
            resolver: self,
            trace_builder,
        });
        state.trace(
            &diagnostics::Resolving_type_reference_directive_0_containing_file_1_root_directory_2,
            vec![
                type_reference_directive_name.to_string(),
                containing_file.to_string(),
                type_roots.join(","),
            ],
        );
        let result = state.resolve_type_reference_directive(
            &type_roots,
            from_config,
            from_inferred_types_containing_file,
        );
        state.trace_type_reference_directive_result(type_reference_directive_name, &result);
        let traces = state
            .tracer
            .as_ref()
            .map(Tracer::get_traces)
            .unwrap_or_default();
        drop(state);
        self.caches
            .type_ref_directive_resolution_cache
            .set(key, result.clone());
        (result, traces)
    }

    pub fn resolve_module_name(
        &mut self,
        module_name: &str,
        containing_file: &str,
        resolution_mode: core::ResolutionMode,
        redirected_reference: Option<&dyn ResolvedProjectReference>,
    ) -> (ResolvedModule, Vec<DiagAndArgs>) {
        let redirect_config_name = crate::get_redirect_config_name(redirected_reference);
        let key = crate::ModuleResolutionCacheKey {
            containing_directory: directory_of(containing_file),
            module_name: module_name.to_string(),
            resolution_mode,
            redirect_config_name,
        };
        let compiler_options =
            get_compiler_options_with_redirect(&self.compiler_options, redirected_reference);
        let trace_builder = self.new_trace_builder();
        if trace_builder.is_none()
            && let Some(cached) = self.caches.module_resolution_cache.get(&key)
        {
            return (cached.clone(), Vec::new());
        }
        let mut trace_builder = trace_builder;
        if let Some(tracer) = &mut trace_builder {
            tracer.write_message(
                &diagnostics::Resolving_module_0_from_1,
                vec![module_name.to_string(), containing_file.to_string()],
            );
        }
        let module_resolution = compiler_options.get_module_resolution_kind();
        if let Some(tracer) = &mut trace_builder {
            if compiler_options.module_resolution != module_resolution {
                tracer.write_message(
                    &diagnostics::Module_resolution_kind_is_not_specified_using_0,
                    vec![module_resolution.string().to_string()],
                );
            } else {
                tracer.write_message(
                    &diagnostics::Explicitly_specified_module_resolution_kind_Colon_0,
                    vec![module_resolution.string().to_string()],
                );
            }
        }
        let containing_directory = directory_of(containing_file);
        let mut state = new_resolution_state(NewResolutionStateParams {
            name: module_name,
            containing_directory: &containing_directory,
            is_type_reference_directive: false,
            resolution_mode,
            compiler_options: &compiler_options,
            redirected_reference,
            resolver: self,
            trace_builder,
        });
        let result = match module_resolution {
            core::ModuleResolutionKind::Node16
            | core::ModuleResolutionKind::NodeNext
            | core::ModuleResolutionKind::Bundler => state.resolve_node_like(),
            module_resolution => panic!("Unexpected moduleResolution: {module_resolution:?}"),
        };
        if let Some(tracer) = &mut state.tracer {
            if result.is_resolved() {
                if !result.package_id.name.is_empty() {
                    tracer.write_message(
                        &diagnostics::Module_name_0_was_successfully_resolved_to_1_with_Package_ID_2,
                        vec![
                            module_name.to_string(),
                            result.resolved_file_name.clone(),
                            result.package_id.to_string(),
                        ],
                    );
                } else {
                    tracer.write_message(
                        &diagnostics::Module_name_0_was_successfully_resolved_to_1,
                        vec![module_name.to_string(), result.resolved_file_name.clone()],
                    );
                }
            } else {
                tracer.write_message(
                    &diagnostics::Module_name_0_was_not_resolved,
                    vec![module_name.to_string()],
                );
            }
        }
        let traces = state
            .tracer
            .as_ref()
            .map(Tracer::get_traces)
            .unwrap_or_default();
        drop(state);
        self.caches.module_resolution_cache.set(key, result.clone());
        (result, traces)
    }

    pub fn resolve_package_directory(
        &mut self,
        module_name: &str,
        containing_file: &str,
        resolution_mode: core::ResolutionMode,
        redirected_reference: Option<&dyn ResolvedProjectReference>,
    ) -> Option<ResolvedModule> {
        let compiler_options =
            get_compiler_options_with_redirect(&self.compiler_options, redirected_reference);
        let containing_directory = directory_of(containing_file);
        let mut state = new_resolution_state(NewResolutionStateParams {
            name: module_name,
            containing_directory: &containing_directory,
            is_type_reference_directive: false,
            resolution_mode,
            compiler_options: &compiler_options,
            redirected_reference,
            resolver: self,
            trace_builder: None,
        });
        state.resolve_package_directory_only = true;
        state
            .load_module_from_nearest_node_modules_directory(false)
            .filter(Resolved::is_resolved)
            .map(|resolved| state.create_resolved_module_handling_symlink(Some(resolved)))
    }

    pub fn get_parsed_patterns_for_paths(&mut self) -> ParsedPatterns {
        parse_pattern_keys(self.compiler_options.paths.keys().cloned()).unwrap_or_default()
    }

    pub fn get_entrypoints_from_package_json_info(
        &self,
        package_json: &packagejson::InfoCacheEntry,
        package_name: &str,
        enable_directory_search: bool,
    ) -> Vec<ResolvedEntrypoint> {
        if !package_json.directory_exists {
            return Vec::new();
        }

        let mut state = ResolutionState::default_for(self);
        state.extensions = EXTENSIONS_TYPESCRIPT | EXTENSIONS_DECLARATION;
        state.features = NODE_RESOLUTION_FEATURES_ALL;
        state.compiler_options = self.compiler_options.clone();
        if package_json.exists()
            && let Some(contents) = package_json.get_contents()
        {
            let exports = contents.fields.path_fields.exports.clone();
            if exports.is_present() {
                return state.load_entrypoints_from_export_map(package_json, package_name, exports);
            }
        }

        let mut result = Vec::new();
        if let Some(main_resolution) = self.load_node_module_from_directory_worker(package_json) {
            result.push(self.create_resolved_entrypoint_handling_symlink(
                &main_resolution,
                package_name,
                None,
                None,
                Ending::Fixed,
            ));
        }

        if enable_directory_search {
            let extensions = entrypoint_extensions();
            let other_files = vfs::vfsmatch::read_directory(
                self.host.fs(),
                &self.host.get_current_directory(),
                &package_json.package_directory,
                &extensions,
                &["node_modules".to_string()],
                &["**/*".to_string()],
                vfs::vfsmatch::UNLIMITED_DEPTH,
            );
            let compare_paths_options = tspath::ComparePathsOptions {
                use_case_sensitive_file_names: self.host.fs().use_case_sensitive_file_names(),
                current_directory: String::new(),
            };
            for file in other_files {
                if result.iter().any(|entrypoint| {
                    tspath::compare_paths(
                        &file,
                        &entrypoint.resolved_file_name,
                        &compare_paths_options,
                    )
                    .is_eq()
                }) {
                    continue;
                }
                let module_specifier = tspath::resolve_path(
                    package_name,
                    &[&tspath::get_relative_path_from_directory(
                        &package_json.package_directory,
                        &file,
                        &compare_paths_options,
                    )],
                );
                result.push(self.create_resolved_entrypoint_handling_symlink(
                    &file,
                    &module_specifier,
                    None,
                    None,
                    Ending::Changeable,
                ));
            }
        }

        result
    }

    fn load_node_module_from_directory_worker(
        &self,
        package_json: &packagejson::InfoCacheEntry,
    ) -> Option<String> {
        let contents = package_json.get_contents();
        let candidates = contents
            .into_iter()
            .flat_map(|contents| {
                [
                    contents.fields.path_fields.types.get_value(),
                    contents.fields.path_fields.typings.get_value(),
                    contents.fields.path_fields.main.get_value(),
                ]
                .into_iter()
                .filter_map(|(value, valid)| valid.then_some(value.clone()))
            })
            .collect::<Vec<_>>();

        for candidate in candidates {
            let candidate = tspath::resolve_path(&package_json.package_directory, &[&candidate]);
            if let Some(file) = self.try_entrypoint_file(&candidate) {
                return Some(file);
            }
        }

        self.try_entrypoint_file(&tspath::combine_paths(
            &package_json.package_directory,
            &["index"],
        ))
    }

    fn try_entrypoint_file(&self, candidate: &str) -> Option<String> {
        let extensions = entrypoint_extensions();
        if self.host.file_exists(candidate) {
            return Some(candidate.to_string());
        }
        for extension in extensions {
            let file = if candidate.ends_with(&extension) {
                candidate.to_string()
            } else {
                format!("{candidate}{extension}")
            };
            if self.host.file_exists(&file) {
                return Some(file);
            }
        }
        None
    }

    fn create_resolved_entrypoint_handling_symlink(
        &self,
        file_name: &str,
        module_specifier: &str,
        include_conditions: Option<HashSet<String>>,
        exclude_conditions: Option<HashSet<String>>,
        ending: Ending,
    ) -> ResolvedEntrypoint {
        let realpath = self.host.fs().realpath(file_name);
        let (original_file_name, resolved_file_name) = if realpath != file_name {
            (file_name.to_string(), realpath)
        } else {
            (String::new(), file_name.to_string())
        };
        ResolvedEntrypoint {
            original_file_name,
            resolved_file_name,
            module_specifier: module_specifier.to_string(),
            ending,
            include_conditions: include_conditions.unwrap_or_default(),
            exclude_conditions: exclude_conditions.unwrap_or_default(),
            ..Default::default()
        }
    }
}

impl<'a, H: ResolutionHost> ResolutionState<'a, H> {
    fn load_entrypoints_from_export_map(
        &mut self,
        package_json: &packagejson::InfoCacheEntry,
        package_name: &str,
        mut exports: packagejson::ExportsOrImports,
    ) -> Vec<ResolvedEntrypoint> {
        let mut entrypoints = Vec::new();

        match exports.json_value.type_ {
            packagejson::JsonValueType::Array => {
                let elements = exports.as_array().to_vec();
                for element in elements {
                    self.load_entrypoints_from_target_exports(
                        &mut entrypoints,
                        package_json,
                        package_name,
                        ".",
                        None,
                        None,
                        packagejson::ExportsOrImports::from_json_value(element),
                    );
                }
            }
            packagejson::JsonValueType::Object => {
                if exports.is_subpaths() {
                    let entries = exports
                        .as_object()
                        .iter()
                        .map(|(subpath, export)| (subpath.clone(), export.clone()))
                        .collect::<Vec<_>>();
                    for (subpath, export) in entries {
                        self.load_entrypoints_from_target_exports(
                            &mut entrypoints,
                            package_json,
                            package_name,
                            &subpath,
                            None,
                            None,
                            packagejson::ExportsOrImports::from_json_value(export),
                        );
                    }
                } else {
                    self.load_entrypoints_from_target_exports(
                        &mut entrypoints,
                        package_json,
                        package_name,
                        ".",
                        None,
                        None,
                        exports,
                    );
                }
            }
            _ => {
                self.load_entrypoints_from_target_exports(
                    &mut entrypoints,
                    package_json,
                    package_name,
                    ".",
                    None,
                    None,
                    exports,
                );
            }
        }

        entrypoints
    }

    #[allow(clippy::too_many_arguments)]
    fn load_entrypoints_from_target_exports(
        &mut self,
        entrypoints: &mut Vec<ResolvedEntrypoint>,
        package_json: &packagejson::InfoCacheEntry,
        package_name: &str,
        subpath: &str,
        include_conditions: Option<HashSet<String>>,
        exclude_conditions: Option<HashSet<String>>,
        exports: packagejson::ExportsOrImports,
    ) {
        match exports.json_value.type_ {
            packagejson::JsonValueType::String => {
                let target = exports.json_value.as_string().to_string();
                if !target.starts_with("./") {
                    return;
                }
                if target.contains('*') {
                    if target.matches('*').count() != 1 {
                        return;
                    }
                    let pattern_path =
                        tspath::resolve_path(&package_json.package_directory, &[&target]);
                    let Some((leading_slice, trailing_slice)) = pattern_path.split_once('*') else {
                        return;
                    };
                    let case_sensitive = self.resolver.host.fs().use_case_sensitive_file_names();
                    let extensions = crate::extensions_array(self.extensions)
                        .into_iter()
                        .map(str::to_string)
                        .collect::<Vec<_>>();
                    let include = vec![tspath::change_full_extension(
                        &target.replacen('*', "**/*", 1),
                        ".*",
                    )];
                    let files = vfs::vfsmatch::read_directory(
                        self.resolver.host.fs(),
                        &self.resolver.host.get_current_directory(),
                        &package_json.package_directory,
                        &extensions,
                        &[],
                        &include,
                        vfs::vfsmatch::UNLIMITED_DEPTH,
                    );
                    for file in files {
                        let Some(matched_star) = self.get_matched_star_for_pattern_entrypoint(
                            &file,
                            leading_slice,
                            trailing_slice,
                            case_sensitive,
                        ) else {
                            continue;
                        };
                        let specifier_subpath = subpath.replacen('*', &matched_star, 1);
                        let module_specifier =
                            tspath::resolve_path(package_name, &[&specifier_subpath]);
                        entrypoints.push(
                            self.resolver.create_resolved_entrypoint_handling_symlink(
                                &file,
                                &module_specifier,
                                include_conditions.clone(),
                                exclude_conditions.clone(),
                                if target.ends_with('*') {
                                    Ending::ExtensionChangeable
                                } else {
                                    Ending::Fixed
                                },
                            ),
                        );
                    }
                } else {
                    let parts_after_first = tspath::get_path_components(&target, "")
                        .into_iter()
                        .skip(2)
                        .collect::<Vec<_>>();
                    if parts_after_first
                        .iter()
                        .any(|part| matches!(part.as_str(), ".." | "." | "node_modules"))
                    {
                        return;
                    }
                    let resolved_target =
                        tspath::resolve_path(&package_json.package_directory, &[&target]);
                    if let Some(result) = self.load_file_name_from_package_json_field(
                        self.extensions,
                        &resolved_target,
                        &target,
                    ) {
                        entrypoints.push(
                            self.resolver.create_resolved_entrypoint_handling_symlink(
                                &result.path,
                                &tspath::resolve_path(package_name, &[subpath]),
                                include_conditions,
                                exclude_conditions,
                                if target.ends_with('*') {
                                    Ending::ExtensionChangeable
                                } else {
                                    Ending::Fixed
                                },
                            ),
                        );
                    }
                }
            }
            packagejson::JsonValueType::Array => {
                let elements = exports.as_array().to_vec();
                for element in elements {
                    self.load_entrypoints_from_target_exports(
                        entrypoints,
                        package_json,
                        package_name,
                        subpath,
                        include_conditions.clone(),
                        exclude_conditions.clone(),
                        packagejson::ExportsOrImports::from_json_value(element),
                    );
                }
            }
            packagejson::JsonValueType::Object => {
                let mut prev_conditions: Vec<String> = Vec::new();
                let mut current_exclude_conditions = exclude_conditions;
                let entries = exports
                    .as_object()
                    .iter()
                    .map(|(condition, export)| (condition.clone(), export.clone()))
                    .collect::<Vec<_>>();
                for (condition, export) in entries {
                    if current_exclude_conditions
                        .as_ref()
                        .is_some_and(|conditions| conditions.contains(&condition))
                    {
                        continue;
                    }

                    let condition_always_matches = condition == "default"
                        || condition == "types"
                        || crate::is_applicable_versioned_types_key(&condition);
                    let mut new_include_conditions = include_conditions.clone();
                    if !condition_always_matches {
                        let include = new_include_conditions.get_or_insert_with(HashSet::new);
                        include.insert(condition.clone());
                        current_exclude_conditions = current_exclude_conditions.clone();
                        for prev_condition in &prev_conditions {
                            current_exclude_conditions
                                .get_or_insert_with(HashSet::new)
                                .insert(prev_condition.clone());
                        }
                    }

                    prev_conditions.push(condition.clone());
                    self.load_entrypoints_from_target_exports(
                        entrypoints,
                        package_json,
                        package_name,
                        subpath,
                        new_include_conditions,
                        current_exclude_conditions.clone(),
                        packagejson::ExportsOrImports::from_json_value(export),
                    );
                    if condition_always_matches {
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    fn get_matched_star_for_pattern_entrypoint(
        &self,
        file: &str,
        leading_slice: &str,
        trailing_slice: &str,
        case_sensitive: bool,
    ) -> Option<String> {
        if stringutil::has_prefix_and_suffix_without_overlap(
            file,
            leading_slice,
            trailing_slice,
            case_sensitive,
        ) {
            return slice_matched_star(file, leading_slice, trailing_slice);
        }

        let js_extension = crate::try_get_js_extension_for_file(
            file,
            self.compiler_options.jsx == core::JsxEmit::Preserve,
        );
        if !js_extension.is_empty() {
            let swapped = tspath::change_full_extension(file, &js_extension);
            if stringutil::has_prefix_and_suffix_without_overlap(
                &swapped,
                leading_slice,
                trailing_slice,
                case_sensitive,
            ) {
                return slice_matched_star(&swapped, leading_slice, trailing_slice);
            }
        }

        None
    }
}

fn slice_matched_star(file: &str, leading_slice: &str, trailing_slice: &str) -> Option<String> {
    let start = leading_slice.len();
    let end = file.len().checked_sub(trailing_slice.len())?;
    file.get(start..end).map(str::to_string)
}

pub fn entrypoint_extensions() -> Vec<String> {
    vec![
        tspath::EXTENSION_DTS.to_string(),
        tspath::EXTENSION_DMTS.to_string(),
        tspath::EXTENSION_DCTS.to_string(),
        tspath::EXTENSION_TS.to_string(),
        tspath::EXTENSION_TSX.to_string(),
        tspath::EXTENSION_MTS.to_string(),
        tspath::EXTENSION_CTS.to_string(),
    ]
}

pub fn new_resolver<H>(
    host: H,
    options: CompilerOptions,
    typings_location: impl Into<String>,
    project_name: impl Into<String>,
) -> Resolver<ResolutionHostBox>
where
    H: ResolutionHost + 'static,
{
    Resolver::new(
        Box::new(host) as ResolutionHostBox,
        options,
        typings_location.into(),
        project_name.into(),
    )
}

pub fn new_resolver_with_options<H>(
    host: H,
    compiler_options: CompilerOptions,
    typings_location: impl Into<String>,
    project_name: impl Into<String>,
    opts: ResolverOptions,
) -> Resolver<ResolutionHostBox>
where
    H: ResolutionHost + 'static,
{
    Resolver::new_with_options(
        Box::new(host) as ResolutionHostBox,
        compiler_options,
        typings_location.into(),
        project_name.into(),
        opts,
    )
}

struct DefaultResolutionHost {
    fs: vfs::osvfs::os::OsFs,
}

impl Default for DefaultResolutionHost {
    fn default() -> Self {
        Self {
            fs: vfs::osvfs::os::fs(),
        }
    }
}

impl ResolutionHost for DefaultResolutionHost {
    fn get_current_directory(&self) -> String {
        std::env::current_dir()
            .ok()
            .and_then(|path| path.to_str().map(str::to_string))
            .unwrap_or_default()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        &self.fs
    }
}

impl Default for Resolver<ResolutionHostBox> {
    fn default() -> Self {
        new_resolver(
            DefaultResolutionHost::default(),
            core::empty_compiler_options(),
            String::new(),
            String::new(),
        )
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParsedPatterns {
    pub matchable_string_set: HashSet<String>,
    pub patterns: Vec<core::Pattern>,
}

pub fn try_parse_patterns(
    path_mappings: &OrderedMap<String, Vec<String>>,
) -> Option<ParsedPatterns> {
    parse_pattern_keys(path_mappings.keys().cloned())
}

fn parse_pattern_keys(paths: impl IntoIterator<Item = String>) -> Option<ParsedPatterns> {
    let mut matchable_string_set = HashSet::new();
    let mut patterns = Vec::new();
    for path in paths {
        let pattern = core::try_parse_pattern(&path);
        if !pattern.is_valid() {
            continue;
        }
        if pattern.star_index == -1 {
            matchable_string_set.insert(path);
        } else {
            patterns.push(pattern);
        }
    }
    (!matchable_string_set.is_empty() || !patterns.is_empty()).then_some(ParsedPatterns {
        matchable_string_set,
        patterns,
    })
}

pub fn match_pattern_or_exact(patterns: &ParsedPatterns, candidate: &str) -> Option<String> {
    if patterns.matchable_string_set.contains(candidate) {
        return Some(candidate.to_string());
    }
    let pattern = core::find_best_pattern_match(&patterns.patterns, Clone::clone, candidate);
    pattern.is_valid().then_some(pattern.text)
}

pub fn normalize_path_for_cjs_resolution(containing_directory: &str, module_name: &str) -> String {
    let combined = tspath::combine_paths(containing_directory, &[module_name]);
    let parts = tspath::get_path_components(&combined, "");
    let last_part = parts.last().map(String::as_str).unwrap_or_default();
    if last_part == "." || last_part == ".." {
        return tspath::ensure_trailing_directory_separator(&tspath::normalize_path(&combined));
    }
    tspath::normalize_path(&combined)
}

pub fn matches_pattern_with_trailer(target: &str, name: &str) -> bool {
    if target.ends_with('*') {
        return false;
    }
    if let Some((prefix, suffix)) = target.split_once('*') {
        name.starts_with(prefix) && name.ends_with(suffix)
    } else {
        false
    }
}

pub fn extension_is_ok(extensions: Extensions, extension: &str) -> bool {
    (extensions & EXTENSIONS_JAVASCRIPT != 0
        && [
            tspath::EXTENSION_JS,
            tspath::EXTENSION_JSX,
            tspath::EXTENSION_MJS,
            tspath::EXTENSION_CJS,
        ]
        .contains(&extension))
        || (extensions & EXTENSIONS_TYPESCRIPT != 0
            && [
                tspath::EXTENSION_TS,
                tspath::EXTENSION_TSX,
                tspath::EXTENSION_MTS,
                tspath::EXTENSION_CTS,
            ]
            .contains(&extension))
        || (extensions & EXTENSIONS_DECLARATION != 0
            && [
                tspath::EXTENSION_DTS,
                tspath::EXTENSION_DMTS,
                tspath::EXTENSION_DCTS,
            ]
            .contains(&extension))
        || (extensions & EXTENSIONS_JSON != 0 && extension == tspath::EXTENSION_JSON)
}

fn compare_pattern_keys(a: &str, b: &str) -> std::cmp::Ordering {
    let a_pattern_index = a.find('*');
    let b_pattern_index = b.find('*');
    let base_len_a = a_pattern_index.map_or(a.len(), |index| index + 1);
    let base_len_b = b_pattern_index.map_or(b.len(), |index| index + 1);

    base_len_b
        .cmp(&base_len_a)
        .then_with(|| match (a_pattern_index, b_pattern_index) {
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Equal,
        })
        .then_with(|| b.len().cmp(&a.len()))
}

pub fn resolve_config<H: ResolutionHost>(
    module_name: &str,
    containing_file: &str,
    host: &H,
) -> Option<ResolvedModule> {
    let candidate = normalize_path_for_cjs_resolution(&directory_of(containing_file), module_name);
    let candidates = if tspath::try_get_extension_from_path(&candidate).is_empty() {
        vec![candidate.clone(), format!("{candidate}.json")]
    } else {
        vec![candidate]
    };
    candidates.into_iter().find_map(|candidate| {
        host.file_exists(&candidate).then_some(ResolvedModule {
            resolved_file_name: candidate,
            original_path: module_name.to_string(),
            extension: tspath::EXTENSION_JSON.to_string(),
            ..Default::default()
        })
    })
}

pub fn get_automatic_type_directive_names<H: ResolutionHost>(
    options: &CompilerOptions,
    host: &H,
) -> Vec<String> {
    if !options.uses_wildcard_types() {
        return options.types.clone();
    }

    let (type_roots, _) = options.get_effective_type_roots(&host.get_current_directory());
    let mut wildcard_matches = Vec::new();
    for root in type_roots {
        if !host.directory_exists(&root) {
            continue;
        }
        for type_directive_path in host.fs().get_accessible_entries(&root).directories {
            let normalized = tspath::normalize_path(&type_directive_path);
            let package_json_path = tspath::combine_paths(&root, &[&normalized, "package.json"]);
            let mut is_not_needed_package = false;
            if host.file_exists(&package_json_path) {
                let (contents, _) = host.fs().read_file(&package_json_path);
                if let Ok(package_json_content) = packagejson::parse(contents.as_bytes()) {
                    is_not_needed_package = package_json_content.path_fields.typings.null;
                }
            }
            if !is_not_needed_package {
                let base_file_name = tspath::get_base_file_name(&normalized);
                if !base_file_name.starts_with('.') {
                    wildcard_matches.push(base_file_name);
                }
            }
        }
    }

    let mut result = Vec::new();
    for t in &options.types {
        if t == "*" {
            result.extend(wildcard_matches.iter().cloned());
        } else {
            result.push(t.clone());
        }
    }
    let mut seen = HashSet::new();
    result
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Ending {
    // EndingFixed indicates that the module specifier cannot be changed without changing its resolution.
    #[default]
    Fixed,
    // EndingExtensionChangeable indicates that the module specifier's extension portion was inferred from a
    // file on disk, so an interchangeable one could be used instead (e.g. replacing .d.ts with .js).
    ExtensionChangeable,
    // EndingChangeable indicates that the module specifier's file name and extension portion were inferred
    // from a file on disk without being matched as part of an 'exports' pattern, so can be changed according
    // to the importer's module resolution rules (e.g. an /index.d.ts may be dropped entirely in CommonJS settings).
    Changeable,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedEntrypoint {
    // OriginalFileName is the symlink path if the entrypoint was discovered at a symlink. Empty otherwise.
    pub original_file_name: String,
    // ResolvedFileName is the real path to the entrypoint file.
    pub resolved_file_name: String,
    // ModuleSpecifier is the package-relative specifier exposed by the package.json entrypoint.
    pub module_specifier: String,
    // Ending indicates whether the file name and extension portion of ModuleSpecifier is fixed or can be changed.
    pub ending: Ending,
    pub include_conditions: HashSet<String>,
    pub exclude_conditions: HashSet<String>,
    // PORT NOTE: compatibility fields retained until auto-import registry callers
    // switch to the Go-shaped fields above.
    pub file_name: String,
    pub package_name: String,
    pub package_id: PackageId,
    pub is_from_exports: bool,
    pub is_from_imports: bool,
    pub symlink: String,
    pub realpath: String,
}

impl ResolvedEntrypoint {
    pub fn symlink_or_realpath(&self) -> String {
        if !self.original_file_name.is_empty() {
            self.original_file_name.clone()
        } else if !self.resolved_file_name.is_empty() {
            self.resolved_file_name.clone()
        } else if self.symlink.is_empty() {
            self.realpath.clone()
        } else {
            self.symlink.clone()
        }
    }
}

pub fn get_conditions(
    options: &CompilerOptions,
    mut resolution_mode: core::ResolutionMode,
) -> Vec<String> {
    let module_resolution = options.get_module_resolution_kind();
    if resolution_mode == core::ModuleKind::None
        && module_resolution == core::ModuleResolutionKind::Bundler
    {
        resolution_mode = core::ModuleKind::ESNext;
    }
    let mut conditions = Vec::with_capacity(3 + options.custom_conditions.len());
    if resolution_mode == core::ModuleKind::ESNext {
        conditions.push("import".to_string());
    } else {
        conditions.push("require".to_string());
    }
    if options.no_dts_resolution != core::TS_TRUE {
        conditions.push("types".to_string());
    }
    if module_resolution != core::ModuleResolutionKind::Bundler {
        conditions.push("node".to_string());
    }
    conditions.extend(options.custom_conditions.clone());
    conditions
}

pub fn get_node_resolution_features(options: &CompilerOptions) -> NodeResolutionFeatures {
    let mut features = match options.get_module_resolution_kind() {
        core::ModuleResolutionKind::Node16 => NODE_RESOLUTION_FEATURES_NODE16_DEFAULT,
        core::ModuleResolutionKind::NodeNext => NODE_RESOLUTION_FEATURES_NODE_NEXT_DEFAULT,
        core::ModuleResolutionKind::Bundler => crate::NODE_RESOLUTION_FEATURES_BUNDLER_DEFAULT,
        _ => NODE_RESOLUTION_FEATURES_NONE,
    };
    if options.resolve_package_json_exports == core::TS_TRUE {
        features |= crate::NODE_RESOLUTION_FEATURES_EXPORTS;
    } else if options.resolve_package_json_exports == core::TS_FALSE {
        features &= !crate::NODE_RESOLUTION_FEATURES_EXPORTS;
    }
    if options.resolve_package_json_imports == core::TS_TRUE {
        features |= crate::NODE_RESOLUTION_FEATURES_IMPORTS;
    } else if options.resolve_package_json_imports == core::TS_FALSE {
        features &= !crate::NODE_RESOLUTION_FEATURES_IMPORTS;
    }
    features
}

pub fn move_to_next_directory_separator_if_available(
    path: &str,
    prev_separator_index: usize,
    is_folder: bool,
) -> usize {
    let start = prev_separator_index + 1;
    path[start..]
        .find('/')
        .map(|index| start + index)
        .unwrap_or(if is_folder {
            path.len()
        } else {
            prev_separator_index
        })
}

fn directory_of(path: &str) -> String {
    tspath::get_directory_path(path)
}
