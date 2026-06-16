use serde::Serialize;
use serde::ser::SerializeSeq;
use serde_json::Value;
use ts_collections as collections;
use ts_diagnostics as diagnostics;
use ts_json as json;

use crate::incremental;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadableBuildInfo {
    #[serde(skip)]
    build_info: incremental::BuildInfo,
    #[serde(skip_serializing_if = "String::is_empty")]
    version: String,

    // Common between incremental and tsc -b buildinfo for non incremental programs
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    errors: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    check_pending: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    root: Vec<ReadableBuildInfoRoot>,

    // IncrementalProgram info
    #[serde(skip_serializing_if = "Vec::is_empty")]
    file_names: Vec<String>,
    #[serde(skip_serializing_if = "ReadableBuildInfoFileInfos::is_empty")]
    file_infos: ReadableBuildInfoFileInfos,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    file_ids_list: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<collections::OrderedMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    referenced_map: Option<collections::OrderedMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    semantic_diagnostics_per_file: Vec<ReadableBuildInfoSemanticDiagnostic>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    emit_diagnostics_per_file: Vec<ReadableBuildInfoDiagnosticsOfFile>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    change_file_set: Vec<String>, // List of changed files in the program, not the whole set of files
    #[serde(skip_serializing_if = "Vec::is_empty")]
    affected_files_pending_emit: Vec<Value>,
    #[serde(skip_serializing_if = "String::is_empty")]
    latest_changed_dts_file: String, // Because this is only output file in the program, we dont need fileId to deduplicate name
    #[serde(skip_serializing_if = "Vec::is_empty")]
    emit_signatures: Vec<ReadableBuildInfoEmitSignature>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    resolved_root: Vec<Value>,
    #[serde(skip_serializing_if = "incremental::is_default")]
    size: i32, // Size of the build info file

    // NonIncrementalProgram info
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    semantic_errors: bool,
}

#[derive(Clone, Default)]
struct ReadableBuildInfoFileInfos {
    present: bool,
    values: Vec<ReadableBuildInfoFileInfo>,
}

impl ReadableBuildInfoFileInfos {
    fn is_empty(&self) -> bool {
        !self.present && self.values.is_empty()
    }
}

impl Serialize for ReadableBuildInfoFileInfos {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.values.serialize(serializer)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadableBuildInfoRoot {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    files: Vec<String>,
    original: incremental::BuildInfoRoot,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadableBuildInfoFileInfo {
    #[serde(skip_serializing_if = "String::is_empty")]
    file_name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    version: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    signature: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    affects_global_scope: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    implied_node_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    original: Option<incremental::BuildInfoFileInfo>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadableBuildInfoDiagnostic {
    // incrementalBuildInfoFileId if it is for a File thats other than its stored for
    #[serde(skip_serializing_if = "String::is_empty")]
    file: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    no_file: bool,
    #[serde(skip_serializing_if = "incremental::is_default")]
    pos: i32,
    #[serde(skip_serializing_if = "incremental::is_default")]
    end: i32,
    #[serde(skip_serializing_if = "incremental::is_default")]
    code: i32,
    #[serde(skip_serializing_if = "incremental::is_default")]
    category: diagnostics::Category,
    #[serde(skip_serializing_if = "String::is_empty")]
    message_key: diagnostics::Key,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    message_args: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    message_chain: Vec<ReadableBuildInfoDiagnostic>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    related_information: Vec<ReadableBuildInfoDiagnostic>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    reports_unnecessary: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    reports_deprecated: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    skipped_on_no_emit: bool,
}

#[derive(Clone, Default)]
struct ReadableBuildInfoDiagnosticsOfFile {
    file: String,
    diagnostics: Vec<ReadableBuildInfoDiagnostic>,
}

impl Serialize for ReadableBuildInfoDiagnosticsOfFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.file)?;
        seq.serialize_element(&self.diagnostics)?;
        seq.end()
    }
}

#[derive(Clone, Default)]
struct ReadableBuildInfoSemanticDiagnostic {
    file: String, // File is not in changedSet and still doesnt have cached diagnostics
    diagnostics: Option<ReadableBuildInfoDiagnosticsOfFile>, // Diagnostics for file
}

impl Serialize for ReadableBuildInfoSemanticDiagnostic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if !self.file.is_empty() {
            return serializer.serialize_str(&self.file);
        }
        if let Some(diagnostics) = &self.diagnostics {
            return diagnostics.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

#[derive(Clone, Default)]
struct ReadableBuildInfoFilePendingEmit {
    file: String,
    emit_kind: String,
    original: incremental::BuildInfoFilePendingEmit,
}

impl ReadableBuildInfoFilePendingEmit {
    fn marshal_json(&self) -> Value {
        Value::Array(vec![
            Value::String(self.file.clone()),
            Value::String(self.emit_kind.clone()),
            serde_json::to_value(&self.original).unwrap_or(Value::Null),
        ])
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadableBuildInfoEmitSignature {
    #[serde(skip_serializing_if = "String::is_empty")]
    file: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    signature: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    differs_only_in_dts_map: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    differs_in_options: bool,
    original: incremental::BuildInfoEmitSignature,
}

#[derive(Clone, Default)]
struct ReadableBuildInfoResolvedRoot {
    resolved: String,
    root: String,
}

impl ReadableBuildInfoResolvedRoot {
    fn marshal_json(&self) -> Value {
        Value::Array(vec![
            Value::String(self.resolved.clone()),
            Value::String(self.root.clone()),
        ])
    }
}

pub fn to_readable_build_info(
    build_info: &incremental::BuildInfo,
    build_info_text: String,
) -> String {
    let mut readable = ReadableBuildInfo {
        build_info: build_info.clone(),
        version: build_info.version.clone(),
        errors: build_info.errors,
        check_pending: build_info.check_pending,
        root: Vec::new(),
        file_names: build_info.file_names.clone(),
        file_infos: ReadableBuildInfoFileInfos {
            present: build_info.file_infos_present,
            values: Vec::new(),
        },
        file_ids_list: Vec::new(),
        options: if build_info.options.size() == 0 {
            None
        } else {
            Some(build_info.options.clone())
        },
        referenced_map: None,
        semantic_diagnostics_per_file: Vec::new(),
        emit_diagnostics_per_file: Vec::new(),
        change_file_set: Vec::new(),
        affected_files_pending_emit: Vec::new(),
        latest_changed_dts_file: build_info.latest_changed_dts_file.clone(),
        emit_signatures: Vec::new(),
        resolved_root: Vec::new(),
        size: build_info_text.len() as i32,
        semantic_errors: build_info.semantic_errors,
    };
    readable.set_file_infos();
    readable.set_root();
    readable.set_file_ids_list();
    readable.set_referenced_map();
    readable.set_change_file_set();
    readable.set_semantic_diagnostics();
    readable.set_emit_diagnostics();
    readable.set_affected_files_pending_emit();
    readable.set_emit_signatures();
    readable.set_resolved_root();

    let contents = json::marshal_indent(&readable, "", "  ");
    match contents {
        Ok(contents) => String::from_utf8(contents).unwrap_or_else(|err| {
            panic!("readableBuildInfo: failed to decode readable build info: {err}")
        }),
        Err(err) => panic!("readableBuildInfo: failed to marshal readable build info: {err}"),
    }
}

impl ReadableBuildInfo {
    fn to_file_path(&self, file_id: incremental::BuildInfoFileId) -> String {
        self.build_info.file_names[file_id as usize - 1].clone()
    }

    fn to_file_path_set(&self, file_id_list_id: incremental::BuildInfoFileIdListId) -> Vec<String> {
        self.file_ids_list[file_id_list_id as usize - 1].clone()
    }

    fn to_readable_build_info_diagnostic(
        &self,
        diagnostics: &[incremental::BuildInfoDiagnostic],
    ) -> Vec<ReadableBuildInfoDiagnostic> {
        diagnostics
            .iter()
            .map(|d| {
                let file = if d.file != 0 {
                    self.to_file_path(d.file)
                } else {
                    String::new()
                };
                ReadableBuildInfoDiagnostic {
                    file,
                    no_file: d.no_file,
                    pos: d.pos,
                    end: d.end,
                    code: d.code,
                    category: d.category,
                    message_key: d.message_key.clone(),
                    message_args: d.message_args.clone(),
                    message_chain: self.to_readable_build_info_diagnostic(&d.message_chain),
                    related_information: self
                        .to_readable_build_info_diagnostic(&d.related_information),
                    reports_unnecessary: d.reports_unnecessary,
                    reports_deprecated: d.reports_deprecated,
                    skipped_on_no_emit: d.skipped_on_no_emit,
                }
            })
            .collect()
    }

    fn to_readable_build_info_diagnostics_of_file(
        &self,
        diagnostics: &incremental::BuildInfoDiagnosticsOfFile,
    ) -> ReadableBuildInfoDiagnosticsOfFile {
        ReadableBuildInfoDiagnosticsOfFile {
            file: self.to_file_path(diagnostics.file_id),
            diagnostics: self.to_readable_build_info_diagnostic(&diagnostics.diagnostics),
        }
    }

    fn set_file_infos(&mut self) {
        self.file_infos.values = self
            .build_info
            .file_infos
            .iter()
            .enumerate()
            .map(|(index, original)| {
                let file_info = original
                    .get_file_info()
                    .expect("build info file info should resolve");
                let original = if original.has_signature() {
                    None
                } else {
                    Some(original.clone())
                };
                ReadableBuildInfoFileInfo {
                    file_name: self.to_file_path(index as i32 + 1),
                    version: file_info.version,
                    signature: file_info.signature,
                    affects_global_scope: file_info.affects_global_scope,
                    implied_node_format: file_info.implied_node_format.to_string(),
                    original,
                }
            })
            .collect();
    }

    fn set_root(&mut self) {
        self.root = self
            .build_info
            .root
            .iter()
            .map(|original| {
                let files = if !original.non_incremental.is_empty() {
                    vec![original.non_incremental.clone()]
                } else if original.end == 0 {
                    vec![self.to_file_path(original.start)]
                } else {
                    let mut files =
                        Vec::with_capacity((original.end - original.start + 1) as usize);
                    for i in original.start..=original.end {
                        files.push(self.to_file_path(i));
                    }
                    files
                };
                ReadableBuildInfoRoot {
                    files,
                    original: original.clone(),
                }
            })
            .collect();
    }

    fn set_file_ids_list(&mut self) {
        self.file_ids_list = self
            .build_info
            .file_ids_list
            .iter()
            .map(|ids| ids.iter().map(|id| self.to_file_path(*id)).collect())
            .collect();
    }

    fn set_referenced_map(&mut self) {
        if !self.build_info.referenced_map.is_empty() {
            let mut referenced_map = collections::OrderedMap::<String, Vec<String>>::default();
            for entry in &self.build_info.referenced_map {
                referenced_map.set(
                    self.to_file_path(entry.file_id),
                    self.to_file_path_set(entry.file_id_list_id),
                );
            }
            self.referenced_map = Some(referenced_map);
        }
    }

    fn set_change_file_set(&mut self) {
        self.change_file_set = self
            .build_info
            .change_file_set
            .iter()
            .map(|file_id| self.to_file_path(*file_id))
            .collect();
    }

    fn set_semantic_diagnostics(&mut self) {
        self.semantic_diagnostics_per_file = self
            .build_info
            .semantic_diagnostics_per_file
            .iter()
            .map(|diagnostics| {
                if diagnostics.file_id != 0 {
                    ReadableBuildInfoSemanticDiagnostic {
                        file: self.to_file_path(diagnostics.file_id),
                        diagnostics: None,
                    }
                } else {
                    ReadableBuildInfoSemanticDiagnostic {
                        file: String::new(),
                        diagnostics: diagnostics
                            .diagnostics
                            .as_ref()
                            .map(|d| self.to_readable_build_info_diagnostics_of_file(d)),
                    }
                }
            })
            .collect();
    }

    fn set_emit_diagnostics(&mut self) {
        self.emit_diagnostics_per_file = self
            .build_info
            .emit_diagnostics_per_file
            .iter()
            .map(|diagnostics| self.to_readable_build_info_diagnostics_of_file(diagnostics))
            .collect();
    }

    fn set_affected_files_pending_emit(&mut self) {
        if self.build_info.affected_files_pending_emit.is_empty() {
            return;
        }
        let full_emit_kind =
            incremental::get_file_emit_kind(self.build_info.get_compiler_options(""));
        self.affected_files_pending_emit = self
            .build_info
            .affected_files_pending_emit
            .iter()
            .map(|pending_emit| {
                let emit_kind = if pending_emit.emit_kind == 0 {
                    full_emit_kind
                } else {
                    pending_emit.emit_kind
                };
                ReadableBuildInfoFilePendingEmit {
                    file: self.to_file_path(pending_emit.file_id),
                    emit_kind: to_readable_file_emit_kind(emit_kind),
                    original: pending_emit.clone(),
                }
                .marshal_json()
            })
            .collect();
    }

    fn set_emit_signatures(&mut self) {
        self.emit_signatures = self
            .build_info
            .emit_signatures
            .iter()
            .map(|signature| ReadableBuildInfoEmitSignature {
                file: self.to_file_path(signature.file_id),
                signature: signature.signature.clone(),
                differs_only_in_dts_map: signature.differs_only_in_dts_map,
                differs_in_options: signature.differs_in_options,
                original: signature.clone(),
            })
            .collect();
    }

    fn set_resolved_root(&mut self) {
        self.resolved_root = self
            .build_info
            .resolved_root
            .iter()
            .map(|original| {
                ReadableBuildInfoResolvedRoot {
                    resolved: self.to_file_path(original.resolved),
                    root: self.to_file_path(original.root),
                }
                .marshal_json()
            })
            .collect();
    }
}

pub fn to_readable_file_emit_kind(file_emit_kind: incremental::FileEmitKind) -> String {
    let mut flags = Vec::new();
    if file_emit_kind != 0 {
        if (file_emit_kind & incremental::FILE_EMIT_KIND_JS) != 0 {
            flags.push("Js");
        }
        if (file_emit_kind & incremental::FILE_EMIT_KIND_JS_MAP) != 0 {
            flags.push("JsMap");
        }
        if (file_emit_kind & incremental::FILE_EMIT_KIND_JS_INLINE_MAP) != 0 {
            flags.push("JsInlineMap");
        }
        if (file_emit_kind & incremental::FILE_EMIT_KIND_DTS) == incremental::FILE_EMIT_KIND_DTS {
            flags.push("Dts");
        } else {
            if (file_emit_kind & incremental::FILE_EMIT_KIND_DTS_EMIT) != 0 {
                flags.push("DtsEmit");
            }
            if (file_emit_kind & incremental::FILE_EMIT_KIND_DTS_ERRORS) != 0 {
                flags.push("DtsErrors");
            }
        }
        if (file_emit_kind & incremental::FILE_EMIT_KIND_DTS_MAP) != 0 {
            flags.push("DtsMap");
        }
    }
    if !flags.is_empty() {
        return flags.join("|");
    }
    "None".to_owned()
}
