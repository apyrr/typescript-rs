use std::collections::HashSet;

use ts_ast as ast;
use ts_compiler as compiler;
use ts_core as core;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use super::snapshot::{
    BuildInfoDiagnosticWithFileName, DiagnosticsOrBuildInfoDiagnosticsWithFileName, EmitSignature,
    Snapshot, get_file_emit_kind,
};
use super::{
    BuildInfo, BuildInfoDiagnostic, BuildInfoDiagnosticsOfFile, BuildInfoFileId,
    BuildInfoFileIdListId, BuildInfoRepopulateInfo,
};

pub fn build_info_to_snapshot(
    build_info: &BuildInfo,
    config: &tsoptions::ParsedCommandLine,
    host: &dyn compiler::CompilerHost,
) -> Snapshot {
    let mut to = ToSnapshot {
        build_info,
        build_info_directory: tspath::get_directory_path(&tspath::get_normalized_absolute_path(
            &config.get_build_info_file_name(),
            &config.get_current_directory(),
        )),
        snapshot: Snapshot::default(),
        file_paths: Vec::with_capacity(build_info.file_names.len()),
        file_path_set: Vec::with_capacity(build_info.file_ids_list.len()),
    };
    to.file_paths = core::map(&build_info.file_names, |file_name| {
        if !file_name.starts_with('.') {
            return tspath::to_path(
                &tspath::combine_paths(&host.default_library_path(), &[file_name]),
                &host.get_current_directory(),
                host.fs().use_case_sensitive_file_names(),
            );
        }
        tspath::to_path(
            file_name,
            &to.build_info_directory,
            config.use_case_sensitive_file_names(),
        )
    });
    to.file_path_set = core::map(&build_info.file_ids_list, |file_id_list| {
        let mut file_set = HashSet::with_capacity(file_id_list.len());
        for file_id in file_id_list {
            file_set.insert(to.to_file_path(*file_id));
        }
        file_set
    });
    to.set_compiler_options();
    to.set_file_info_and_emit_signatures();
    to.set_referenced_map();
    to.set_change_file_set();
    to.set_semantic_diagnostics();
    to.set_emit_diagnostics();
    to.set_affected_files_pending_emit();
    if !build_info.latest_changed_dts_file.is_empty() {
        to.snapshot.latest_changed_dts_file =
            to.to_absolute_path(&build_info.latest_changed_dts_file);
    }
    to.snapshot.has_errors = core::if_else(build_info.errors, core::TS_TRUE, core::TS_FALSE);
    to.snapshot.has_semantic_errors = build_info.semantic_errors;
    to.snapshot.check_pending = build_info.check_pending;
    to.snapshot
}

pub struct ToSnapshot<'a> {
    pub build_info: &'a BuildInfo,
    pub build_info_directory: String,
    pub snapshot: Snapshot,
    pub file_paths: Vec<tspath::Path>,
    pub file_path_set: Vec<HashSet<tspath::Path>>,
}

impl<'a> ToSnapshot<'a> {
    pub fn to_absolute_path(&self, path: &str) -> String {
        tspath::get_normalized_absolute_path(path, &self.build_info_directory)
    }

    pub fn to_file_path(&self, file_id: BuildInfoFileId) -> tspath::Path {
        self.file_paths[(file_id - 1) as usize].clone()
    }

    pub fn to_file_path_set(
        &self,
        file_id_list_id: BuildInfoFileIdListId,
    ) -> HashSet<tspath::Path> {
        self.file_path_set[(file_id_list_id - 1) as usize].clone()
    }

    pub fn to_build_info_diagnostics_with_file_name(
        &self,
        diagnostics: Vec<BuildInfoDiagnostic>,
    ) -> Vec<BuildInfoDiagnosticWithFileName> {
        core::map(&diagnostics, |d| {
            let mut file = String::new();
            if d.file != 0 {
                file = self.to_file_path(d.file);
            }
            BuildInfoDiagnosticWithFileName {
                file,
                no_file: d.no_file,
                pos: d.pos,
                end: d.end,
                code: d.code,
                category: d.category,
                message_key: d.message_key.clone(),
                message_args: d.message_args.clone(),
                message_chain: self
                    .to_build_info_diagnostics_with_file_name(d.message_chain.clone()),
                related_information: self
                    .to_build_info_diagnostics_with_file_name(d.related_information.clone()),
                reports_unnecessary: d.reports_unnecessary,
                reports_deprecated: d.reports_deprecated,
                skipped_on_no_emit: d.skipped_on_no_emit,
                repopulate_info: from_build_info_repopulate_info(d.repopulate_info.as_ref()),
            }
        })
    }

    pub fn to_diagnostics_or_build_info_diagnostics_with_file_name(
        &self,
        dig: BuildInfoDiagnosticsOfFile,
    ) -> DiagnosticsOrBuildInfoDiagnosticsWithFileName {
        DiagnosticsOrBuildInfoDiagnosticsWithFileName {
            diagnostics: Vec::new(),
            build_info_diagnostics: self.to_build_info_diagnostics_with_file_name(dig.diagnostics),
        }
    }
}

pub fn from_build_info_repopulate_info(
    info: Option<&BuildInfoRepopulateInfo>,
) -> Option<ast::RepopulateDiagnosticInfo> {
    let info = info?;
    Some(ast::RepopulateDiagnosticInfo {
        kind: info.kind,
        module_reference: info.module_reference.clone(),
        mode: info.mode,
        package_name: info.package_name.clone(),
    })
}

impl<'a> ToSnapshot<'a> {
    pub fn set_compiler_options(&mut self) {
        self.snapshot.options = self
            .build_info
            .get_compiler_options(&self.build_info_directory);
    }

    pub fn set_file_info_and_emit_signatures(&mut self) {
        let is_composite = self.snapshot.options.composite.is_true();
        for (index, build_info_file_info) in self.build_info.file_infos.iter().enumerate() {
            let path = self.to_file_path((index + 1) as BuildInfoFileId);
            let build_info_file_info = build_info_file_info.get_file_info().unwrap_or_default();
            let info = super::snapshot::FileInfo {
                version: build_info_file_info.version,
                signature: build_info_file_info.signature,
                affects_global_scope: build_info_file_info.affects_global_scope,
                implied_node_format: build_info_file_info.implied_node_format,
            };
            self.snapshot.file_infos.insert(path.clone(), info.clone());
            // Add default emit signature as file's signature
            if !info.signature.is_empty() && is_composite {
                self.snapshot.emit_signatures.insert(
                    path,
                    EmitSignature {
                        signature: info.signature,
                        signature_with_different_options: Vec::new(),
                    },
                );
            }
        }
        // Fix up emit signatures
        for value in &self.build_info.emit_signatures {
            if value.no_emit_signature() {
                self.snapshot
                    .emit_signatures
                    .remove(&self.to_file_path(value.file_id));
            } else {
                let path = self.to_file_path(value.file_id);
                self.snapshot.emit_signatures.insert(
                    path.clone(),
                    value.to_emit_signature(path, &self.snapshot.emit_signatures),
                );
            }
        }
    }

    pub fn set_referenced_map(&mut self) {
        for entry in &self.build_info.referenced_map {
            self.snapshot.referenced_map.store_references(
                self.to_file_path(entry.file_id),
                self.to_file_path_set(entry.file_id_list_id),
            );
        }
    }

    pub fn set_change_file_set(&mut self) {
        for file_id in &self.build_info.change_file_set {
            let file_path = self.to_file_path(*file_id);
            self.snapshot.changed_files_set.insert(file_path);
        }
    }

    pub fn set_semantic_diagnostics(&mut self) {
        for (path, _) in self.snapshot.file_infos.clone() {
            // Initialize to have no diagnostics if its not changed file
            if !self.snapshot.changed_files_set.contains(&path) {
                self.snapshot.semantic_diagnostics_per_file.insert(
                    path,
                    DiagnosticsOrBuildInfoDiagnosticsWithFileName::default(),
                );
            }
        }
        for diagnostic in &self.build_info.semantic_diagnostics_per_file {
            if diagnostic.file_id != 0 {
                let file_path = self.to_file_path(diagnostic.file_id);
                self.snapshot
                    .semantic_diagnostics_per_file
                    .remove(&file_path); // does not have cached diagnostics
            } else {
                let diagnostics = diagnostic
                    .diagnostics
                    .as_ref()
                    .expect("BuildInfoSemanticDiagnostic with file_id 0 must include diagnostics");
                let file_path = self.to_file_path(diagnostics.file_id);
                self.snapshot.semantic_diagnostics_per_file.insert(
                    file_path,
                    self.to_diagnostics_or_build_info_diagnostics_with_file_name(
                        diagnostics.clone(),
                    ),
                );
            }
        }
    }

    pub fn set_emit_diagnostics(&mut self) {
        for diagnostic in &self.build_info.emit_diagnostics_per_file {
            let file_path = self.to_file_path(diagnostic.file_id);
            self.snapshot.emit_diagnostics_per_file.insert(
                file_path,
                self.to_diagnostics_or_build_info_diagnostics_with_file_name(diagnostic.clone()),
            );
        }
    }

    pub fn set_affected_files_pending_emit(&mut self) {
        if self.build_info.affected_files_pending_emit.is_empty() {
            return;
        }
        let own_options_emit_kind = get_file_emit_kind(self.snapshot.options.clone());
        for pending_emit in &self.build_info.affected_files_pending_emit {
            self.snapshot.affected_files_pending_emit.insert(
                self.to_file_path(pending_emit.file_id),
                core::if_else(
                    pending_emit.emit_kind == 0,
                    own_options_emit_kind,
                    pending_emit.emit_kind,
                ),
            );
        }
    }
}
