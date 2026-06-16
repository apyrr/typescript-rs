use ts_ast as ast;
use ts_checker as checker;
use ts_core as core;
use ts_module as module;
use ts_modulespecifiers as modulespecifiers;
use ts_outputpaths as outputpaths;
use ts_packagejson as packagejson;
use ts_printer as printer;
use ts_symlinks as symlinks;
use ts_transformers::declarations;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::Program;
use crate::emitter::SourceFileMayBeEmittedHost;

// NOTE: EmitHost operations must be thread-safe
pub trait EmitHost:
    printer::EmitHost
    + declarations::DeclarationEmitHost
    + SourceFileMayBeEmittedHost
    + outputpaths::OutputPathsHost
{
    fn get_mode_for_usage_location(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> core::ResolutionMode;
    fn get_resolved_module_from_module_specifier(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> Option<module::ResolvedModule>;
    fn get_default_resolution_mode_for_file(
        &self,
        file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode;
    fn file_exists(&self, path: &str) -> bool;
    fn get_global_typings_cache_location(&self) -> String;
    fn get_nearest_ancestor_directory_with_package_json(&self, dirname: &str) -> String;
    fn get_package_json_info(&self, pkg_json_path: &str) -> Option<packagejson::InfoCacheEntry>;
    fn get_source_of_project_reference_if_output_included(
        &self,
        file: &dyn ast::HasFileName,
    ) -> String;
    fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference>;
    fn get_redirect_targets(&self, path: tspath::Path) -> Vec<String>;
    fn get_effective_declaration_flags(
        &mut self,
        node: ast::Node,
        flags: ast::ModifierFlags,
    ) -> ast::ModifierFlags;
    fn get_output_paths_for(
        &self,
        file: &ast::SourceFile,
        force_dts_paths: bool,
    ) -> outputpaths::OutputPaths;
    fn get_resolution_mode_override(&mut self, node: ast::Node) -> core::ResolutionMode;
    fn get_source_file_from_reference(
        &self,
        origin: &ast::SourceFile,
        r#ref: &ast::FileReference,
    ) -> Option<ast::SourceFile>;
    fn get_symlink_cache(&self) -> symlinks::KnownSymlinks;
    fn resolve_module_name(
        &self,
        module_name: &str,
        containing_file: &str,
        resolution_mode: core::ResolutionMode,
    ) -> Option<module::ResolvedModule>;
}

// NOTE: emitHost operations must be thread-safe
pub struct EmitHostImpl<'program, 'checker, 'state> {
    program: &'program Program,
    checker: &'checker mut checker::Checker<'program, 'state>,
}

pub(crate) fn new_emit_host_with_checker<'program, 'checker, 'state>(
    program: &'program Program,
    checker: &'checker mut checker::Checker<'program, 'state>,
) -> EmitHostImpl<'program, 'checker, 'state> {
    EmitHostImpl { program, checker }
}

impl<'program, 'checker, 'state> EmitHost for EmitHostImpl<'program, 'checker, 'state> {
    fn get_mode_for_usage_location(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        self.program
            .get_mode_for_usage_location(file, module_specifier)
    }

    fn get_resolved_module_from_module_specifier(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> Option<module::ResolvedModule> {
        self.program
            .get_resolved_module_from_module_specifier(file, module_specifier)
    }

    fn get_default_resolution_mode_for_file(
        &self,
        file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        self.program.get_default_resolution_mode_for_file(file)
    }

    fn file_exists(&self, path: &str) -> bool {
        self.program.file_exists(path)
    }

    fn get_global_typings_cache_location(&self) -> String {
        self.program.get_global_typings_cache_location()
    }

    fn get_nearest_ancestor_directory_with_package_json(&self, dirname: &str) -> String {
        self.program
            .get_nearest_ancestor_directory_with_package_json(dirname)
    }

    fn get_package_json_info(&self, pkg_json_path: &str) -> Option<packagejson::InfoCacheEntry> {
        self.program.get_package_json_info(pkg_json_path)
    }

    fn get_source_of_project_reference_if_output_included(
        &self,
        file: &dyn ast::HasFileName,
    ) -> String {
        self.program
            .get_source_of_project_reference_if_output_included(file)
    }

    fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        self.program.get_project_reference_from_source(path)
    }

    fn get_redirect_targets(&self, path: tspath::Path) -> Vec<String> {
        self.program.get_redirect_targets(path)
    }

    fn get_effective_declaration_flags(
        &mut self,
        node: ast::Node,
        flags: ast::ModifierFlags,
    ) -> ast::ModifierFlags {
        printer::with_emit_resolver(self, |resolver| {
            resolver.get_effective_declaration_flags(node, flags)
        })
    }

    fn get_output_paths_for(
        &self,
        file: &ast::SourceFile,
        force_dts_paths: bool,
    ) -> outputpaths::OutputPaths {
        // TODO: cache
        let output_source_file = outputpaths_source_file(file);
        let output_options = outputpaths_compiler_options(self.options());
        outputpaths::get_output_paths_for(
            &output_source_file,
            &output_options,
            self,
            force_dts_paths,
        )
    }

    fn get_resolution_mode_override(&mut self, node: ast::Node) -> core::ResolutionMode {
        printer::with_emit_resolver(self, |resolver| resolver.get_resolution_mode_override(node))
    }

    fn get_source_file_from_reference(
        &self,
        origin: &ast::SourceFile,
        r#ref: &ast::FileReference,
    ) -> Option<ast::SourceFile> {
        self.program.get_source_file_from_reference(origin, r#ref)
    }

    fn get_symlink_cache(&self) -> symlinks::KnownSymlinks {
        self.program.get_symlink_cache()
    }

    fn resolve_module_name(
        &self,
        module_name: &str,
        containing_file: &str,
        resolution_mode: core::ResolutionMode,
    ) -> Option<module::ResolvedModule> {
        self.program
            .resolve_module_name(module_name, containing_file, resolution_mode)
    }
}

impl<'program, 'checker, 'state> printer::EmitHost for EmitHostImpl<'program, 'checker, 'state> {
    fn options(&self) -> Option<core::CompilerOptions> {
        Some(self.program.options().clone())
    }

    fn source_files(&self) -> Vec<ast::SourceFile> {
        self.program.source_files()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.program.opts.host.fs().use_case_sensitive_file_names()
    }

    fn get_current_directory(&self) -> String {
        self.program.get_current_directory()
    }

    fn common_source_directory(&self) -> String {
        crate::program::ProgramLike::common_source_directory(self.program)
    }

    fn is_emit_blocked(&self, file: &str) -> bool {
        self.program.is_emit_blocked(file)
    }

    fn write_file(&self, file_name: &str, text: &str) -> Result<(), String> {
        self.program
            .opts
            .host
            .fs()
            .write_file(file_name, text)
            .map_err(|err| err.to_string())
    }

    fn emit_binding_facts(
        &self,
        file: &ast::SourceFile,
    ) -> std::sync::Arc<dyn printer::EmitBindingFacts> {
        checker::Program::binding_state(self.program, file)
    }

    fn source_file_common_js_module_indicator(&self, file: &ast::SourceFile) -> Option<ast::Node> {
        checker::Program::binding_state(self.program, file).common_js_module_indicator()
    }

    fn source_file_external_module_indicator(&self, file: &ast::SourceFile) -> Option<ast::Node> {
        checker::Program::binding_state(self.program, file).external_module_indicator()
    }

    fn source_file_export_equals_declarations(&self, file: &ast::SourceFile) -> Vec<ast::Node> {
        let binding_state = checker::Program::binding_state(self.program, file);
        binding_state
            .source_symbol()
            .and_then(|symbol| {
                binding_state.with_symbol_exports(symbol, |exports| {
                    exports
                        .and_then(|exports| exports.get(ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS))
                        .copied()
                })
            })
            .map(|export_equals| {
                binding_state
                    .with_symbol_declarations(export_equals, |declarations| declarations.to_vec())
            })
            .unwrap_or_default()
    }

    fn source_file_nested_cjs_exports(&self, file: &ast::SourceFile) -> Vec<ast::Node> {
        checker::Program::binding_state(self.program, file)
            .nested_cjs_exports()
            .to_vec()
    }

    fn get_emit_module_format_of_file(&self, file: &dyn ast::HasFileName) -> core::ModuleKind {
        self.program.get_emit_module_format_of_file(file)
    }

    fn can_include_bind_and_check_diagnostics(&self, file: &ast::SourceFile) -> bool {
        self.program.can_include_bind_and_check_diagnostics(file)
    }

    fn with_emit_resolver(&mut self, f: &mut dyn FnMut(&mut dyn printer::EmitResolver)) {
        f(self.checker.get_emit_resolver())
    }

    fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        self.program.get_project_reference_from_source(path)
    }

    fn get_source_file_from_reference(
        &self,
        origin: &ast::SourceFile,
        r#ref: &ast::FileReference,
    ) -> Option<ast::SourceFile> {
        self.program.get_source_file_from_reference(origin, r#ref)
    }

    fn is_source_file_from_external_library(&self, file: &ast::SourceFile) -> bool {
        self.program
            .processed_files
            .source_files_found_searching_node_modules
            .has(&file.path())
    }
}

impl<'program, 'checker, 'state> outputpaths::OutputPathsHost
    for EmitHostImpl<'program, 'checker, 'state>
{
    fn common_source_directory(&self) -> String {
        crate::program::ProgramLike::common_source_directory(self.program)
    }

    fn get_current_directory(&self) -> String {
        self.program.get_current_directory()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.program.opts.host.fs().use_case_sensitive_file_names()
    }
}

impl<'program, 'checker, 'state> SourceFileMayBeEmittedHost
    for EmitHostImpl<'program, 'checker, 'state>
{
    fn options(&self) -> &core::CompilerOptions {
        self.program.options()
    }

    fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        self.program.get_project_reference_from_source(path)
    }

    fn is_source_file_from_external_library(&self, file: &ast::SourceFile) -> bool {
        self.program
            .processed_files
            .source_files_found_searching_node_modules
            .has(&file.path())
    }

    fn get_current_directory(&self) -> String {
        self.program.get_current_directory()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.program.opts.host.fs().use_case_sensitive_file_names()
    }

    fn source_files(&self) -> Vec<&ast::SourceFile> {
        self.program
            .source_files
            .iter()
            .map(crate::program::ProgramSourceFile::as_source_file)
            .collect()
    }
}

impl<'program, 'checker, 'state> modulespecifiers::ModuleSpecifierGenerationHost
    for EmitHostImpl<'program, 'checker, 'state>
{
    fn symlink_cache(&self) -> Option<symlinks::KnownSymlinks> {
        Some(self.program.get_symlink_cache())
    }

    fn common_source_directory(&self) -> String {
        crate::program::ProgramLike::common_source_directory(self.program)
    }

    fn global_typings_cache_location(&self) -> String {
        self.program.opts.typings_location.clone()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.program.opts.host.fs().use_case_sensitive_file_names()
    }

    fn current_directory(&self) -> String {
        self.program.get_current_directory()
    }

    fn project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        self.program.get_project_reference_from_source(path)
    }

    fn redirect_targets(&self, path: tspath::Path) -> Vec<String> {
        self.program.get_redirect_targets(path)
    }

    fn source_of_project_reference_if_output_included(
        &self,
        file: &dyn ast::HasFileName,
    ) -> String {
        self.program
            .get_source_of_project_reference_if_output_included(file)
    }

    fn file_exists(&self, path: &str) -> bool {
        self.program.file_exists(path)
    }

    fn nearest_ancestor_directory_with_package_json(&self, dirname: &str) -> String {
        self.program
            .get_nearest_ancestor_directory_with_package_json(dirname)
    }

    fn package_json_info(&self, pkg_json_path: &str) -> Option<packagejson::InfoCacheEntry> {
        self.program.get_package_json_info(pkg_json_path)
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

fn outputpaths_compiler_options(options: &core::CompilerOptions) -> outputpaths::CompilerOptions {
    options.clone()
}

fn outputpaths_source_file(source_file: &ast::SourceFile) -> outputpaths::SourceFile {
    source_file.share_readonly()
}
