use std::collections::HashMap;

use serde_json::Value;
use ts_ast as ast;
use ts_compiler as compiler;
use ts_core as core;
use ts_tsoptions::{self as tsoptions, CommandLineOptionKind};
use ts_tspath as tspath;

use super::{
    BuildInfo, BuildInfoDiagnostic, BuildInfoDiagnosticWithFileName, BuildInfoDiagnosticsOfFile,
    BuildInfoEmitSignature, BuildInfoFileId, BuildInfoFileIdListId, BuildInfoFilePendingEmit,
    BuildInfoReferenceMapEntry, BuildInfoRepopulateInfo, BuildInfoResolvedRoot, BuildInfoRoot,
    BuildInfoSemanticDiagnostic, DiagnosticsOrBuildInfoDiagnosticsWithFileName, Snapshot,
    get_file_emit_kind, new_build_info_file_info,
};

pub fn snapshot_to_build_info(
    snapshot: &Snapshot,
    program: &compiler::Program,
    build_info_file_name: &str,
) -> BuildInfo {
    let mut build_info = BuildInfo {
        version: core::version().to_owned(),
        ..Default::default()
    };
    let mut to = ToBuildInfo {
        snapshot,
        program,
        build_info: &mut build_info,
        build_info_directory: tspath::get_directory_path(build_info_file_name),
        compare_paths_options: tspath::ComparePathsOptions {
            current_directory: program.get_current_directory(),
            use_case_sensitive_file_names: program.use_case_sensitive_file_names(),
        },
        file_name_to_file_id: HashMap::new(),
        file_names_to_file_id_list_id: HashMap::new(),
        roots: HashMap::new(),
    };

    if snapshot.options.is_incremental() {
        to.collect_root_files();
        to.set_file_info_and_emit_signatures();
        to.set_root_of_incremental_program();
        to.set_compiler_options();
        to.set_referenced_map();
        to.set_change_file_set();
        to.set_semantic_diagnostics();
        to.set_emit_diagnostics();
        to.set_affected_files_pending_emit();
        if !snapshot.latest_changed_dts_file.is_empty() {
            to.build_info.latest_changed_dts_file =
                to.relative_to_build_info(&snapshot.latest_changed_dts_file);
        }
    } else {
        to.set_root_of_non_incremental_program();
    }
    to.build_info.errors = snapshot.has_errors.is_true();
    to.build_info.semantic_errors = snapshot.has_semantic_errors;
    to.build_info.check_pending = snapshot.check_pending;
    build_info
}

pub struct ToBuildInfo<'a> {
    pub snapshot: &'a Snapshot,
    pub program: &'a compiler::Program,
    pub build_info: &'a mut BuildInfo,
    pub build_info_directory: String,
    pub compare_paths_options: tspath::ComparePathsOptions,
    pub file_name_to_file_id: HashMap<String, BuildInfoFileId>,
    pub file_names_to_file_id_list_id: HashMap<String, BuildInfoFileIdListId>,
    pub roots: HashMap<tspath::Path, tspath::Path>,
}

impl<'a> ToBuildInfo<'a> {
    pub fn relative_to_build_info(&self, path: &str) -> String {
        let relative = tspath::get_relative_path_from_directory(
            &self.build_info_directory,
            path,
            &self.compare_paths_options,
        );
        tspath::ensure_path_is_non_module_name(&relative)
    }

    pub fn to_file_id(&mut self, path: tspath::Path) -> BuildInfoFileId {
        let mut file_id = *self.file_name_to_file_id.get(path.as_str()).unwrap_or(&0);
        if file_id == 0 {
            if let Some(lib_file) = self.program.get_default_lib_file(path.clone()) {
                if !lib_file.replaced {
                    self.build_info.file_names.push(lib_file.name);
                } else {
                    self.build_info
                        .file_names
                        .push(self.relative_to_build_info(&path));
                }
            } else {
                self.build_info
                    .file_names
                    .push(self.relative_to_build_info(&path));
            }
            file_id = self.build_info.file_names.len() as BuildInfoFileId;
            self.file_name_to_file_id.insert(path, file_id);
        }
        file_id
    }

    pub fn to_file_id_list_id(
        &mut self,
        set: &std::collections::HashSet<tspath::Path>,
    ) -> BuildInfoFileIdListId {
        let paths = set.iter().cloned().collect::<Vec<_>>();
        let mut file_ids = Vec::with_capacity(paths.len());
        for path in paths {
            file_ids.push(self.to_file_id(path));
        }
        file_ids.sort();
        let key = core::map(&file_ids, |id| format!("{id}")).join(",");

        let mut file_id_list_id = *self.file_names_to_file_id_list_id.get(&key).unwrap_or(&0);
        if file_id_list_id == 0 {
            self.build_info.file_ids_list.push(file_ids);
            file_id_list_id = self.build_info.file_ids_list.len() as BuildInfoFileIdListId;
            self.file_names_to_file_id_list_id
                .insert(key, file_id_list_id);
        }
        file_id_list_id
    }

    pub fn to_relative_to_build_info_compiler_option_value(
        &self,
        option: &tsoptions::CommandLineOption,
        v: Value,
    ) -> Value {
        if option.kind == Some(CommandLineOptionKind::List) {
            if option
                .elements()
                .is_some_and(|element| element.is_file_path)
            {
                if let Some(arr) = v.as_array() {
                    if !arr.iter().all(|value| value.is_string()) {
                        return v;
                    }
                    return Value::Array(
                        arr.iter()
                            .map(|value| value.as_str().unwrap())
                            .map(|value| Value::String(self.relative_to_build_info(value)))
                            .collect(),
                    );
                }
            }
        } else if option.is_file_path {
            if let Some(str_) = v.as_str() {
                if !str_.is_empty() {
                    return Value::String(self.relative_to_build_info(str_));
                }
            }
        }
        v
    }

    pub fn to_build_info_diagnostics_from_file_name_diagnostics(
        &mut self,
        diagnostics: Vec<BuildInfoDiagnosticWithFileName>,
    ) -> Vec<BuildInfoDiagnostic> {
        let mut result = Vec::with_capacity(diagnostics.len());
        for d in diagnostics {
            let mut file = 0;
            if !d.file.is_empty() {
                file = self.to_file_id(d.file.clone());
            }
            result.push(BuildInfoDiagnostic {
                file,
                no_file: d.no_file,
                pos: d.pos,
                end: d.end,
                code: d.code,
                category: d.category,
                message_key: d.message_key.clone(),
                message_args: d.message_args.clone(),
                message_chain: self
                    .to_build_info_diagnostics_from_file_name_diagnostics(d.message_chain.clone()),
                related_information: self.to_build_info_diagnostics_from_file_name_diagnostics(
                    d.related_information.clone(),
                ),
                reports_unnecessary: d.reports_unnecessary,
                reports_deprecated: d.reports_deprecated,
                skipped_on_no_emit: d.skipped_on_no_emit,
                repopulate_info: to_build_info_repopulate_info(d.repopulate_info.as_ref()),
            });
        }
        result
    }

    pub fn to_build_info_diagnostics_from_diagnostics(
        &mut self,
        file_path: tspath::Path,
        diagnostics: Vec<ast::Diagnostic>,
    ) -> Vec<BuildInfoDiagnostic> {
        let mut result = Vec::with_capacity(diagnostics.len());
        for d in diagnostics {
            let mut file = 0;
            let mut no_file = false;
            if let Some(diagnostic_file) = d.file() {
                if diagnostic_file.path() != &file_path {
                    file = self.to_file_id(diagnostic_file.path().clone());
                }
            } else {
                no_file = true;
            }
            result.push(BuildInfoDiagnostic {
                file,
                no_file,
                pos: d.loc().pos(),
                end: d.loc().end(),
                code: d.code(),
                category: d.category(),
                message_key: d.message_key(),
                message_args: d.message_args().to_vec(),
                message_chain: self.to_build_info_diagnostics_from_diagnostics(
                    file_path.clone(),
                    d.message_chain().to_vec(),
                ),
                related_information: self.to_build_info_diagnostics_from_diagnostics(
                    file_path.clone(),
                    d.related_information().to_vec(),
                ),
                reports_unnecessary: d.reports_unnecessary(),
                reports_deprecated: d.reports_deprecated(),
                skipped_on_no_emit: d.skipped_on_no_emit(),
                repopulate_info: to_build_info_repopulate_info(d.repopulate_info()),
            });
        }
        result
    }

    pub fn to_build_info_diagnostics_of_file(
        &mut self,
        file_path: tspath::Path,
        diags: DiagnosticsOrBuildInfoDiagnosticsWithFileName,
    ) -> Option<BuildInfoDiagnosticsOfFile> {
        if !diags.diagnostics.is_empty() {
            return Some(BuildInfoDiagnosticsOfFile {
                file_id: self.to_file_id(file_path.clone()),
                diagnostics: self
                    .to_build_info_diagnostics_from_diagnostics(file_path, diags.diagnostics),
            });
        }
        if !diags.build_info_diagnostics.is_empty() {
            return Some(BuildInfoDiagnosticsOfFile {
                file_id: self.to_file_id(file_path),
                diagnostics: self.to_build_info_diagnostics_from_file_name_diagnostics(
                    diags.build_info_diagnostics,
                ),
            });
        }
        None
    }

    pub fn collect_root_files(&mut self) {
        for file_name in self.program.command_line().file_names() {
            let redirect = self.program.get_parse_file_redirect(file_name);
            let file = if !redirect.is_empty() {
                self.program.get_source_file(&redirect)
            } else {
                self.program.get_source_file(file_name)
            };
            if let Some(file) = file {
                self.roots.insert(
                    file.path(),
                    tspath::to_path(
                        &file_name,
                        &self.compare_paths_options.current_directory,
                        self.compare_paths_options.use_case_sensitive_file_names,
                    ),
                );
            }
        }
    }

    pub fn set_file_info_and_emit_signatures(&mut self) {
        let source_files = self.program.get_source_files();
        self.build_info.file_infos.clear();
        self.build_info.file_infos_present = true;
        for file in &source_files {
            let info = self
                .snapshot
                .file_infos
                .get(&file.path())
                .cloned()
                .unwrap_or_else(|| {
                    panic!("file info must exist for every source file in snapshot")
                });
            let file_id = self.to_file_id(file.path());
            //  tryAddRoot(key, fileId);
            if self.build_info.file_names[(file_id - 1) as usize]
                != self.relative_to_build_info(&file.path())
            {
                let lib_file = self.program.get_default_lib_file(file.path());
                if lib_file.as_ref().is_none_or(|lib_file| {
                    lib_file.replaced
                        || self.build_info.file_names[(file_id - 1) as usize] != lib_file.name
                }) {
                    panic!(
                        "File name at index {} does not match expected relative path or libName: {} != {}",
                        file_id - 1,
                        self.build_info.file_names[(file_id - 1) as usize],
                        self.relative_to_build_info(&file.path())
                    );
                }
            }
            if self.snapshot.options.composite.is_true() {
                if !ast::is_json_source_file(&file)
                    && self.program.source_file_may_be_emitted(&file, false)
                {
                    if let Some(emit_signature) = self.snapshot.emit_signatures.get(&file.path()) {
                        if emit_signature.signature != info.signature {
                            let mut incremental_emit_signature = BuildInfoEmitSignature {
                                file_id,
                                ..Default::default()
                            };
                            if !emit_signature.signature.is_empty() {
                                incremental_emit_signature.signature =
                                    emit_signature.signature.clone();
                            } else if emit_signature.signature_with_different_options[0]
                                == info.signature
                            {
                                incremental_emit_signature.differs_only_in_dts_map = true;
                            } else {
                                incremental_emit_signature.signature =
                                    emit_signature.signature_with_different_options[0].clone();
                                incremental_emit_signature.differs_in_options = true;
                            }
                            self.build_info
                                .emit_signatures
                                .push(incremental_emit_signature);
                        }
                    } else {
                        self.build_info
                            .emit_signatures
                            .push(BuildInfoEmitSignature {
                                file_id,
                                ..Default::default()
                            });
                    }
                }
            }
            self.build_info.file_infos.push(new_build_info_file_info(
                &super::build_info::FileInfo {
                    version: info.version.clone(),
                    signature: info.signature.clone(),
                    affects_global_scope: info.affects_global_scope,
                    implied_node_format: info.implied_node_format,
                },
            ));
        }
    }

    pub fn set_root_of_incremental_program(&mut self) {
        let mut keys = self.roots.keys().cloned().collect::<Vec<_>>();
        keys.sort_by_key(|path| self.to_file_id(path.clone()));
        for file in keys {
            let root = self.to_file_id(self.roots.get(&file).cloned().unwrap());
            let resolved = self.to_file_id(file);
            if self.build_info.root.is_empty() {
                // First fileId as is
                self.build_info.root.push(BuildInfoRoot {
                    start: resolved,
                    ..Default::default()
                });
            } else {
                let last = self.build_info.root.last_mut().unwrap();
                if last.end == resolved - 1 {
                    // If its [..., last = [start, end = fileId - 1]], update last to [start, fileId]
                    last.end = resolved;
                } else if last.end == 0 && last.start == resolved - 1 {
                    // If its [..., last = start = fileId - 1 ], update last to [start, fileId]
                    last.end = resolved;
                } else {
                    self.build_info.root.push(BuildInfoRoot {
                        start: resolved,
                        ..Default::default()
                    });
                }
            }
            if root != resolved {
                self.build_info
                    .resolved_root
                    .push(BuildInfoResolvedRoot { resolved, root });
            }
        }
    }

    pub fn set_compiler_options(&mut self) {
        let Value::Object(options) =
            serde_json::to_value(&self.snapshot.options).unwrap_or(Value::Null)
        else {
            return;
        };
        for option in build_info_compiler_options() {
            let Some(value) = options.get(&option.name).cloned() else {
                continue;
            };
            if is_zero_compiler_option_value(&value) {
                continue;
            }
            // Make it relative to buildInfo directory if file path
            self.build_info.options.set(
                option.name.clone(),
                self.to_relative_to_build_info_compiler_option_value(&option, value),
            );
        }
    }

    pub fn set_referenced_map(&mut self) {
        let mut keys = self.snapshot.referenced_map.get_paths_with_references();
        keys.sort();
        let mut referenced_map = Vec::with_capacity(keys.len());
        for file_path in keys {
            let references = self
                .snapshot
                .referenced_map
                .get_references(file_path.clone());
            referenced_map.push(BuildInfoReferenceMapEntry {
                file_id: self.to_file_id(file_path.clone()),
                file_id_list_id: self.to_file_id_list_id(&references),
            });
        }
        self.build_info.referenced_map = referenced_map;
    }

    pub fn set_change_file_set(&mut self) {
        let mut files = self
            .snapshot
            .changed_files_set
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        files.sort();
        let mut change_file_set = Vec::with_capacity(files.len());
        for file in files {
            change_file_set.push(self.to_file_id(file));
        }
        self.build_info.change_file_set = change_file_set;
    }

    pub fn set_semantic_diagnostics(&mut self) {
        for file in self.program.get_source_files() {
            let file_path = file.path();
            let value = self
                .snapshot
                .semantic_diagnostics_per_file
                .get(file_path.as_str())
                .cloned();
            if value.is_none() {
                if !self.snapshot.changed_files_set.contains(file_path.as_str()) {
                    let file_id = self.to_file_id(file_path.clone());
                    self.build_info.semantic_diagnostics_per_file.push(
                        BuildInfoSemanticDiagnostic {
                            file_id,
                            diagnostics: None,
                        },
                    );
                }
            } else if let Some(diagnostics) =
                self.to_build_info_diagnostics_of_file(file_path, value.unwrap())
            {
                self.build_info
                    .semantic_diagnostics_per_file
                    .push(BuildInfoSemanticDiagnostic {
                        file_id: 0,
                        diagnostics: Some(diagnostics),
                    });
            }
        }
    }

    pub fn set_emit_diagnostics(&mut self) {
        let mut files = self
            .snapshot
            .emit_diagnostics_per_file
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        files.sort();
        let mut emit_diagnostics_per_file = Vec::with_capacity(files.len());
        for file_path in files {
            let value = self
                .snapshot
                .emit_diagnostics_per_file
                .get(file_path.as_str())
                .cloned()
                .unwrap_or_default();
            emit_diagnostics_per_file.push(
                self.to_build_info_diagnostics_of_file(file_path, value)
                    .unwrap_or_default(),
            );
        }
        self.build_info.emit_diagnostics_per_file = emit_diagnostics_per_file;
    }

    pub fn set_affected_files_pending_emit(&mut self) {
        let mut files = self
            .snapshot
            .affected_files_pending_emit
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        files.sort();
        let full_emit_kind = get_file_emit_kind(self.snapshot.options.clone());
        for file_path in files {
            let file = self.program.get_source_file_by_path(file_path.clone());
            if file
                .as_ref()
                .is_none_or(|file| !self.program.source_file_may_be_emitted(file, false))
            {
                continue;
            }
            let pending_emit = self
                .snapshot
                .affected_files_pending_emit
                .get(&file_path)
                .copied()
                .unwrap_or_default();
            let file_id = self.to_file_id(file_path);
            self.build_info
                .affected_files_pending_emit
                .push(BuildInfoFilePendingEmit {
                    file_id,
                    emit_kind: core::if_else(pending_emit == full_emit_kind, 0, pending_emit),
                });
        }
    }

    pub fn set_root_of_non_incremental_program(&mut self) {
        self.build_info.root = core::map(self.program.command_line().file_names(), |file_name| {
            BuildInfoRoot {
                non_incremental: self.relative_to_build_info(&tspath::to_path(
                    file_name,
                    &self.compare_paths_options.current_directory,
                    self.compare_paths_options.use_case_sensitive_file_names,
                )),
                ..Default::default()
            }
        });
    }
}

fn build_info_compiler_options() -> Vec<tsoptions::CommandLineOption> {
    // Keep core.CompilerOptions field order: Go's ForEachCompilerOptionValue
    // reflects fields in declaration order before filtering AffectsBuildInfo.
    build_info_compiler_option_names()
        .iter()
        .map(|name| {
            let mut option = tsoptions::options_declaration_for(name).unwrap_or_else(|| {
                tsoptions::CommandLineOption::new(*name, CommandLineOptionKind::String)
            });
            if matches!(
                *name,
                "declarationDir" | "outDir" | "rootDir" | "tsBuildInfoFile" | "outFile"
            ) {
                option.is_file_path = true;
            }
            option.affects_build_info = true;
            option
        })
        .collect()
}

fn build_info_compiler_option_names() -> &'static [&'static str] {
    &[
        "allowJs",
        "allowImportingTsExtensions",
        "allowUmdGlobalAccess",
        "allowUnreachableCode",
        "allowUnusedLabels",
        "assumeChangesOnlyAffectDirectDependencies",
        "checkJs",
        "composite",
        "emitDeclarationOnly",
        "emitBOM",
        "emitDecoratorMetadata",
        "declaration",
        "declarationDir",
        "declarationMap",
        "erasableSyntaxOnly",
        "exactOptionalPropertyTypes",
        "experimentalDecorators",
        "isolatedDeclarations",
        "importHelpers",
        "inlineSourceMap",
        "inlineSources",
        "jsx",
        "jsxImportSource",
        "mapRoot",
        "module",
        "newLine",
        "noErrorTruncation",
        "noFallthroughCasesInSwitch",
        "noImplicitAny",
        "noImplicitThis",
        "noImplicitReturns",
        "noEmitHelpers",
        "noPropertyAccessFromIndexSignature",
        "noUncheckedIndexedAccess",
        "noEmitOnError",
        "noUnusedLocals",
        "noUnusedParameters",
        "noImplicitOverride",
        "noUncheckedSideEffectImports",
        "outDir",
        "preserveConstEnums",
        "removeComments",
        "rewriteRelativeImportExtensions",
        "reactNamespace",
        "rootDir",
        "skipLibCheck",
        "stableTypeOrdering",
        "strict",
        "strictBindCallApply",
        "strictBuiltinIteratorReturn",
        "strictFunctionTypes",
        "strictNullChecks",
        "strictPropertyInitialization",
        "stripInternal",
        "skipDefaultLibCheck",
        "sourceMap",
        "sourceRoot",
        "target",
        "tsBuildInfoFile",
        "useDefineForClassFields",
        "useUnknownInCatchVariables",
        "verbatimModuleSyntax",
        "allowSyntheticDefaultImports",
        "alwaysStrict",
        "downlevelIteration",
        "esModuleInterop",
        "outFile",
    ]
}

pub fn to_build_info_repopulate_info(
    info: Option<&ast::RepopulateDiagnosticInfo>,
) -> Option<BuildInfoRepopulateInfo> {
    let info = info?;
    Some(BuildInfoRepopulateInfo {
        kind: info.kind,
        module_reference: info.module_reference.clone(),
        mode: info.mode,
        package_name: info.package_name.clone(),
    })
}

pub fn is_zero_compiler_option_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        // CompilerOptions booleans are serialized Tristate values; false is
        // TSFalse, not the Go zero value TSUnknown.
        Value::Bool(_) => false,
        Value::Number(value) => {
            value.as_i64().is_some_and(|value| value == 0)
                || value.as_u64().is_some_and(|value| value == 0)
                || value.as_f64().is_some_and(|value| value == 0.0)
        }
        Value::String(value) => value.is_empty(),
        Value::Array(_) | Value::Object(_) => false,
    }
}
