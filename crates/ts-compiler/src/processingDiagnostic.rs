use ts_ast as ast;
use ts_core as core;
use ts_diagnostics::{self as diagnostics, Any};
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::{
    FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE, FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE,
    FileIncludeReason, Program,
};

#[derive(Clone)]
pub(crate) enum ProcessingDiagnosticKind {
    UnknownReference,
    ExplainingFileInclude,
}

#[derive(Clone)]
pub(crate) struct ProcessingDiagnostic {
    pub(crate) kind: ProcessingDiagnosticKind,
    pub(crate) data: ProcessingDiagnosticData,
}

#[derive(Clone)]
pub(crate) enum ProcessingDiagnosticData {
    FileIncludeReason(FileIncludeReason),
    IncludeExplaining(IncludeExplainingDiagnostic),
}

impl ProcessingDiagnostic {
    fn as_file_include_reason(&self) -> &FileIncludeReason {
        match &self.data {
            ProcessingDiagnosticData::FileIncludeReason(reason) => reason,
            _ => panic!("expected FileIncludeReason"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct IncludeExplainingDiagnostic {
    pub(crate) file: Option<tspath::Path>,
    pub(crate) diagnostic_reason: Option<FileIncludeReason>,
    pub(crate) message: &'static diagnostics::Message,
    pub(crate) args: Vec<String>,
}

fn diagnostic_args(args: &[String]) -> Vec<Any> {
    args.iter()
        .cloned()
        .map(|arg| Box::new(arg) as Any)
        .collect()
}

impl ProcessingDiagnostic {
    fn as_include_explaining_diagnostic(&self) -> &IncludeExplainingDiagnostic {
        match &self.data {
            ProcessingDiagnosticData::IncludeExplaining(diag) => diag,
            _ => panic!("expected includeExplainingDiagnostic"),
        }
    }

    pub(crate) fn to_diagnostic(&self, program: &Program) -> ast::Diagnostic {
        match self.kind {
            ProcessingDiagnosticKind::UnknownReference => {
                let r#ref = self.as_file_include_reason();
                let loc = r#ref.get_referenced_location(program);
                match r#ref.kind {
                    FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE => loc.diagnostic_at(
                        &diagnostics::Cannot_find_type_definition_file_for_0,
                        diagnostic_args(&[loc.r#ref.as_ref().unwrap().file_name.clone()]),
                    ),
                    FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE => {
                        let lib_name =
                            tspath::to_file_name_lower_case(&loc.r#ref.as_ref().unwrap().file_name);
                        let unqualified_lib_name =
                            lib_name.strip_prefix("lib.").unwrap_or(&lib_name);
                        let unqualified_lib_name = unqualified_lib_name
                            .strip_suffix(".d.ts")
                            .unwrap_or(unqualified_lib_name);
                        let suggestion = core::get_spelling_suggestion_for_strings(
                            unqualified_lib_name,
                            tsoptions::LIBS.iter().cloned(),
                        );
                        loc.diagnostic_at(
                            if !suggestion.is_empty() {
                                &diagnostics::Cannot_find_lib_definition_for_0_Did_you_mean_1
                            } else {
                                &diagnostics::Cannot_find_lib_definition_for_0
                            },
                            diagnostic_args(&[lib_name, suggestion]),
                        )
                    }
                    _ => panic!("unknown include kind"),
                }
            }
            ProcessingDiagnosticKind::ExplainingFileInclude => {
                self.create_diagnostic_explaining_file(program)
            }
        }
    }

    fn create_diagnostic_explaining_file(&self, program: &Program) -> ast::Diagnostic {
        let diag = self.as_include_explaining_diagnostic();
        let mut include_details: Option<Vec<ast::Diagnostic>> = None;
        let mut related_info: Vec<ast::Diagnostic> = Vec::new();
        let mut redirect_info: Vec<ast::Diagnostic> = Vec::new();
        let mut preferred_location: Option<FileIncludeReason> = None;
        let mut preferred_location_key: Option<*const FileIncludeReason> = None;
        let mut seen_reasons: std::collections::HashSet<*const FileIncludeReason> =
            std::collections::HashSet::new();
        if diag.diagnostic_reason.as_ref().is_some_and(|reason| {
            reason.is_referenced_file()
                && !program
                    .processed_files
                    .include_processor
                    .get_reference_location(reason, program)
                    .is_synthetic
        }) {
            preferred_location = diag.diagnostic_reason.clone();
            preferred_location_key = diag
                .diagnostic_reason
                .as_ref()
                .map(|reason| reason as *const FileIncludeReason);
        }

        fn process_related_info(
            program: &Program,
            include_reason: &FileIncludeReason,
            preferred_location: &mut Option<FileIncludeReason>,
            preferred_location_key: &mut Option<*const FileIncludeReason>,
            related_info: &mut Vec<ast::Diagnostic>,
        ) {
            if preferred_location.is_none()
                && include_reason.is_referenced_file()
                && !program
                    .processed_files
                    .include_processor
                    .get_reference_location(include_reason, program)
                    .is_synthetic
            {
                *preferred_location = Some(include_reason.clone());
                *preferred_location_key = Some(include_reason as *const FileIncludeReason);
            } else if match preferred_location {
                Some(_) => !std::ptr::eq(
                    preferred_location_key.unwrap(),
                    include_reason as *const FileIncludeReason,
                ),
                None => true,
            } {
                let info = program
                    .processed_files
                    .include_processor
                    .get_related_info(include_reason, program);
                if let Some(info) = info {
                    related_info.push(info);
                }
            }
        }
        fn process_include(
            program: &Program,
            include_reason: &FileIncludeReason,
            include_details: &mut Option<Vec<ast::Diagnostic>>,
            related_info: &mut Vec<ast::Diagnostic>,
            preferred_location: &mut Option<FileIncludeReason>,
            preferred_location_key: &mut Option<*const FileIncludeReason>,
            seen_reasons: &mut std::collections::HashSet<*const FileIncludeReason>,
        ) {
            if !seen_reasons.insert(include_reason as *const FileIncludeReason) {
                return;
            }
            include_details
                .get_or_insert_with(Vec::new)
                .push(include_reason.to_diagnostic(program, false));
            process_related_info(
                program,
                include_reason,
                preferred_location,
                preferred_location_key,
                related_info,
            );
        }

        // !!! todo sheetal caching

        if let Some(file) = &diag.file {
            let reasons = program
                .processed_files
                .include_processor
                .file_include_reasons
                .get(file);
            include_details = Some(Vec::with_capacity(reasons.map_or(0, Vec::len)));
            if let Some(reasons) = reasons {
                for reason in reasons {
                    process_include(
                        program,
                        reason,
                        &mut include_details,
                        &mut related_info,
                        &mut preferred_location,
                        &mut preferred_location_key,
                        &mut seen_reasons,
                    );
                }
            }
            redirect_info = program
                .processed_files
                .include_processor
                .explain_redirect_and_implied_format(program, file.clone(), |file_name| {
                    file_name.to_string()
                });
        }
        if let Some(diagnostic_reason) = &diag.diagnostic_reason {
            process_include(
                program,
                diagnostic_reason,
                &mut include_details,
                &mut related_info,
                &mut preferred_location,
                &mut preferred_location_key,
                &mut seen_reasons,
            );
        }
        let mut chain = Vec::new();
        if let Some(include_details) = include_details {
            if preferred_location.is_none() || seen_reasons.len() != 1 {
                let mut file_reason = ast::new_compiler_diagnostic(
                    &diagnostics::The_file_is_in_the_program_because_Colon,
                    &[],
                );
                file_reason.set_message_chain(include_details);
                chain.push(file_reason);
            }
        }
        if !redirect_info.is_empty() {
            chain.extend(redirect_info);
        }

        let mut result = preferred_location.as_ref().map(|preferred_location| {
            program
                .processed_files
                .include_processor
                .get_reference_location(preferred_location, program)
                .diagnostic_at(diag.message, diagnostic_args(&diag.args))
        });
        if result.is_none() {
            let args = diagnostic_args(&diag.args);
            result = Some(ast::new_compiler_diagnostic(diag.message, &args));
        }
        let mut result = result.unwrap();
        if !chain.is_empty() {
            result.set_message_chain(chain);
        }
        if !related_info.is_empty() {
            result.set_related_info(related_info);
        }
        result
    }
}
