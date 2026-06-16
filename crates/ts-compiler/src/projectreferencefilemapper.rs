use std::collections::HashSet;

use ts_ast as ast;
use ts_core as core;
use ts_module as module;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use super::{ProjectReferenceFileMapper, SourceOutputAndProjectReference};

impl ProjectReferenceFileMapper {
    fn opts(&self) -> &crate::ProgramOptions {
        self.opts.as_ref().unwrap()
    }

    pub fn get_parse_file_redirect(&self, file: &(impl ast::HasFileName + ?Sized)) -> String {
        if self.opts().can_use_project_reference_source() {
            // Map to source file from project reference.
            let mut source = self.get_project_reference_from_output_dts(file.path());
            if source.is_none() {
                source = self.get_source_to_dts_if_symlink(file);
            }
            if let Some(source) = source {
                return source.source;
            }
        } else {
            // Map to dts file from project reference.
            let output = self.get_project_reference_from_source(file.path());
            if let Some(output) = output {
                if !output.output_dts.is_empty() {
                    return output.output_dts;
                }
            }
        }
        String::new()
    }

    pub fn get_resolved_project_references(
        &self,
    ) -> Option<Vec<Option<tsoptions::ParsedCommandLine>>> {
        let Some(config_file) = &self.opts().config.config_file else {
            return None;
        };
        let Some(refs) = self.references_in_config_file.get(&config_file.path) else {
            return None;
        };

        let mut result = Vec::with_capacity(refs.len());
        for ref_path in refs {
            result.push(
                self.config_to_project_reference
                    .get(ref_path)
                    .cloned()
                    .flatten(),
            );
        }
        Some(result)
    }

    pub fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<SourceOutputAndProjectReference> {
        self.source_to_project_reference.get(&path).cloned()
    }

    pub fn get_project_reference_from_output_dts(
        &self,
        path: tspath::Path,
    ) -> Option<SourceOutputAndProjectReference> {
        self.output_dts_to_project_reference.get(&path).cloned()
    }

    pub fn is_source_from_project_reference(&self, path: tspath::Path) -> bool {
        self.opts().can_use_project_reference_source()
            && self.get_project_reference_from_source(path).is_some()
    }

    pub fn get_compiler_options_for_file(
        &self,
        file: &(impl ast::HasFileName + ?Sized),
    ) -> core::CompilerOptions {
        let redirect = self.get_redirect_parsed_command_line_for_resolution(file);
        module::get_compiler_options_with_redirect(
            &self.opts().config.compiler_options(),
            redirect
                .as_ref()
                .map(|redirect| redirect as &dyn module::ResolvedProjectReference),
        )
    }

    pub fn get_redirect_parsed_command_line_for_resolution(
        &self,
        file: &(impl ast::HasFileName + ?Sized),
    ) -> Option<tsoptions::ParsedCommandLine> {
        self.get_redirect_for_resolution(file).0
    }

    pub fn get_redirect_for_resolution(
        &self,
        file: &(impl ast::HasFileName + ?Sized),
    ) -> (Option<tsoptions::ParsedCommandLine>, String) {
        let path = file.path();

        // Check if outputdts of source file from project reference.
        if let Some(output) = self.get_project_reference_from_source(path.clone()) {
            return (output.resolved.map(|resolved| *resolved), output.source);
        }

        // Source file from project reference.
        if let Some(result_from_dts) = self.get_project_reference_from_output_dts(path) {
            return (
                result_from_dts.resolved.map(|resolved| *resolved),
                result_from_dts.source,
            );
        }

        if let Some(realpath_dts_to_source) = self.get_source_to_dts_if_symlink(file) {
            return (
                realpath_dts_to_source.resolved.map(|resolved| *resolved),
                realpath_dts_to_source.source,
            );
        }
        (None, file.file_name())
    }

    pub fn get_resolved_reference_for(
        &self,
        path: tspath::Path,
    ) -> (Option<tsoptions::ParsedCommandLine>, bool) {
        let config = self
            .config_to_project_reference
            .get(&path)
            .cloned()
            .flatten();
        let ok = self.config_to_project_reference.contains_key(&path);
        (config, ok)
    }

    pub fn range_resolved_project_reference(
        &self,
        mut f: impl FnMut(
            tspath::Path,
            Option<tsoptions::ParsedCommandLine>,
            Option<tsoptions::ParsedCommandLine>,
            usize,
        ) -> bool,
    ) -> bool {
        let Some(config_file) = &self.opts().config.config_file else {
            return false;
        };
        let mut seen_ref = HashSet::with_capacity(self.references_in_config_file.len());
        seen_ref.insert(config_file.path.clone());
        let refs = self
            .references_in_config_file
            .get(&config_file.path)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        self.range_resolved_reference_worker(
            refs,
            &mut f,
            Some(self.opts().config.as_ref().clone()),
            &mut seen_ref,
        )
    }

    pub fn range_resolved_reference_worker(
        &self,
        references: &[tspath::Path],
        f: &mut impl FnMut(
            tspath::Path,
            Option<tsoptions::ParsedCommandLine>,
            Option<tsoptions::ParsedCommandLine>,
            usize,
        ) -> bool,
        parent: Option<tsoptions::ParsedCommandLine>,
        seen_ref: &mut HashSet<tspath::Path>,
    ) -> bool {
        for (index, path) in references.iter().enumerate() {
            if !seen_ref.insert(path.clone()) {
                continue;
            }
            let config = self
                .config_to_project_reference
                .get(path)
                .cloned()
                .flatten();
            if !f(path.clone(), config.clone(), parent.clone(), index) {
                return false;
            }
            if !self.range_resolved_reference_worker(
                self.references_in_config_file
                    .get(path)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                f,
                config,
                seen_ref,
            ) {
                return false;
            }
        }
        true
    }

    pub fn range_resolved_project_reference_in_child_config(
        &self,
        child_config: Option<&tsoptions::ParsedCommandLine>,
        mut f: impl FnMut(
            tspath::Path,
            Option<tsoptions::ParsedCommandLine>,
            Option<tsoptions::ParsedCommandLine>,
            usize,
        ) -> bool,
    ) -> bool {
        let Some(child_config) = child_config else {
            return false;
        };
        let Some(config_file) = &child_config.config_file else {
            return false;
        };
        let mut seen_ref = HashSet::with_capacity(self.references_in_config_file.len());
        seen_ref.insert(config_file.path.clone());
        let refs = self
            .references_in_config_file
            .get(&config_file.path)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        self.range_resolved_reference_worker(
            refs,
            &mut f,
            Some(self.opts().config.as_ref().clone()),
            &mut seen_ref,
        )
    }

    pub fn get_source_to_dts_if_symlink(
        &self,
        file: &(impl ast::HasFileName + ?Sized),
    ) -> Option<SourceOutputAndProjectReference> {
        let path = file.path();
        let (cached, ok) = self.realpath_dts_to_source.load(&path);
        if ok {
            return cached;
        }

        if let Some(host) = self.host.as_ref()
            && self
                .opts()
                .config
                .compiler_options()
                .preserve_symlinks
                .is_true()
        {
            let file_name = file.file_name();
            if !file_name.contains("/node_modules/") {
                self.realpath_dts_to_source.store(path, None);
            } else {
                let real_file_name = host.fs().realpath(&file_name);
                let real_declaration_path = tspath::to_path(
                    &real_file_name,
                    &self.opts().host.get_current_directory(),
                    self.opts().host.fs().use_case_sensitive_file_names(),
                );
                if real_declaration_path == path {
                    self.realpath_dts_to_source.store(path, None);
                } else if let Some(realpath_dts_to_source) =
                    self.get_project_reference_from_output_dts(real_declaration_path)
                {
                    self.realpath_dts_to_source
                        .store(path, Some(realpath_dts_to_source.clone()));
                    return Some(realpath_dts_to_source);
                } else {
                    self.realpath_dts_to_source.store(path, None);
                }
            }
        }
        None
    }
}
