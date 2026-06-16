use std::collections::HashSet;
use std::sync::atomic::Ordering;

use ts_ast as ast;
use ts_binder as binder;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use super::{
    DiagnosticsOrBuildInfoDiagnosticsWithFileName, FILE_EMIT_KIND_NONE, FileEmitKind, FileInfo,
    Program, Snapshot, get_file_emit_kind, get_pending_emit_kind_with_options,
    repopulate_diagnostic_chain,
};

pub fn program_to_snapshot(
    program: &compiler::Program,
    old_program: Option<&Program>,
    hash_with_text: bool,
) -> Snapshot {
    if let Some(old_program) = old_program {
        if old_program
            .program
            .as_ref()
            .is_some_and(|old| std::ptr::eq(old, program))
        {
            return old_program.snapshot.clone();
        }
    }
    let snapshot = Snapshot {
        options: program.options().clone(),
        hash_with_text,
        check_pending: program.options().no_check.is_true(),
        ..Default::default()
    };
    let mut to = ToProgramSnapshot {
        program,
        old_program,
        snapshot,
        global_file_removed: false,
    };

    if to.snapshot.can_use_incremental_state() {
        to.reuse_from_old_program();
        to.compute_program_file_changes();
        to.handle_file_delete();
        to.handle_pending_emit();
        to.handle_pending_check();
    }
    to.snapshot
}

pub struct ToProgramSnapshot<'a> {
    pub program: &'a compiler::Program,
    pub old_program: Option<&'a Program>,
    pub snapshot: Snapshot,
    pub global_file_removed: bool,
}

impl<'a> ToProgramSnapshot<'a> {
    pub fn reuse_from_old_program(&mut self) {
        if let Some(old_program) = self.old_program {
            if self.snapshot.options.composite.is_true() {
                self.snapshot.latest_changed_dts_file =
                    old_program.snapshot.latest_changed_dts_file.clone();
            }
            // Copy old snapshot's changed files set
            for key in &old_program.snapshot.changed_files_set {
                self.snapshot.changed_files_set.insert(key.clone());
            }
            for (key, emit_kind) in &old_program.snapshot.affected_files_pending_emit {
                self.snapshot
                    .affected_files_pending_emit
                    .insert(key.clone(), *emit_kind);
            }
            self.snapshot.build_info_emit_pending.store(
                old_program
                    .snapshot
                    .build_info_emit_pending
                    .load(Ordering::SeqCst),
                Ordering::SeqCst,
            );
            self.snapshot.has_errors_from_old_state = old_program.snapshot.has_errors;
            self.snapshot.has_semantic_errors_from_old_state =
                old_program.snapshot.has_semantic_errors;
        } else {
            self.snapshot
                .build_info_emit_pending
                .store(self.snapshot.options.is_incremental(), Ordering::SeqCst);
        }
    }

    pub fn compute_program_file_changes(&mut self) {
        let can_copy_semantic_diagnostics = self.old_program.is_some()
            && !tsoptions::compiler_options_affect_semantic_diagnostics(
                self.old_program.unwrap().snapshot.options.clone(),
                self.program.options(),
            );
        // We can only reuse emit signatures (i.e. .d.ts signatures) if the .d.ts file is unchanged,
        // which will eg be depedent on change in options like declarationDir and outDir options are unchanged.
        // We need to look in oldState.compilerOptions, rather than oldCompilerOptions (i.e.we need to disregard useOldState) because
        // oldCompilerOptions can be undefined if there was change in say module from None to some other option
        // which would make useOldState as false since we can now use reference maps that are needed to track what to emit, what to check etc
        // but that option change does not affect d.ts file name so emitSignatures should still be reused.
        let can_copy_emit_signatures = self.snapshot.options.composite.is_true()
            && self.old_program.is_some()
            && !tsoptions::compiler_options_affect_declaration_path(
                self.old_program.unwrap().snapshot.options.clone(),
                self.program.options(),
            );
        let copy_declaration_file_diagnostics = can_copy_semantic_diagnostics
            && self.snapshot.options.skip_lib_check.is_true()
                == self
                    .old_program
                    .unwrap()
                    .snapshot
                    .options
                    .skip_lib_check
                    .is_true();
        let copy_lib_file_diagnostics = copy_declaration_file_diagnostics
            && self.snapshot.options.skip_default_lib_check.is_true()
                == self
                    .old_program
                    .unwrap()
                    .snapshot
                    .options
                    .skip_default_lib_check
                    .is_true();

        let files = self.program.get_source_files();
        for file in files {
            // PORT NOTE: reshaped for borrowck; the current Rust Snapshot uses plain maps,
            // not Go's concurrent SyncMap fields, so mutating it through a WorkGroup would
            // require wider snapshot representation changes outside this file.
            {
                let version = self.snapshot.compute_hash(file.text());
                let implied_node_format = self
                    .program
                    .get_source_file_meta_data(file.path())
                    .implied_node_format;
                let binding_state = checker::Program::binding_state(self.program, &file);
                let affects_global_scope = file_affects_global_scope(&file, &binding_state);
                let mut signature = String::new();
                let new_references = get_referenced_files(self.program, &file);
                if let Some(new_references) = &new_references {
                    self.snapshot
                        .referenced_map
                        .store_references(file.path(), new_references.clone());
                }
                if let Some(old_program) = self.old_program {
                    if let Some(old_file_info) = old_program.snapshot.file_infos.get(&file.path()) {
                        signature = old_file_info.signature.clone();
                        if old_file_info.version != version
                            || old_file_info.affects_global_scope != affects_global_scope
                            || old_file_info.implied_node_format != implied_node_format
                        {
                            self.snapshot.add_file_to_change_set(file.path());
                        } else if new_references.clone().unwrap_or_default()
                            != old_program
                                .snapshot
                                .referenced_map
                                .get_references(file.path())
                        {
                            // Referenced files changed
                            self.snapshot.add_file_to_change_set(file.path());
                        } else if let Some(new_references) = &new_references {
                            for ref_path in new_references {
                                if self
                                    .program
                                    .get_source_file_by_path(ref_path.clone())
                                    .is_none()
                                    && old_program.snapshot.file_infos.contains_key(ref_path)
                                {
                                    // Referenced file was deleted in the new program
                                    self.snapshot.add_file_to_change_set(file.path());
                                    break;
                                }
                            }
                        }
                    } else {
                        self.snapshot.add_file_to_change_set(file.path());
                    }
                    if !self.snapshot.changed_files_set.contains(&file.path()) {
                        if let Some(emit_diagnostics) = old_program
                            .snapshot
                            .emit_diagnostics_per_file
                            .get(&file.path())
                        {
                            self.snapshot.emit_diagnostics_per_file.insert(
                                file.path(),
                                repopulate_diagnostics_of_file(
                                    emit_diagnostics,
                                    self.program,
                                    &file,
                                ),
                            );
                        }
                        if can_copy_semantic_diagnostics {
                            if (!file.is_declaration_file() || copy_declaration_file_diagnostics)
                                && (!self.program.is_source_file_default_library(file.path())
                                    || copy_lib_file_diagnostics)
                            {
                                // Unchanged file copy diagnostics
                                if let Some(diagnostics) = old_program
                                    .snapshot
                                    .semantic_diagnostics_per_file
                                    .get(&file.path())
                                {
                                    self.snapshot.semantic_diagnostics_per_file.insert(
                                        file.path(),
                                        repopulate_diagnostics_of_file(
                                            diagnostics,
                                            self.program,
                                            &file,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    if can_copy_emit_signatures {
                        if let Some(old_emit_signature) =
                            old_program.snapshot.emit_signatures.get(&file.path())
                        {
                            self.snapshot.emit_signatures.insert(
                                file.path(),
                                old_emit_signature.get_new_emit_signature(
                                    old_program.snapshot.options.clone(),
                                    self.snapshot.options.clone(),
                                ),
                            );
                        }
                    }
                } else {
                    self.snapshot.add_file_to_affected_files_pending_emit(
                        file.path(),
                        get_file_emit_kind(self.snapshot.options.clone()),
                    );
                    signature = version.clone();
                }
                self.snapshot.file_infos.insert(
                    file.path(),
                    FileInfo {
                        version,
                        signature,
                        affects_global_scope,
                        implied_node_format,
                    },
                );
            }
        }
    }

    pub fn handle_file_delete(&mut self) {
        if let Some(old_program) = self.old_program {
            // If the global file is removed, add all files as changed
            for (file_path, old_info) in &old_program.snapshot.file_infos {
                if !self.snapshot.file_infos.contains_key(file_path) {
                    if old_info.affects_global_scope {
                        for file in self
                            .snapshot
                            .get_all_files_excluding_default_library_file(self.program, None)
                        {
                            self.snapshot.add_file_to_change_set(file.path());
                        }
                        self.global_file_removed = true;
                    } else {
                        self.snapshot
                            .build_info_emit_pending
                            .store(true, Ordering::SeqCst);
                    }
                    break;
                }
            }
        }
    }

    pub fn handle_pending_emit(&mut self) {
        if let Some(old_program) = self.old_program {
            if !self.global_file_removed {
                // If options affect emit, then we need to do complete emit per compiler options
                // otherwise only the js or dts that needs to emitted because its different from previously emitted options
                let pending_emit_kind: FileEmitKind = if tsoptions::compiler_options_affect_emit(
                    old_program.snapshot.options.clone(),
                    self.snapshot.options.clone(),
                ) {
                    get_file_emit_kind(self.snapshot.options.clone())
                } else {
                    get_pending_emit_kind_with_options(
                        self.snapshot.options.clone(),
                        old_program.snapshot.options.clone(),
                    )
                };
                if pending_emit_kind != FILE_EMIT_KIND_NONE {
                    // Add all files to affectedFilesPendingEmit since emit changed
                    for file in self.program.get_source_files() {
                        // Add to affectedFilesPending emit only if not changed since any changed file will do full emit
                        if !self.snapshot.changed_files_set.contains(&file.path()) {
                            self.snapshot.add_file_to_affected_files_pending_emit(
                                file.path(),
                                pending_emit_kind,
                            );
                        }
                    }
                    self.snapshot
                        .build_info_emit_pending
                        .store(true, Ordering::SeqCst);
                }
            }
        }
    }

    pub fn handle_pending_check(&mut self) {
        if let Some(old_program) = self.old_program {
            if self.snapshot.semantic_diagnostics_per_file.len()
                != self.program.get_source_files().len()
                && old_program.snapshot.check_pending != self.snapshot.check_pending
            {
                self.snapshot
                    .build_info_emit_pending
                    .store(true, Ordering::SeqCst);
            }
        }
    }
}

pub fn file_affects_global_scope(
    file: &ast::SourceFile,
    binding_state: &binder::ProgramBindingState,
) -> bool {
    let store = file.store();
    // if file contains anything that augments to global scope we need to build them as if
    // they are global files as well as module
    if core::some(file.module_augmentations(), |augmentation: &ast::Node| {
        store
            .parent(*augmentation)
            .is_some_and(|parent| ast::is_global_scope_augmentation(store, parent))
    }) {
        return true;
    }

    if binding_state.external_module_indicator().is_some()
        || binding_state.common_js_module_indicator().is_some()
        || file.script_kind() == core::ScriptKind::JSON
    {
        return false;
    }

    // For script files that contains only ambient external modules, although they are not actually external module files,
    // they can only be consumed via importing elements from them. Regular script files cannot consume them. Therefore,
    // there are no point to rebuild all script files if these special files have changed. However, if any statement
    // in the file is not ambient external module, we treat it as a regular script file.
    let statements = file.statements_view();
    statements
        .iter()
        .any(|stmt| !ast::is_module_with_string_literal_name(store, stmt))
}

pub fn add_referenced_files_from_symbol(
    file: &ast::SourceFile,
    referenced_files: &mut HashSet<tspath::Path>,
    checker: &mut checker::Checker<'_, '_>,
    symbol: Option<ast::SymbolIdentity>,
) {
    let Some(symbol) = symbol else {
        return;
    };
    for declaration in checker.collect_symbol_declarations_public(symbol) {
        let Some(file_of_decl) = checker.try_source_file_for_node_public(declaration) else {
            continue;
        };
        if file.path() != file_of_decl.path() {
            referenced_files.insert(file_of_decl.path());
        }
    }
}

// Get the module source file and all augmenting files from the import name node from file
pub fn add_referenced_files_from_import_literal(
    file: &ast::SourceFile,
    referenced_files: &mut HashSet<tspath::Path>,
    checker: &mut checker::Checker<'_, '_>,
    import_name: &ast::Node,
) {
    let symbol = checker.get_symbol_at_location_public(*import_name);
    add_referenced_files_from_symbol(file, referenced_files, checker, symbol);
}

// Gets the path to reference file from file name, it could be resolvedPath if present otherwise path
pub fn add_referenced_file_from_file_name(
    program: &compiler::Program,
    file_name: &str,
    referenced_files: &mut HashSet<tspath::Path>,
    source_file_directory: &str,
) {
    let redirect = program.get_parse_file_redirect(file_name);
    if !redirect.is_empty() {
        referenced_files.insert(tspath::to_path(
            &redirect,
            &program.get_current_directory(),
            program.use_case_sensitive_file_names(),
        ));
    } else {
        referenced_files.insert(tspath::to_path(
            file_name,
            source_file_directory,
            program.use_case_sensitive_file_names(),
        ));
    }
}

// Gets the referenced files for a file from the program with values for the keys as referenced file's path to be true
pub fn get_referenced_files(
    program: &compiler::Program,
    file: &ast::SourceFile,
) -> Option<HashSet<tspath::Path>> {
    let mut referenced_files = HashSet::new();

    // We need to use a set here since the code can contain the same import twice,
    // but that will only be one dependency.
    // To avoid invernal conversion, the key of the referencedFiles map must be of type Path
    program.with_type_checker_for_file_exclusive(core::Context::todo(), file, |checker| {
        for import_name in file.imports() {
            add_referenced_files_from_import_literal(
                file,
                &mut referenced_files,
                checker,
                import_name,
            );
        }

        let source_file_directory = tspath::get_directory_path(&file.file_name());
        // Handle triple slash references
        for referenced_file in file.referenced_files() {
            add_referenced_file_from_file_name(
                program,
                &referenced_file.file_name,
                &mut referenced_files,
                &source_file_directory,
            );
        }

        // Handle type reference directives
        if let Some(type_refs_in_file) = program
            .get_resolved_type_reference_directives()
            .get(&file.path())
        {
            for type_ref in type_refs_in_file.values() {
                if !type_ref.resolved_file_name.is_empty() {
                    add_referenced_file_from_file_name(
                        program,
                        &type_ref.resolved_file_name,
                        &mut referenced_files,
                        &source_file_directory,
                    );
                }
            }
        }

        // Add module augmentation as references
        for module_name in file.module_augmentations() {
            if !ast::is_string_literal(file.store(), *module_name) {
                continue;
            }
            add_referenced_files_from_import_literal(
                file,
                &mut referenced_files,
                checker,
                module_name,
            );
        }

        // From ambient modules
        for ambient_module in checker.get_ambient_modules() {
            add_referenced_files_from_symbol(
                file,
                &mut referenced_files,
                checker,
                Some(ambient_module),
            );
        }
    });

    core::if_else(referenced_files.len() > 0, Some(referenced_files), None)
}

// repopulateDiagnosticsOfFile repopulates diagnostic chains that depend on program state.
// When diagnostics are copied from a previous build, their message chains may reference
// stale program state (e.g., resolved module alternate results, package.json scope).
// This function recomputes those chains using the current program's state.
pub fn repopulate_diagnostics_of_file(
    diags: &DiagnosticsOrBuildInfoDiagnosticsWithFileName,
    p: &compiler::Program,
    file: &ast::SourceFile,
) -> DiagnosticsOrBuildInfoDiagnosticsWithFileName {
    if !diags.diagnostics.is_empty() || diags.build_info_diagnostics.is_empty() {
        let repopulated = repopulate_diagnostics_list(&diags.diagnostics, p, file);
        if repopulated.is_none() {
            return diags.clone();
        }
        return DiagnosticsOrBuildInfoDiagnosticsWithFileName {
            diagnostics: repopulated.unwrap(),
            ..Default::default()
        };
    }
    // buildInfoDiagnostics will be repopulated via toDiagnostic's repopulateInfo handling
    diags.clone()
}

// repopulateDiagnosticsList repopulates diagnostic chains in a list of diagnostics.
// Returns nil if no diagnostics needed repopulation (i.e., no changes were made).
pub fn repopulate_diagnostics_list(
    diags: &[ast::Diagnostic],
    p: &compiler::Program,
    file: &ast::SourceFile,
) -> Option<Vec<ast::Diagnostic>> {
    let mut changed = false;
    let mut result = Vec::with_capacity(diags.len());
    for d in diags {
        let repopulated = repopulate_diagnostic_message_chain(d.message_chain(), p, file);
        if let Some(repopulated) = repopulated {
            let mut clone = d.clone();
            clone.set_message_chain(repopulated);
            result.push(clone);
            changed = true;
        } else {
            result.push(d.clone());
        }
    }
    if !changed {
        return None;
    }
    Some(result)
}

// repopulateDiagnosticMessageChain repopulates chains that have repopulate info.
// Returns nil if no changes were made.
pub fn repopulate_diagnostic_message_chain(
    chain: &[ast::Diagnostic],
    p: &compiler::Program,
    file: &ast::SourceFile,
) -> Option<Vec<ast::Diagnostic>> {
    if chain.is_empty() {
        return None;
    }
    let mut changed = false;
    let mut result = Vec::with_capacity(chain.len());
    for c in chain {
        if c.repopulate_info().is_some() {
            // Convert to buildInfoDiagnosticWithFileName and repopulate
            let mut b = super::BuildInfoDiagnosticWithFileName {
                pos: c.pos(),
                end: c.end(),
                code: c.code(),
                category: c.category(),
                message_key: c.message_key(),
                message_args: c.message_args().to_vec(),
                repopulate_info: c.repopulate_info().cloned(),
                ..Default::default()
            };
            // Recursively handle nested chains
            for nested in c.message_chain() {
                b.message_chain.push(ast_diag_to_build_info_diag(&nested));
            }
            result.push(repopulate_diagnostic_chain(&b, p, Some(file)));
            changed = true;
        } else {
            // Check nested chains
            let nested = repopulate_diagnostic_message_chain(c.message_chain(), p, file);
            if let Some(nested) = nested {
                let mut clone = c.clone();
                clone.set_message_chain(nested);
                result.push(clone);
                changed = true;
            } else {
                result.push(c.clone());
            }
        }
    }
    if !changed {
        return None;
    }
    Some(result)
}

pub fn ast_diag_to_build_info_diag(d: &ast::Diagnostic) -> super::BuildInfoDiagnosticWithFileName {
    let mut b = super::BuildInfoDiagnosticWithFileName {
        pos: d.pos(),
        end: d.end(),
        code: d.code(),
        category: d.category(),
        message_key: d.message_key(),
        message_args: d.message_args().to_vec(),
        repopulate_info: d.repopulate_info().cloned(),
        ..Default::default()
    };
    for nested in d.message_chain() {
        b.message_chain.push(ast_diag_to_build_info_diag(&nested));
    }
    b
}
