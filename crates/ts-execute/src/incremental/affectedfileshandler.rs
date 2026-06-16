use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Once};

use ts_ast as ast;
use ts_checker as checker;
use ts_compiler as compiler;
use ts_core as core;
use ts_tspath as tspath;

use super::{
    FILE_EMIT_KIND_ALL_DTS, FILE_EMIT_KIND_DTS, FileEmitKind, MaybeProgramExt as _, Program,
    SignatureUpdateKind, get_file_emit_kind, snapshot,
};

type DtsMayChange = HashMap<tspath::Path, FileEmitKind>;

fn add_file_to_affected_files_pending_emit(
    c: &mut DtsMayChange,
    file_path: tspath::Path,
    emit_kind: FileEmitKind,
) {
    c.insert(file_path, emit_kind);
}

pub struct UpdatedSignature {
    pub mu: Mutex<()>,
    pub signature: Mutex<String>,
    pub kind: AtomicU8,
}

pub struct AffectedFilesHandler<'a> {
    pub ctx: core::Context,
    pub program: &'a mut Program,
    pub has_all_files_excluding_default_library_file: AtomicBool,
    pub updated_signatures: Mutex<HashMap<tspath::Path, Arc<UpdatedSignature>>>,
    pub dts_may_change: Mutex<Vec<DtsMayChange>>,
    pub files_to_remove_diagnostics: Mutex<HashSet<tspath::Path>>,
    pub cleaned_diagnostics_of_lib_files: Once,
    pub seen_file_and_references: Mutex<HashMap<tspath::Path, bool>>,
}

impl AffectedFilesHandler<'_> {
    pub fn get_dts_may_change(
        &self,
        affected_file_path: tspath::Path,
        affected_file_emit_kind: FileEmitKind,
    ) -> DtsMayChange {
        DtsMayChange::from([(affected_file_path, affected_file_emit_kind)])
    }

    pub fn is_changed_signature(&self, path: tspath::Path) -> bool {
        let updated_signatures = self
            .updated_signatures
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let new_signature = updated_signatures.get(&path).unwrap();
        // This method is called after updating signatures of that path, so signature is present in updatedSignatures
        // And is already calculated, so no need to lock and unlock mutex on the entry
        let old_info = self.program.snapshot.file_infos.get(&path).unwrap();
        *new_signature
            .signature
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            != old_info.signature
    }

    pub fn remove_semantic_diagnostics_of(&self, path: tspath::Path) {
        self.files_to_remove_diagnostics
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(path);
    }

    pub fn remove_diagnostics_of_library_files(&mut self) {
        let mut library_files = Vec::new();
        self.cleaned_diagnostics_of_lib_files.call_once(|| {
            for file in self.program.get_source_files() {
                if self
                    .program
                    .program
                    .is_source_file_default_library(file.path())
                    && !self.program.program.skip_type_checking(&file, true)
                {
                    library_files.push(file);
                }
            }
        });
        for file in library_files {
            self.remove_semantic_diagnostics_of(file.path());
        }
    }

    pub fn compute_dts_signature(&mut self, file: &ast::SourceFile) -> String {
        let signature = Arc::new(Mutex::new(String::new()));
        let signature_result = Arc::clone(&signature);
        let hash_with_text = self.program.snapshot.hash_with_text;
        let file = file.share_readonly();
        self.program.program.emit(
            self.ctx.clone(),
            compiler::EmitOptions {
                target_source_file: Some(file.share_readonly()),
                emit_only: compiler::EMIT_ONLY_FORCED_DTS,
                write_file: Some(Rc::new(RefCell::new(
                    move |file_name: &str,
                          text: &str,
                          data: Option<&mut compiler::WriteFileData>| {
                        if !tspath::is_declaration_file_name(file_name) {
                            panic!(
                                "File extension for signature expected to be dts, got : {file_name}"
                            );
                        }
                        let default_data;
                        let data = match data {
                            Some(data) => data,
                            None => {
                                default_data = compiler::WriteFileData::default();
                                &default_data
                            }
                        };
                        let mut builder = String::new();
                        builder.push_str(&snapshot::get_text_handling_source_map_for_signature(
                            text, data,
                        ));
                        for diag in &data.diagnostics {
                            snapshot::diagnostic_to_string_builder(diag, &file, &mut builder);
                        }
                        *signature_result
                            .lock()
                            .unwrap_or_else(|err| err.into_inner()) =
                            snapshot::compute_hash(&builder, hash_with_text);
                        Ok(())
                    },
                ))),
                ..Default::default()
            },
        );
        signature
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn update_shape_signature(
        &mut self,
        file: &ast::SourceFile,
        use_file_version_as_signature: bool,
    ) -> bool {
        let update = Arc::new(UpdatedSignature {
            mu: Mutex::new(()),
            signature: Mutex::new(String::new()),
            kind: AtomicU8::new(0),
        });
        let _guard = update.mu.lock().unwrap_or_else(|err| err.into_inner());
        // If we have cached the result for this file, that means hence forth we should assume file shape is uptodate
        let mut updated_signatures = self
            .updated_signatures
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if let Some(existing) = updated_signatures.get(&file.path()).cloned() {
            // Ensure calculations for existing ones are complete before using the value
            let _existing_guard = existing.mu.lock().unwrap_or_else(|err| err.into_inner());
            return false;
        }
        updated_signatures.insert(file.path(), Arc::clone(&update));
        drop(updated_signatures);

        let info = self.program.snapshot.file_infos.get(&file.path()).unwrap();
        let prev_signature = info.signature.clone();
        let version = info.version.clone();
        if !file.is_declaration_file() && !use_file_version_as_signature {
            update.set_signature(self.compute_dts_signature(file));
        }
        // Default is to use file version as signature
        if update
            .signature
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .is_empty()
        {
            update.set_signature(version);
            update.set_kind(super::SIGNATURE_UPDATE_KIND_USED_VERSION);
        }
        *update
            .signature
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            != prev_signature
    }

    pub fn get_files_affected_by(&mut self, path: tspath::Path) -> Vec<ast::SourceFile> {
        let file = self.program.program.get_source_file_by_path(&path);
        if file.is_none() {
            return Vec::new();
        }
        let file = file.unwrap();

        if !self.update_shape_signature(&file, false) {
            return vec![file];
        }

        if self
            .program
            .snapshot
            .file_infos
            .get(&file.path())
            .is_some_and(|info| info.affects_global_scope)
        {
            self.has_all_files_excluding_default_library_file
                .store(true, Ordering::SeqCst);
            self.program
                .snapshot
                .get_all_files_excluding_default_library_file(
                    self.program.program.as_program(),
                    Some(file.share_readonly()),
                );
        }

        if self.program.snapshot.options.isolated_modules.is_true() {
            return vec![file];
        }

        // Now we need to if each file in the referencedBy list has a shape change as well.
        // Because if so, its own referencedBy files need to be saved as well to make the
        // emitting result consistent with files on disk.
        let mut seen_file_names_map = HashMap::new();
        seen_file_names_map.insert(file.path(), Some(file.share_readonly()));
        let mut queue: VecDeque<_> = self
            .program
            .snapshot
            .referenced_map
            .get_referenced_by(file.path())
            .into_iter()
            .collect();
        while let Some(current_path) = queue.pop_back() {
            if seen_file_names_map.contains_key(&current_path) {
                continue;
            }
            let current_file = self.program.program.get_source_file_by_path(&current_path);
            seen_file_names_map.insert(
                current_path.clone(),
                current_file.as_ref().map(ast::SourceFile::share_readonly),
            );
            let Some(current_file) = current_file else {
                continue;
            };
            if self.update_shape_signature(&current_file, false) {
                for r#ref in self
                    .program
                    .snapshot
                    .referenced_map
                    .get_referenced_by(current_file.path())
                {
                    queue.push_back(r#ref);
                }
            }
        }
        // Return array of values that needs emit
        seen_file_names_map.into_values().flatten().collect()
    }

    pub fn for_each_file_referenced_by(
        &self,
        file: &ast::SourceFile,
        mut fn_: impl FnMut(Option<ast::SourceFile>, tspath::Path) -> (bool, bool),
    ) -> HashMap<tspath::Path, Option<ast::SourceFile>> {
        // Now we need to if each file in the referencedBy list has a shape change as well.
        // Because if so, its own referencedBy files need to be saved as well to make the
        // emitting result consistent with files on disk.
        let mut seen_file_names_map = HashMap::new();
        // Start with the paths this file was referenced by
        seen_file_names_map.insert(file.path(), Some(file.share_readonly()));
        let mut queue: VecDeque<_> = self
            .program
            .snapshot
            .referenced_map
            .get_referenced_by(file.path())
            .into_iter()
            .collect();
        while let Some(current_path) = queue.pop_back() {
            if !seen_file_names_map.contains_key(&current_path) {
                let current_file = self.program.program.get_source_file_by_path(&current_path);
                seen_file_names_map.insert(
                    current_path.clone(),
                    current_file.as_ref().map(ast::SourceFile::share_readonly),
                );
                let (queue_for_file, fast_return) = fn_(
                    current_file.as_ref().map(ast::SourceFile::share_readonly),
                    current_path.clone(),
                );
                if fast_return {
                    return seen_file_names_map;
                }
                if queue_for_file {
                    let Some(current_file) = current_file else {
                        continue;
                    };
                    for r#ref in self
                        .program
                        .snapshot
                        .referenced_map
                        .get_referenced_by(current_file.path())
                    {
                        queue.push_back(r#ref);
                    }
                }
            }
        }
        seen_file_names_map
    }

    // Handles semantic diagnostics and dts emit for affectedFile and files, that are referencing modules that export entities from affected file
    // This is because even though js emit doesnt change, dts emit / type used can change resulting in need for dts emit and js change
    pub fn handle_dts_may_change_of_affected_file(
        &mut self,
        dts_may_change: &mut DtsMayChange,
        affected_file: &ast::SourceFile,
    ) {
        self.remove_semantic_diagnostics_of(affected_file.path());

        // If affected files is everything except default library, then nothing more to do
        if self
            .has_all_files_excluding_default_library_file
            .load(Ordering::SeqCst)
        {
            self.remove_diagnostics_of_library_files();
            // When a change affects the global scope, all files are considered to be affected without updating their signature
            // That means when affected file is handled, its signature can be out of date
            // To avoid this, ensure that we update the signature for any affected file in this scenario.
            self.update_shape_signature(affected_file, false);
            return;
        }

        if self
            .program
            .snapshot
            .options
            .assume_changes_only_affect_direct_dependencies
            .is_true()
        {
            return;
        }

        // Iterate on referencing modules that export entities from affected file and delete diagnostics and add pending emit
        // If there was change in signature (dts output) for the changed file,
        // then only we need to handle pending file emit
        if !self
            .program
            .snapshot
            .changed_files_set
            .contains(&affected_file.path())
            || !self.is_changed_signature(affected_file.path())
        {
            return;
        }

        // At this point affectedFile is actually one of the changed files
        // that has some change in its .d.ts signature.

        // Since isolated modules dont change js files, files affected by change in signature is itself
        // But we need to cleanup semantic diagnostics and queue dts emit for affected files
        if self.program.snapshot.options.isolated_modules.is_true() {
            let mut referenced_paths = Vec::new();
            self.for_each_file_referenced_by(affected_file, |_current_file, current_path| {
                referenced_paths.push(current_path);
                (false, false)
            });
            for current_path in referenced_paths {
                if self.handle_dts_may_change_of_global_scope(
                    dts_may_change,
                    current_path.clone(),
                    false,
                ) {
                    return;
                }
                self.handle_dts_may_change_of(dts_may_change, current_path.clone(), false);
                if self.is_changed_signature(current_path) {
                    continue;
                }
            }
        }

        let mut invalidate_js_files = false;
        // If exported const enum, we need to ensure that js files are emitted as well since the const enum value changed
        let binding_state =
            checker::Program::binding_state(self.program.program.as_program(), affected_file);
        if let Some(symbol) = binding_state.source_symbol() {
            let exported_symbols: Vec<_> = binding_state.with_symbol_exports(symbol, |exports| {
                exports
                    .map(|exports| exports.values().copied().collect())
                    .unwrap_or_default()
            });
            for exported in exported_symbols {
                let exported_identity = ast::SymbolIdentity::from_symbol_handle(exported);
                if binding_state.symbol_flags(exported) & ast::SYMBOL_FLAGS_CONST_ENUM != 0 {
                    invalidate_js_files = true;
                    break;
                }
                let aliased = self
                    .program
                    .program
                    .as_program()
                    .with_type_checker_for_file_exclusive(
                        self.ctx.clone(),
                        affected_file,
                        |checker| {
                            checker
                                .skip_alias_public(exported_identity)
                                .unwrap_or(exported_identity)
                        },
                    );
                if aliased == exported_identity {
                    continue;
                }
                let aliased_flags = self
                    .program
                    .program
                    .as_program()
                    .with_type_checker_for_file_exclusive(
                        self.ctx.clone(),
                        affected_file,
                        |checker| {
                            checker
                                .symbol_flags_public(aliased)
                                .unwrap_or(ast::SYMBOL_FLAGS_NONE)
                        },
                    );
                let aliased_declarations = self
                    .program
                    .program
                    .as_program()
                    .with_type_checker_for_file_exclusive(
                        self.ctx.clone(),
                        affected_file,
                        |checker| checker.collect_symbol_declarations_public(aliased),
                    );
                if (aliased_flags & ast::SYMBOL_FLAGS_CONST_ENUM) != 0
                    && aliased_declarations.iter().any(|d| {
                        d.store_id() == affected_file.store().store_id()
                            && ast::get_source_file_of_node(affected_file.store(), Some(*d))
                                .is_some_and(|source_file| {
                                    affected_file.store().as_source_file(source_file).path()
                                        == affected_file.path()
                                })
                    })
                {
                    invalidate_js_files = true;
                    break;
                }
            }
        }
        // Go through files that reference affected file and handle dts emit and semantic diagnostics for them and their references
        let files_referencing_changed: Vec<_> = self
            .program
            .snapshot
            .referenced_map
            .get_referenced_by(affected_file.path());
        for file_referencing_changed_file in files_referencing_changed {
            if self.handle_dts_may_change_of_global_scope(
                dts_may_change,
                file_referencing_changed_file.clone(),
                invalidate_js_files,
            ) {
                return;
            }
            // Since references of changed file = affected files - we would have already handled d.ts emit and semantic diagnostics
            // for those files. Now we need to handle files referencing those affected files to ensure correctness.
            let files_referencing_affected: Vec<_> = self
                .program
                .snapshot
                .referenced_map
                .get_referenced_by(file_referencing_changed_file.clone());
            for file_referencing_affected_file in files_referencing_affected {
                if self.handle_dts_may_change_of_file_and_references(
                    dts_may_change,
                    file_referencing_affected_file,
                    invalidate_js_files,
                ) {
                    return;
                }
            }
        }
    }

    pub fn handle_dts_may_change_of_file_and_references(
        &mut self,
        dts_may_change: &mut DtsMayChange,
        file_path: tspath::Path,
        invalidate_js_files: bool,
    ) -> bool {
        let mut seen = self
            .seen_file_and_references
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if let Some(existing) = seen.get(&file_path).copied() {
            if existing || !invalidate_js_files {
                return false;
            }
            if invalidate_js_files {
                seen.insert(file_path.clone(), true);
            }
        } else {
            seen.insert(file_path.clone(), invalidate_js_files);
        }
        drop(seen);

        if self.handle_dts_may_change_of_global_scope(
            dts_may_change,
            file_path.clone(),
            invalidate_js_files,
        ) {
            return true;
        }
        self.handle_dts_may_change_of(dts_may_change, file_path.clone(), invalidate_js_files);

        // Remove the diagnostics of files that import this file and
        // any files that are referenced by it (directly or indirectly)
        for referencing_file_path in self
            .program
            .snapshot
            .referenced_map
            .get_referenced_by(file_path)
        {
            if self.handle_dts_may_change_of_file_and_references(
                dts_may_change,
                referencing_file_path,
                invalidate_js_files,
            ) {
                return true;
            }
        }
        false
    }

    pub fn handle_dts_may_change_of_global_scope(
        &mut self,
        dts_may_change: &mut DtsMayChange,
        file_path: tspath::Path,
        invalidate_js_files: bool,
    ) -> bool {
        let Some(info) = self.program.snapshot.file_infos.get(&file_path) else {
            return false;
        };
        if !info.affects_global_scope {
            return false;
        }
        // Every file needs to be handled
        for file in self
            .program
            .snapshot
            .get_all_files_excluding_default_library_file(self.program.program.as_program(), None)
        {
            self.handle_dts_may_change_of(dts_may_change, file.path(), invalidate_js_files);
        }
        self.remove_diagnostics_of_library_files();
        true
    }

    // Handle the dts may change, so they need to be added to pending emit if dts emit is enabled,
    // Also we need to make sure signature is updated for these files
    pub fn handle_dts_may_change_of(
        &mut self,
        dts_may_change: &mut DtsMayChange,
        path: tspath::Path,
        invalidate_js_files: bool,
    ) {
        if self.program.snapshot.changed_files_set.contains(&path) {
            return;
        }
        let file = self.program.program.get_source_file_by_path(&path);
        if file.is_none() {
            return;
        }
        let file = file.unwrap();
        self.remove_semantic_diagnostics_of(path.clone());
        // Even though the js emit doesnt change and we are already handling dts emit and semantic diagnostics
        // we need to update the signature to reflect correctness of the signature(which is output d.ts emit) of this file
        // This ensures that we dont later during incremental builds considering wrong signature.
        // Eg where this also is needed to ensure that .tsbuildinfo generated by incremental build should be same as if it was first fresh build
        // But we avoid expensive full shape computation, as using file version as shape is enough for correctness.
        self.update_shape_signature(&file, true);
        // If not dts emit, nothing more to do
        if invalidate_js_files {
            add_file_to_affected_files_pending_emit(
                dts_may_change,
                path,
                get_file_emit_kind(self.program.snapshot.options.clone()),
            );
        } else if self.program.snapshot.options.get_emit_declarations() {
            add_file_to_affected_files_pending_emit(
                dts_may_change,
                path,
                core::if_else(
                    self.program.snapshot.options.declaration_map.is_true(),
                    FILE_EMIT_KIND_ALL_DTS,
                    FILE_EMIT_KIND_DTS,
                ),
            );
        }
    }

    pub fn update_snapshot(&mut self) {
        if self.ctx.err().is_some() {
            return;
        }
        for (file_path, update) in self
            .updated_signatures
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
        {
            if let Some(info) = self.program.snapshot.file_infos.get_mut(file_path) {
                info.signature = update
                    .signature
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone();
                if let Some(testing_data) = &mut self.program.testing_data {
                    testing_data
                        .updated_signature_kinds
                        .insert(file_path.clone(), update.kind.load(Ordering::SeqCst));
                }
            }
        }
        for file in self
            .files_to_remove_diagnostics
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
        {
            self.program
                .snapshot
                .semantic_diagnostics_per_file
                .remove(file);
            if let Some(testing_data) = self.program.testing_data.as_mut() {
                testing_data
                    .refreshed_semantic_diagnostics
                    .insert(file.clone());
            }
        }
        for change in self
            .dts_may_change
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
        {
            for (file_path, emit_kind) in change {
                self.program
                    .snapshot
                    .add_file_to_affected_files_pending_emit(file_path.clone(), *emit_kind);
            }
        }
        self.program.snapshot.changed_files_set.clear();
        self.program
            .snapshot
            .build_info_emit_pending
            .store(true, Ordering::SeqCst);
    }
}

impl UpdatedSignature {
    fn set_signature(&self, signature: String) {
        *self.signature.lock().unwrap_or_else(|err| err.into_inner()) = signature;
    }

    fn set_kind(&self, kind: SignatureUpdateKind) {
        self.kind.store(kind, Ordering::SeqCst);
    }
}

pub fn collect_all_affected_files(ctx: core::Context, program: &mut Program) {
    if program.snapshot.changed_files_set.is_empty() {
        return;
    }

    let changed_files: Vec<_> = program.snapshot.changed_files_set.iter().cloned().collect();
    let mut handler = AffectedFilesHandler {
        ctx: ctx.clone(),
        program,
        has_all_files_excluding_default_library_file: AtomicBool::new(false),
        updated_signatures: Mutex::new(HashMap::new()),
        dts_may_change: Mutex::new(Vec::new()),
        files_to_remove_diagnostics: Mutex::new(HashSet::new()),
        cleaned_diagnostics_of_lib_files: Once::new(),
        seen_file_and_references: Mutex::new(HashMap::new()),
    };
    let mut result = HashMap::new();
    for file in changed_files {
        // PORT NOTE: reshaped for borrowck/thread-safety; the current Rust AST
        // graph is not Send, so affected-file discovery runs inline while
        // preserving the TypeScript-Go iteration order.
        for affected_file in handler.get_files_affected_by(file) {
            result.insert(affected_file.path(), affected_file);
        }
    }

    if ctx.err().is_some() {
        return;
    }

    // For all the affected files, get all the files that would need to change their dts or js files,
    // update their diagnostics
    let emit_kind = get_file_emit_kind(handler.program.snapshot.options.clone());
    for file in result.values() {
        // remove the cached semantic diagnostics and handle dts emit and js emit if needed
        let mut dts_may_change = handler.get_dts_may_change(file.path(), emit_kind);
        let file = file.share_readonly();
        handler.handle_dts_may_change_of_affected_file(&mut dts_may_change, &file);
        handler
            .dts_may_change
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(dts_may_change);
    }

    // Update the snapshot with the new state
    handler.update_snapshot();
}
