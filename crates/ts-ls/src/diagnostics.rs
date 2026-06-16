use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;

use crate::LanguageService;
use crate::lsconv;

// getAllDiagnostics collects all diagnostics for a file: syntactic, semantic,
// suggestion, and (when declarations are emitted) declaration diagnostics.
pub fn get_all_diagnostics<'a>(
    ctx: &core::Context,
    program: &'a compiler::Program,
    file: &'a ast::SourceFile,
) -> Vec<ast::Diagnostic> {
    let mut diags = Vec::new();
    diags.extend(program.get_syntactic_diagnostics(ctx.clone(), Some(file)));
    diags.extend(program.get_semantic_diagnostics(ctx.clone(), Some(file)));
    diags.extend(program.get_suggestion_diagnostics(ctx.clone(), Some(file)));
    if program.options().get_emit_declarations() {
        diags.extend(program.get_declaration_diagnostics(ctx.clone(), Some(file)));
    }
    diags
}

pub fn get_all_diagnostics_with_checker<'a>(
    ctx: &core::Context,
    program: &'a compiler::Program,
    file: &'a ast::SourceFile,
    checker: &mut checker::Checker<'a, '_>,
) -> Vec<ast::Diagnostic> {
    let mut diags = Vec::new();
    diags.extend(program.get_syntactic_diagnostics(ctx.clone(), Some(file)));
    diags.extend(program.get_semantic_diagnostics_with_checker(ctx.clone(), checker, file));
    diags.extend(program.get_suggestion_diagnostics_with_checker(ctx.clone(), checker, file));
    if program.options().get_emit_declarations() {
        diags.extend(program.get_declaration_diagnostics_with_checker(ctx.clone(), checker, file));
    }
    diags
}

impl LanguageService<'_> {
    pub fn provide_diagnostics(
        &self,
        ctx: &core::Context,
        uri: lsproto::DocumentUri,
    ) -> Result<lsproto::DocumentDiagnosticResponse, core::Error> {
        let (program, file) = self.get_program_and_file(uri);

        let diagnostics = get_all_diagnostics(ctx, program, file);

        Ok(
            lsproto::RelatedFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport {
                full_document_diagnostic_report: Some(
                    lsproto::RelatedFullDocumentDiagnosticReport {
                        kind: lsproto::StringLiteralFull,
                        result_id: None,
                        items: self.to_lsp_diagnostics(ctx, &[diagnostics]),
                        related_documents: None,
                    },
                ),
                ..Default::default()
            },
        )
    }

    pub fn to_lsp_diagnostics(
        &self,
        ctx: &core::Context,
        diagnostics: &[Vec<ast::Diagnostic>],
    ) -> Vec<lsproto::Diagnostic> {
        let mut size = 0;
        for diag_slice in diagnostics {
            size += diag_slice.len();
        }
        let mut lsp_diagnostics = Vec::with_capacity(size);
        for diag_slice in diagnostics {
            for diag in diag_slice {
                lsp_diagnostics.push(lsconv::diagnostic_to_lsp_pull(
                    ctx,
                    &self.converters,
                    diag,
                    self.user_preferences()
                        .report_style_checks_as_warnings
                        .is_true(),
                ));
            }
        }
        lsp_diagnostics
    }
}
