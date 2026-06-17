use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use ts_ast as ast;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_lsproto as lsproto;

use crate::LanguageService;
use crate::codeactions_fixclassincorrectlyimplementsinterface::FIX_CLASS_INCORRECTLY_IMPLEMENTS_INTERFACE_PROVIDER;
use crate::codeactions_fixmissingtypeannotation::ISOLATED_DECLARATIONS_FIX_PROVIDER;
use crate::codeactions_importfixes::IMPORT_FIX_PROVIDER;
use crate::codeactions_spelling::SPELLING_PROVIDER;
use crate::diagnostics::get_all_diagnostics;
use crate::lsconv;

// CodeFixProvider represents a provider for a specific type of code fix
pub struct CodeFixProvider {
    pub error_codes: fn() -> Vec<i32>,
    pub get_code_actions:
        fn(&core::Context, &CodeFixContext) -> Result<Vec<CodeAction>, core::Error>,
    pub fix_ids: &'static [&'static str],
    pub get_all_code_actions: Option<
        fn(&core::Context, &CodeFixContext) -> Result<Option<CombinedCodeActions>, core::Error>,
    >,
}

// CodeFixContext contains the context needed to generate code fixes
pub struct CodeFixContext<'a> {
    pub source_file: &'a ast::SourceFile,
    pub span: core::TextRange,
    pub error_code: i32,
    pub program: &'a compiler::Program,
    pub ls: &'a LanguageService<'a>,
    pub diagnostic: Option<&'a lsproto::Diagnostic>,
    pub params: Option<&'a lsproto::CodeActionParams>,
}

// CodeAction represents a single code action fix
pub struct CodeAction {
    pub description: String,
    pub changes: Vec<lsproto::TextEdit>,
    pub fix_id: String,
    pub fix_all_description: String,
}

impl CodeAction {
    // Compare defines a total ordering for CodeAction values, comparing description
    // then text edits lexicographically. Used with slices.BinarySearchFunc.
    pub fn compare(&self, b: &CodeAction) -> i32 {
        match self.description.cmp(&b.description) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {}
        }
        match self.changes.len().cmp(&b.changes.len()) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {}
        }
        for (i, edit) in self.changes.iter().enumerate() {
            let c = edit.compare(&b.changes[i]);
            if c != 0 {
                return c;
            }
        }
        0
    }
}

// CombinedCodeActions represents combined code actions for fix-all scenarios
pub struct CombinedCodeActions {
    pub description: String,
    pub changes: Vec<lsproto::TextEdit>,
}

// codeFixProviders is the list of all registered code fix providers
pub static CODE_FIX_PROVIDERS: &[&CodeFixProvider] = &[
    &SPELLING_PROVIDER,
    &IMPORT_FIX_PROVIDER,
    &ISOLATED_DECLARATIONS_FIX_PROVIDER,
    &FIX_CLASS_INCORRECTLY_IMPLEMENTS_INTERFACE_PROVIDER,
    // Add more code fix providers here as they are implemented
];

impl LanguageService<'_> {
    // ProvideCodeActions returns code actions for the given range and context
    pub fn provide_code_actions(
        &self,
        ctx: &core::Context,
        params: &lsproto::CodeActionParams,
    ) -> Result<lsproto::CodeActionResponse, core::Error> {
        let (program, file) = self.get_program_and_file(params.text_document.uri.clone());

        let mut actions: Vec<lsproto::CommandOrCodeAction> = Vec::new();

        if let Some(only) = &params.context.only {
            for kind in only {
                let matching_kinds = get_organize_imports_actions_for_kind(kind.clone());
                for matching_kind in matching_kinds {
                    let organize_action =
                        self.create_organize_imports_action(ctx, program, file, matching_kind)?;
                    actions.push(organize_action);
                }

                if is_fix_all_kind(kind.clone()) {
                    let fix_all_action = self.create_fix_all_action(
                        ctx,
                        program,
                        file,
                        params.text_document.uri.clone(),
                    )?;
                    if let Some(fix_all_action) = fix_all_action {
                        actions.push(fix_all_action);
                    }
                }
            }
        }

        if !params.context.diagnostics.is_empty() && wants_quick_fixes(params.context.only.as_ref())
        {
            let mut fix_id_seen: HashMap<String, &CodeFixProvider> = HashMap::new();

            for diag in &params.context.diagnostics {
                if diag.code.is_none() || diag.code.as_ref().unwrap().integer.is_none() {
                    continue;
                }

                let error_code = diag.code.as_ref().unwrap().integer.unwrap();

                for provider in CODE_FIX_PROVIDERS {
                    let error_codes = (provider.error_codes)();
                    if !contains_error_code(&error_codes, error_code) {
                        continue;
                    }

                    let position = self
                        .converters
                        .line_and_character_to_position(file, diag.range.start);
                    let end_position = self
                        .converters
                        .line_and_character_to_position(file, diag.range.end);
                    let fix_context = CodeFixContext {
                        source_file: file,
                        span: core::new_text_range(position as i32, end_position as i32),
                        error_code,
                        program,
                        ls: self,
                        diagnostic: Some(diag),
                        params: Some(params),
                    };

                    let provider_actions = (provider.get_code_actions)(ctx, &fix_context)?;
                    for action in provider_actions {
                        actions.push(convert_to_lsp_code_action(
                            &action,
                            diag,
                            params.text_document.uri.clone(),
                        ));
                        if !action.fix_id.is_empty() {
                            fix_id_seen.insert(action.fix_id.clone(), provider);
                        }
                    }
                }
            }

            let fix_all_actions = self.get_fix_all_quick_fixes(
                ctx,
                program,
                file,
                params.text_document.uri.clone(),
                fix_id_seen,
            )?;
            actions.extend(fix_all_actions);
        }

        Ok(lsproto::CommandOrCodeActionArrayOrNull {
            command_or_code_action_array: Some(actions),
        })
    }

    // getFixAllQuickFixes returns per-provider "Fix all in file" quickfix entries for providers
    // that matched at least 2 diagnostics in the full file.
    pub fn get_fix_all_quick_fixes(
        &self,
        ctx: &core::Context,
        program: &compiler::Program,
        file: &ast::SourceFile,
        uri: lsproto::DocumentUri,
        fix_id_seen: HashMap<String, &CodeFixProvider>,
    ) -> Result<Vec<lsproto::CommandOrCodeAction>, core::Error> {
        let mut actions = Vec::new();

        // Deduplicate providers; multiple fixIds may map to the same provider.
        let mut seen: HashSet<*const CodeFixProvider> = HashSet::new();
        for provider in fix_id_seen.values() {
            let provider_key = *provider as *const CodeFixProvider;
            if seen.contains(&provider_key) {
                continue;
            }
            seen.insert(provider_key);

            let Some(get_all_code_actions) = provider.get_all_code_actions else {
                continue;
            };

            let error_codes = (provider.error_codes)();
            if !has_multiple_fixable_diagnostics(ctx, program, file, &error_codes) {
                continue;
            }

            let fix_context = CodeFixContext {
                source_file: file,
                span: core::TextRange::default(),
                error_code: 0,
                program,
                ls: self,
                diagnostic: None,
                params: None,
            };
            let combined = get_all_code_actions(ctx, &fix_context)?;
            if combined
                .as_ref()
                .is_some_and(|combined| !combined.changes.is_empty())
            {
                let combined = combined.unwrap();
                let kind = lsproto::CodeActionKind::QuickFix;
                let mut changes = HashMap::new();
                changes.insert(uri.clone(), combined.changes);
                actions.push(lsproto::CommandOrCodeAction {
                    code_action: Some(lsproto::CodeAction {
                        title: combined.description,
                        kind: Some(kind),
                        edit: Some(lsproto::WorkspaceEdit {
                            changes: Some(changes),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                });
            }
        }

        Ok(actions)
    }

    // createFixAllAction creates a source.fixAll code action that applies all auto-fixable
    // code fixes across the file.
    pub fn create_fix_all_action(
        &self,
        ctx: &core::Context,
        program: &compiler::Program,
        file: &ast::SourceFile,
        uri: lsproto::DocumentUri,
    ) -> Result<Option<lsproto::CommandOrCodeAction>, core::Error> {
        let kind = lsproto::CodeActionKind::SourceFixAll;
        let mut lsp_changes: HashMap<lsproto::DocumentUri, Vec<lsproto::TextEdit>> = HashMap::new();

        for provider in CODE_FIX_PROVIDERS {
            let Some(get_all_code_actions) = provider.get_all_code_actions else {
                continue;
            };

            let fix_context = CodeFixContext {
                source_file: file,
                span: core::TextRange::default(),
                error_code: 0,
                program,
                ls: self,
                diagnostic: None,
                params: None,
            };

            let combined = get_all_code_actions(ctx, &fix_context)?;
            if combined
                .as_ref()
                .is_some_and(|combined| !combined.changes.is_empty())
            {
                lsp_changes
                    .entry(uri.clone())
                    .or_default()
                    .extend(combined.unwrap().changes);
            }
        }

        if lsp_changes.is_empty() {
            return Ok(None);
        }

        Ok(Some(lsproto::CommandOrCodeAction {
            code_action: Some(lsproto::CodeAction {
                title: diagnostics::FIX_ALL.localize(locale::und(), vec![]),
                kind: Some(kind),
                edit: Some(lsproto::WorkspaceEdit {
                    changes: Some(lsp_changes),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }))
    }

    // createOrganizeImportsAction creates the organize imports code action
    pub fn create_organize_imports_action(
        &self,
        ctx: &core::Context,
        program: &compiler::Program,
        file: &ast::SourceFile,
        kind: lsproto::CodeActionKind,
    ) -> Result<lsproto::CommandOrCodeAction, core::Error> {
        let title = get_organize_imports_action_title(ctx, kind.clone());
        let changes = self.organize_imports(ctx, file, program, kind.clone())?;
        if changes.is_empty() {
            return Ok(lsproto::CommandOrCodeAction {
                code_action: Some(lsproto::CodeAction {
                    title,
                    kind: Some(kind),
                    edit: Some(lsproto::WorkspaceEdit {
                        changes: Some(HashMap::new()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            });
        }

        let mut lsp_changes = HashMap::new();
        for (file_name, edits) in changes {
            let file_uri = lsconv::file_name_to_document_uri(&file_name);
            lsp_changes.insert(file_uri, edits);
        }

        Ok(lsproto::CommandOrCodeAction {
            code_action: Some(lsproto::CodeAction {
                title,
                kind: Some(kind),
                edit: Some(lsproto::WorkspaceEdit {
                    changes: Some(lsp_changes),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        })
    }
}

// hasMultipleFixableDiagnostics returns true if the file has at least 2 diagnostics
// matching the given error codes. Checks all diagnostic sources (semantic,
// syntactic, suggestion, declaration) to match ProvideDiagnostics.
pub fn has_multiple_fixable_diagnostics(
    ctx: &core::Context,
    program: &compiler::Program,
    file: &ast::SourceFile,
    error_codes: &[i32],
) -> bool {
    let all_diags = get_all_diagnostics(ctx, program, file);
    let mut count = 0;
    for d in all_diags {
        if contains_error_code(error_codes, d.code()) {
            count += 1;
            if count >= 2 {
                return true;
            }
        }
    }
    false
}

// codeActionKindContains returns true if the requested kind equals or is a
// hierarchical parent of actionKind, using '.' as the separator. This matches
// the semantics of VS Code's HierarchicalKind.contains.
pub fn code_action_kind_contains(
    requested_kind: lsproto::CodeActionKind,
    action_kind: lsproto::CodeActionKind,
) -> bool {
    requested_kind == action_kind
        || requested_kind.as_str().is_empty()
        || action_kind
            .as_str()
            .starts_with(&(requested_kind.as_str().to_string() + "."))
}

// isFixAllKind returns true if the requested kind matches source.fixAll
pub fn is_fix_all_kind(kind: lsproto::CodeActionKind) -> bool {
    code_action_kind_contains(kind, lsproto::CodeActionKind::SourceFixAll)
}

// wantsQuickFixes returns true if the Only filter is nil/empty (meaning all kinds are wanted)
// or explicitly includes the quickfix kind.
pub fn wants_quick_fixes(only: Option<&Vec<lsproto::CodeActionKind>>) -> bool {
    let Some(only) = only else {
        return true;
    };
    if only.is_empty() {
        return true;
    }
    for kind in only {
        if code_action_kind_contains(kind.clone(), lsproto::CodeActionKind::QuickFix) {
            return true;
        }
    }
    false
}

// getOrganizeImportsActionTitle returns the appropriate title for the given organize imports kind
pub fn get_organize_imports_action_title(
    _ctx: &core::Context,
    kind: lsproto::CodeActionKind,
) -> String {
    let loc = locale::und();
    match kind {
        lsproto::CodeActionKind::SourceRemoveUnusedImports => {
            diagnostics::REMOVE_UNUSED_IMPORTS.localize(loc, vec![])
        }
        lsproto::CodeActionKind::SourceSortImports => {
            diagnostics::SORT_IMPORTS.localize(loc, vec![])
        }
        _ => diagnostics::ORGANIZE_IMPORTS.localize(loc, vec![]),
    }
}

// getOrganizeImportsActionsForKind returns the organize imports code action kinds that should be
// returned for the given requested kind.
pub fn get_organize_imports_actions_for_kind(
    requested_kind: lsproto::CodeActionKind,
) -> Vec<lsproto::CodeActionKind> {
    let organize_imports_kinds = vec![
        lsproto::CodeActionKind::SourceOrganizeImports,
        lsproto::CodeActionKind::SourceRemoveUnusedImports,
        lsproto::CodeActionKind::SourceSortImports,
    ];

    let mut result = Vec::new();
    for organize_kind in organize_imports_kinds {
        if code_action_kind_contains(requested_kind.clone(), organize_kind.clone()) {
            result.push(organize_kind);
        }
    }

    if result.contains(&requested_kind) {
        return vec![requested_kind];
    }

    result
}

// containsErrorCode checks if the error code is in the list
pub fn contains_error_code(codes: &[i32], code: i32) -> bool {
    codes.contains(&code)
}

// convertToLSPCodeAction converts an internal CodeAction to an LSP CodeAction
pub fn convert_to_lsp_code_action(
    action: &CodeAction,
    diag: &lsproto::Diagnostic,
    uri: lsproto::DocumentUri,
) -> lsproto::CommandOrCodeAction {
    let kind = lsproto::CodeActionKind::QuickFix;
    let mut changes = HashMap::new();
    changes.insert(uri, action.changes.clone());
    let diagnostics = vec![diag.clone()];

    lsproto::CommandOrCodeAction {
        code_action: Some(lsproto::CodeAction {
            title: action.description.clone(),
            kind: Some(kind),
            edit: Some(lsproto::WorkspaceEdit {
                changes: Some(changes),
                ..Default::default()
            }),
            diagnostics: Some(diagnostics),
            ..Default::default()
        }),
        ..Default::default()
    }
}
