use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use ts_ast as ast;
use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::file_include::ReferenceFileLocation;
use crate::{
    FileIncludeReason, IncludeExplainingDiagnostic, ProcessingDiagnostic, ProcessingDiagnosticData,
    ProcessingDiagnosticKind, Program,
};

macro_rules! diagnostic_args {
    ($($arg:expr),* $(,)?) => {
        vec![$(Box::new($arg) as diagnostics::Argument),*]
    };
}

#[derive(Default)]
pub(crate) struct IncludeProcessor {
    pub(crate) file_include_reasons: HashMap<tspath::Path, Vec<FileIncludeReason>>,
    pub(crate) processing_diagnostics: Mutex<Vec<ProcessingDiagnostic>>,

    pub(crate) reason_to_reference_location:
        collections::SyncMap<FileIncludeReason, ReferenceFileLocation>,
    pub(crate) include_reason_to_related_info:
        collections::SyncMap<FileIncludeReason, ast::Diagnostic>,
    pub(crate) redirect_and_file_format: collections::SyncMap<tspath::Path, Vec<ast::Diagnostic>>,
    pub(crate) computed_diagnostics: OnceLock<ast::DiagnosticsCollection>,
    pub(crate) compiler_options_syntax: OnceLock<Option<ast::Node>>,
}

impl Clone for IncludeProcessor {
    fn clone(&self) -> Self {
        Self {
            file_include_reasons: self.file_include_reasons.clone(),
            processing_diagnostics: Mutex::new(
                self.processing_diagnostics
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone(),
            ),
            ..Default::default()
        }
    }
}

pub(crate) fn update_file_include_processor(p: &mut Program) {
    let file_include_reasons = p
        .processed_files
        .include_processor
        .file_include_reasons
        .clone();
    let processing_diagnostics = p
        .processed_files
        .include_processor
        .processing_diagnostics
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone();
    p.processed_files.include_processor = IncludeProcessor {
        file_include_reasons,
        processing_diagnostics: Mutex::new(processing_diagnostics),
        ..Default::default()
    };
}

impl IncludeProcessor {
    pub(crate) fn get_diagnostics(&self, p: &Program) -> &ast::DiagnosticsCollection {
        self.computed_diagnostics.get_or_init(|| {
            let computed_diagnostics = ast::DiagnosticsCollection::default();
            for d in self
                .processing_diagnostics
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .iter()
            {
                computed_diagnostics.add(d.to_diagnostic(p));
            }
            for resolutions in p.processed_files.resolved_modules.values() {
                for resolved_module in resolutions.values() {
                    for diag in &resolved_module.resolution_diagnostics {
                        computed_diagnostics.add(diag.clone());
                    }
                }
            }
            for type_resolutions in p.processed_files.type_resolutions_in_file.values() {
                for resolved_type_ref in type_resolutions.values() {
                    for diag in &resolved_type_ref.resolution_diagnostics {
                        computed_diagnostics.add(diag.clone());
                    }
                }
            }
            computed_diagnostics
        })
    }

    pub(crate) fn add_processing_diagnostic(&self, d: Vec<ProcessingDiagnostic>) {
        self.processing_diagnostics
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .extend(d);
    }

    pub(crate) fn add_processing_diagnostics_for_file_casing(
        &mut self,
        file: &tspath::Path,
        existing_casing: &str,
        current_casing: &str,
        reason: Option<FileIncludeReason>,
    ) {
        if !reason.as_ref().is_some_and(|r| r.is_referenced_file())
            && self
                .file_include_reasons
                .get(file)
                .is_some_and(|reasons| reasons.iter().any(|r| r.is_referenced_file()))
        {
            self.add_processing_diagnostic(vec![ProcessingDiagnostic {
                kind: ProcessingDiagnosticKind::ExplainingFileInclude,
                data: ProcessingDiagnosticData::IncludeExplaining(IncludeExplainingDiagnostic {
                    file: Some(file.clone()),
                    diagnostic_reason: reason,
                    message: &diagnostics::Already_included_file_name_0_differs_from_file_name_1_only_in_casing,
                    args: vec![existing_casing.to_string(), current_casing.to_string()],
                }),
            }]);
        } else {
            self.add_processing_diagnostic(vec![ProcessingDiagnostic {
                kind: ProcessingDiagnosticKind::ExplainingFileInclude,
                data: ProcessingDiagnosticData::IncludeExplaining(IncludeExplainingDiagnostic {
                    file: Some(file.clone()),
                    diagnostic_reason: reason,
                    message: &diagnostics::File_name_0_differs_from_already_included_file_name_1_only_in_casing,
                    args: vec![current_casing.to_string(), existing_casing.to_string()],
                }),
            }]);
        }
    }

    pub(crate) fn get_reference_location(
        &self,
        r: &FileIncludeReason,
        program: &Program,
    ) -> ReferenceFileLocation {
        let (existing, ok) = self.reason_to_reference_location.load(r);
        if ok {
            return existing.unwrap();
        }

        let (loc, _) = self
            .reason_to_reference_location
            .load_or_store(r.clone(), Some(r.get_referenced_location(program)));
        loc.unwrap()
    }

    pub(crate) fn get_compiler_options_object_literal_syntax(
        &self,
        program: &Program,
    ) -> Option<ast::Node> {
        self.compiler_options_syntax
            .get_or_init(|| {
                let config_file = program.opts.config.config_file.as_ref();
                if let Some(config_file) = config_file {
                    let store = config_file.source_file.store();
                    if let Some(compiler_options_property) =
                        tsoptions::for_each_ts_config_prop_array(
                            Some(&config_file.source_file),
                            "compilerOptions",
                            Some,
                        )
                    {
                        if let Some(initializer) = store.initializer(compiler_options_property)
                            && ast::is_object_literal_expression(store, initializer)
                        {
                            return Some(initializer);
                        }
                    }
                } else {
                    return None;
                }
                None
            })
            .clone()
    }

    pub(crate) fn get_related_info(
        &self,
        r: &FileIncludeReason,
        program: &Program,
    ) -> Option<ast::Diagnostic> {
        let (existing, ok) = self.include_reason_to_related_info.load(r);
        if ok {
            return existing;
        }

        let (related_info, _) = self
            .include_reason_to_related_info
            .load_or_store(r.clone(), r.to_related_info(program));
        related_info
    }

    pub(crate) fn explain_redirect_and_implied_format(
        &self,
        program: &Program,
        file_path: tspath::Path,
        to_file_name: impl Fn(&str) -> String,
    ) -> Vec<ast::Diagnostic> {
        let (existing, ok) = self.redirect_and_file_format.load(&file_path);
        if ok {
            return existing.unwrap();
        }

        let mut source_file = None;
        let redirects_file = program
            .processed_files
            .redirect_files_by_path
            .get(&file_path);
        if redirects_file.is_none() {
            source_file = program.get_source_file_by_path(file_path.clone());
        }
        let file: &dyn ast::HasFileName = if let Some(redirects_file) = redirects_file {
            redirects_file
        } else {
            source_file.as_ref().unwrap()
        };

        let mut result = Vec::new();
        let source = program.get_source_of_project_reference_if_output_included(file);
        if source != file.file_name() {
            result.push(ast::new_compiler_diagnostic(
                &diagnostics::File_is_output_of_project_reference_source_0,
                &diagnostic_args![to_file_name(&source)],
            ));
        }

        if let Some(redirects_file) = redirects_file {
            let target_file = program
                .get_source_file_by_path(redirects_file.target.clone())
                .unwrap();
            result.push(ast::new_compiler_diagnostic(
                &diagnostics::File_redirects_to_file_0,
                &diagnostic_args![to_file_name(&target_file.file_name())],
            ));
        }

        if source_file
            .as_ref()
            .is_some_and(|source_file| ast::is_external_or_common_js_module(source_file))
        {
            let meta_data = program.get_source_file_meta_data(file.path());
            match program.get_implied_node_format_for_emit(file) {
                core::ModuleKind::EsNext => {
                    if meta_data.package_json_type == "module" {
                        result.push(ast::new_compiler_diagnostic(
                            &diagnostics::File_is_ECMAScript_module_because_0_has_field_type_with_value_module,
                            &diagnostic_args![to_file_name(&format!(
                                "{}/package.json",
                                meta_data.package_json_directory
                            ))],
                        ));
                    }
                }
                core::ModuleKind::CommonJs => {
                    if !meta_data.package_json_type.is_empty() {
                        result.push(ast::new_compiler_diagnostic(
                            &diagnostics::File_is_CommonJS_module_because_0_has_field_type_whose_value_is_not_module,
                            &diagnostic_args![to_file_name(&format!(
                                "{}/package.json",
                                meta_data.package_json_directory
                            ))],
                        ));
                    } else if !meta_data.package_json_directory.is_empty() {
                        if meta_data.package_json_type.is_empty() {
                            result.push(ast::new_compiler_diagnostic(
                                &diagnostics::File_is_CommonJS_module_because_0_does_not_have_field_type,
                                &diagnostic_args![to_file_name(&format!(
                                    "{}/package.json",
                                    meta_data.package_json_directory
                                ))],
                            ));
                        }
                    } else {
                        result.push(ast::new_compiler_diagnostic(
                            &diagnostics::File_is_CommonJS_module_because_package_json_was_not_found,
                            &[],
                        ));
                    }
                }
                _ => {}
            }
        }

        let (result, _) = self
            .redirect_and_file_format
            .load_or_store(file_path, Some(result));
        result.unwrap()
    }
}
