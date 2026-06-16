use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ts_ast as ast;
use ts_binder as binder;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_module as module;
use ts_modulespecifiers as modulespecifiers;
use ts_packagejson as packagejson;
use ts_symlinks as symlinks;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs as vfs;

#[derive(Clone, Debug)]
pub struct PathAndFileName {
    pub path: tspath::Path,
    pub file_name: String,
}

pub trait RegistryCloneHost: module::ResolutionHost {
    fn fs(&self) -> vfs::FS;
    fn get_default_project(&self, path: tspath::Path)
    -> (tspath::Path, Option<&compiler::Program>);
    fn get_program_for_project(&self, project_path: tspath::Path) -> Option<&compiler::Program>;
    fn get_package_json(&self, file_name: &str) -> Option<packagejson::InfoCacheEntry>;
    fn get_source_file(&self, file_name: &str, path: tspath::Path) -> Option<ast::SourceFile>;
    fn dispose(&self);
}

struct AliasResolverBindingStates {
    root_states: Vec<Arc<binder::ProgramBindingState>>,
    root_by_root: HashMap<ast::Node, usize>,
    fetched: Mutex<HashMap<ast::Node, Arc<binder::ProgramBindingState>>>,
}

impl AliasResolverBindingStates {
    fn new(root_files: &[ast::SourceFile]) -> Self {
        let mut root_states = Vec::with_capacity(root_files.len());
        let mut root_by_root = HashMap::with_capacity(root_files.len());
        for file in root_files {
            let root = file.as_node();
            let state = binder::bind_source_file(file);
            let index = root_states.len();
            root_by_root.insert(root, index);
            root_states.push(state);
        }

        Self {
            root_states,
            root_by_root,
            fetched: Mutex::new(HashMap::new()),
        }
    }

    fn register_fetched_source_file(&self, file: ast::SourceFile) -> ast::SourceFile {
        let root = file.as_node();
        if self.root_by_root.contains_key(&root) {
            return file;
        }

        let mut fetched = self.fetched.lock().unwrap();
        fetched
            .entry(root)
            .or_insert_with(|| binder::bind_source_file(&file));
        file
    }

    fn get(&self, file: &ast::SourceFile) -> Option<Arc<binder::ProgramBindingState>> {
        let root = file.as_node();
        if let Some(index) = self.root_by_root.get(&root) {
            return self.root_states.get(*index).map(Arc::clone);
        }

        self.fetched.lock().unwrap().get(&root).map(Arc::clone)
    }
}

pub struct AliasResolver<'host> {
    pub to_path: Box<dyn Fn(&str) -> tspath::Path + Send + Sync>,
    pub host: &'host dyn RegistryCloneHost,
    pub module_resolver: Mutex<module::Resolver>,
    pub root_files: Vec<ast::SourceFile>,
    binding_states: AliasResolverBindingStates,
    // symlinks maps from realpath to symlinked path and file name
    pub symlinks: HashMap<tspath::Path, PathAndFileName>,
    pub on_failed_ambient_module_lookup: Box<dyn Fn(&dyn ast::HasFileName, &str) + Send + Sync>,
    pub resolved_modules: Mutex<
        HashMap<tspath::Path, HashMap<module::ModeAwareCacheKey, Option<module::ResolvedModule>>>,
    >,
    pub options: core::CompilerOptions,
}

pub fn new_alias_resolver(
    root_files: Vec<ast::SourceFile>,
    symlinks: HashMap<tspath::Path, PathAndFileName>,
    host: &dyn RegistryCloneHost,
    module_resolver: module::Resolver,
    to_path: impl Fn(&str) -> tspath::Path + Send + Sync + 'static,
    on_failed_ambient_module_lookup: impl Fn(&dyn ast::HasFileName, &str) + Send + Sync + 'static,
) -> AliasResolver<'_> {
    let binding_states = AliasResolverBindingStates::new(&root_files);
    AliasResolver {
        to_path: Box::new(to_path),
        host,
        module_resolver: Mutex::new(module_resolver),
        root_files,
        binding_states,
        symlinks,
        on_failed_ambient_module_lookup: Box::new(on_failed_ambient_module_lookup),
        resolved_modules: Mutex::new(HashMap::new()),
        options: core::CompilerOptions {
            no_check: core::TS_TRUE,
            ..Default::default()
        },
    }
}

impl AliasResolver<'_> {
    fn store_for_node(&self, node: ast::Node) -> &ast::AstStore {
        self.root_files
            .iter()
            .find(|file| file.store().store_id() == node.store_id())
            .map(|file| file.store())
            .expect("module specifier should belong to an alias resolver root file")
    }

    // BindSourceFiles implements checker.Program.
    pub fn bind_source_files(&self) {
        // We will bind as we parse
    }

    // SourceFiles implements checker.Program.
    pub fn source_files(&self) -> Vec<&ast::SourceFile> {
        self.root_files.iter().collect()
    }

    // Options implements checker.Program.
    pub fn options(&self) -> &core::CompilerOptions {
        &self.options
    }

    // GetCurrentDirectory implements checker.Program.
    pub fn get_current_directory(&self) -> String {
        self.host.get_current_directory()
    }

    // UseCaseSensitiveFileNames implements checker.Program.
    pub fn use_case_sensitive_file_names(&self) -> bool {
        RegistryCloneHost::fs(self.host).use_case_sensitive_file_names()
    }

    // GetSourceFile implements checker.Program.
    pub fn get_source_file(&self, file_name: &str) -> Option<ast::SourceFile> {
        let file = self
            .host
            .get_source_file(file_name, (self.to_path)(file_name))?;
        // file may be nil due to symlink/realpath mismatch; see TestAutoImportBuilderFS
        Some(self.binding_states.register_fetched_source_file(file))
    }

    fn binding_state(&self, source_file: &ast::SourceFile) -> Arc<binder::ProgramBindingState> {
        self.binding_states.get(source_file).unwrap_or_else(|| {
            panic!(
                "source file `{}` is not part of this alias resolver binding state",
                source_file.file_name()
            )
        })
    }

    // GetDefaultResolutionModeForFile implements checker.Program.
    pub fn get_default_resolution_mode_for_file(
        &self,
        _file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        core::ModuleKind::ESNext
    }

    // GetEmitModuleFormatOfFile implements checker.Program.
    pub fn get_emit_module_format_of_file(
        &self,
        _source_file: &dyn ast::HasFileName,
    ) -> core::ModuleKind {
        core::ModuleKind::ESNext
    }

    // GetEmitSyntaxForUsageLocation implements checker.Program.
    pub fn get_emit_syntax_for_usage_location(
        &self,
        _source_file: &dyn ast::HasFileName,
        _usage_location: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        core::ModuleKind::ESNext
    }

    // GetImpliedNodeFormatForEmit implements checker.Program.
    pub fn get_implied_node_format_for_emit(
        &self,
        _source_file: &dyn ast::HasFileName,
    ) -> core::ModuleKind {
        core::ModuleKind::ESNext
    }

    // GetModeForUsageLocation implements checker.Program.
    pub fn get_mode_for_usage_location(
        &self,
        _file: &dyn ast::HasFileName,
        _module_specifier: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        core::ModuleKind::ESNext
    }

    // GetResolvedModule implements checker.Program.
    pub fn get_resolved_module(
        &self,
        current_source_file: &dyn ast::HasFileName,
        module_reference: &str,
        mode: core::ResolutionMode,
    ) -> Option<module::ResolvedModule> {
        let key = module::ModeAwareCacheKey {
            name: module_reference.to_string(),
            mode,
        };

        {
            let mut resolved_modules = self.resolved_modules.lock().unwrap();
            let cache = resolved_modules
                .entry(current_source_file.path())
                .or_default();
            if let Some(resolved) = cache.get(&key).cloned() {
                return resolved;
            }
        }

        let (resolved, _) = self.module_resolver.lock().unwrap().resolve_module_name(
            module_reference,
            &current_source_file.file_name(),
            mode,
            None,
        );
        let resolved = Some(resolved);

        let resolved = {
            let mut resolved_modules = self.resolved_modules.lock().unwrap();
            let cache = resolved_modules
                .entry(current_source_file.path())
                .or_default();
            cache.entry(key).or_insert(resolved).clone()
        };

        if !resolved
            .as_ref()
            .is_some_and(|resolved| resolved.is_resolved())
            && !tspath::path_is_relative(module_reference)
        {
            (self.on_failed_ambient_module_lookup)(current_source_file, module_reference);
        }
        resolved
    }

    // GetSourceFileForResolvedModule implements checker.Program.
    pub fn get_source_file_for_resolved_module(&self, file_name: &str) -> Option<ast::SourceFile> {
        self.get_source_file(file_name)
    }

    // GetResolvedModules implements checker.Program.
    pub fn get_resolved_modules(
        &self,
    ) -> HashMap<tspath::Path, module::ModeAwareCache<module::ResolvedModule>> {
        // only used when producing diagnostics, which hopefully the checker won't do
        HashMap::new()
    }

    // ---

    // GetSymlinkCache implements checker.Program.
    pub fn get_symlink_cache(&self) -> symlinks::KnownSymlinks {
        let mut cache = symlinks::KnownSymlinks {
            cwd: self.get_current_directory(),
            use_case_sensitive_file_names: self.use_case_sensitive_file_names(),
            ..Default::default()
        };
        for (realpath, symlink) in &self.symlinks {
            cache.set_file(
                symlink.file_name.clone(),
                symlink.path.clone(),
                realpath.clone(),
            );
        }
        cache
    }

    // GetSourceFileMetaData implements checker.Program.
    pub fn get_source_file_meta_data(&self, path: tspath::Path) -> ast::SourceFileMetaData {
        let file_name = self
            .root_files
            .iter()
            .find(|source_file| source_file.path() == path)
            .map(|source_file| source_file.file_name())
            .unwrap_or_default();
        ast::SourceFileMetaData {
            package_json_type: String::new(),
            package_json_directory: tspath::get_directory_path(&file_name),
            implied_node_format: core::RESOLUTION_MODE_ESM,
        }
    }

    // CommonSourceDirectory implements checker.Program.
    pub fn common_source_directory(&self) -> String {
        self.get_current_directory()
    }

    // FileExists implements checker.Program.
    pub fn file_exists(&self, file_name: &str) -> bool {
        self.host.file_exists(file_name)
    }

    // GetGlobalTypingsCacheLocation implements checker.Program.
    pub fn get_global_typings_cache_location(&self) -> String {
        String::new()
    }

    // GetImportHelpersImportSpecifier implements checker.Program.
    pub fn get_import_helpers_import_specifier(&self, _path: tspath::Path) -> Option<ast::Node> {
        None
    }

    // GetJSXRuntimeImportSpecifier implements checker.Program.
    pub fn get_jsx_runtime_import_specifier(
        &self,
        _path: tspath::Path,
    ) -> (String, Option<ast::Node>) {
        (String::new(), None)
    }

    // GetNearestAncestorDirectoryWithPackageJson implements checker.Program.
    pub fn get_nearest_ancestor_directory_with_package_json(&self, dirname: &str) -> String {
        let mut current = dirname.to_string();
        loop {
            let package_json_path = tspath::combine_paths(&current, &["package.json"]);
            if self.host.get_package_json(&package_json_path).is_some() {
                return current;
            }
            let parent = tspath::get_directory_path(&current);
            if parent == current {
                return String::new();
            }
            current = parent;
        }
    }

    // GetPackageJsonInfo implements checker.Program.
    pub fn get_package_json_info(
        &self,
        pkg_json_path: &str,
    ) -> Option<packagejson::InfoCacheEntry> {
        self.host.get_package_json(pkg_json_path)
    }

    // GetProjectReferenceFromOutputDts implements checker.Program.
    pub fn get_project_reference_from_output_dts(
        &self,
        _path: tspath::Path,
    ) -> Option<&tsoptions::SourceOutputAndProjectReference> {
        None
    }

    // GetProjectReferenceFromSource implements checker.Program.
    pub fn get_project_reference_from_source(
        &self,
        _path: tspath::Path,
    ) -> Option<&tsoptions::SourceOutputAndProjectReference> {
        None
    }

    // GetRedirectForResolution implements checker.Program.
    pub fn get_redirect_for_resolution(
        &self,
        _file: &dyn ast::HasFileName,
    ) -> Option<&tsoptions::ParsedCommandLine> {
        None
    }

    // GetRedirectTargets implements checker.Program.
    pub fn get_redirect_targets(&self, _path: tspath::Path) -> Vec<String> {
        Vec::new()
    }

    // GetResolvedModuleFromModuleSpecifier implements checker.Program.
    pub fn get_resolved_module_from_module_specifier(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> Option<module::ResolvedModule> {
        let store = self.store_for_node(*module_specifier);
        self.get_resolved_module(
            file,
            &store.text(*module_specifier),
            core::RESOLUTION_MODE_ESM,
        )
    }

    // GetSourceOfProjectReferenceIfOutputIncluded implements checker.Program.
    pub fn get_source_of_project_reference_if_output_included(
        &self,
        file: &dyn ast::HasFileName,
    ) -> String {
        file.file_name()
    }

    // IsSourceFileDefaultLibrary implements checker.Program.
    pub fn is_source_file_default_library(&self, _path: tspath::Path) -> bool {
        false
    }

    // IsSourceFromProjectReference implements checker.Program.
    pub fn is_source_from_project_reference(&self, _path: tspath::Path) -> bool {
        false
    }

    // SourceFileMayBeEmitted implements checker.Program.
    pub fn source_file_may_be_emitted(
        &self,
        _source_file: &ast::SourceFile,
        _force_dts_emit: bool,
    ) -> bool {
        true
    }

    pub fn get_packages_map(&self) -> HashMap<String, bool> {
        HashMap::new()
    }
}

impl modulespecifiers::ModuleSpecifierGenerationHost for AliasResolver<'_> {
    fn symlink_cache(&self) -> Option<symlinks::KnownSymlinks> {
        Some(self.get_symlink_cache())
    }

    fn common_source_directory(&self) -> String {
        AliasResolver::common_source_directory(self)
    }

    fn global_typings_cache_location(&self) -> String {
        self.get_global_typings_cache_location()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        AliasResolver::use_case_sensitive_file_names(self)
    }

    fn current_directory(&self) -> String {
        self.get_current_directory()
    }

    fn project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        self.get_project_reference_from_source(path).cloned()
    }

    fn redirect_targets(&self, path: tspath::Path) -> Vec<String> {
        self.get_redirect_targets(path)
    }

    fn source_of_project_reference_if_output_included(
        &self,
        file: &dyn ast::HasFileName,
    ) -> String {
        self.get_source_of_project_reference_if_output_included(file)
    }

    fn file_exists(&self, path: &str) -> bool {
        AliasResolver::file_exists(self, path)
    }

    fn nearest_ancestor_directory_with_package_json(&self, dirname: &str) -> String {
        self.get_nearest_ancestor_directory_with_package_json(dirname)
    }

    fn package_json_info(&self, pkg_json_path: &str) -> Option<packagejson::InfoCacheEntry> {
        self.get_package_json_info(pkg_json_path)
    }

    fn default_resolution_mode_for_file(
        &self,
        file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        self.get_default_resolution_mode_for_file(file)
    }

    fn resolved_module_from_module_specifier(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> Option<module::ResolvedModule> {
        self.get_resolved_module_from_module_specifier(file, module_specifier)
    }

    fn mode_for_usage_location(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        self.get_mode_for_usage_location(file, module_specifier)
    }
}

impl checker::Host for AliasResolver<'_> {}

impl checker::Program for AliasResolver<'_> {
    fn options(&self) -> &core::CompilerOptions {
        self.options()
    }

    fn source_files(&self) -> Vec<&ast::SourceFile> {
        self.source_files()
    }

    fn bind_source_files(&self) {
        self.bind_source_files();
    }

    fn binding_state(&self, source_file: &ast::SourceFile) -> Arc<binder::ProgramBindingState> {
        self.binding_state(source_file)
    }

    fn file_exists(&self, file_name: &str) -> bool {
        self.file_exists(file_name)
    }

    fn get_source_file(&self, file_name: &str) -> Option<ast::SourceFile> {
        self.get_source_file(file_name)
    }

    fn get_source_file_for_resolved_module(&self, file_name: &str) -> Option<ast::SourceFile> {
        self.get_source_file_for_resolved_module(file_name)
    }

    fn get_emit_module_format_of_file(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ModuleKind {
        self.get_emit_module_format_of_file(source_file)
    }

    fn get_emit_syntax_for_usage_location(
        &self,
        source_file: &dyn ast::HasFileName,
        usage_location: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        self.get_emit_syntax_for_usage_location(source_file, usage_location)
    }

    fn get_mode_for_usage_location(
        &self,
        source_file: &dyn ast::HasFileName,
        usage_location: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        self.get_mode_for_usage_location(source_file, usage_location)
    }

    fn get_default_resolution_mode_for_file(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        self.get_default_resolution_mode_for_file(source_file)
    }

    fn get_implied_node_format_for_emit(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ModuleKind {
        self.get_implied_node_format_for_emit(source_file)
    }

    fn get_resolved_module(
        &self,
        current_source_file: &dyn ast::HasFileName,
        module_reference: &str,
        mode: core::ResolutionMode,
    ) -> Option<module::ResolvedModule> {
        self.get_resolved_module(current_source_file, module_reference, mode)
    }

    fn get_resolved_modules(
        &self,
    ) -> HashMap<tspath::Path, module::ModeAwareCache<module::ResolvedModule>> {
        self.get_resolved_modules()
    }

    fn get_packages_map(&self) -> HashMap<String, bool> {
        self.get_packages_map()
    }

    fn get_source_file_meta_data(&self, path: tspath::Path) -> ast::SourceFileMetaData {
        self.get_source_file_meta_data(path)
    }

    fn get_jsx_runtime_import_specifier(&self, path: tspath::Path) -> (String, Option<ast::Node>) {
        self.get_jsx_runtime_import_specifier(path)
    }

    fn get_import_helpers_import_specifier(&self, path: tspath::Path) -> Option<ast::Node> {
        self.get_import_helpers_import_specifier(path)
    }

    fn source_file_may_be_emitted(
        &self,
        source_file: &ast::SourceFile,
        force_dts_emit: bool,
    ) -> bool {
        self.source_file_may_be_emitted(source_file, force_dts_emit)
    }

    fn is_source_file_default_library(&self, path: tspath::Path) -> bool {
        self.is_source_file_default_library(path)
    }

    fn get_project_reference_from_output_dts(
        &self,
        path: tspath::Path,
    ) -> Option<&tsoptions::SourceOutputAndProjectReference> {
        self.get_project_reference_from_output_dts(path)
    }

    fn get_redirect_for_resolution(
        &self,
        file: &dyn ast::HasFileName,
    ) -> Option<&tsoptions::ParsedCommandLine> {
        self.get_redirect_for_resolution(file)
    }

    fn common_source_directory(&self) -> String {
        self.common_source_directory()
    }
}
