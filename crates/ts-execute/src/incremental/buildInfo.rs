use std::collections::HashMap;

use serde::de::Error as DeError;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use ts_ast as ast;
use ts_collections::OrderedMap;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use super::snapshot::{
    self, EmitSignature, FILE_EMIT_KIND_DTS, FILE_EMIT_KIND_DTS_ERRORS, FileEmitKind,
};

pub use super::snapshot::FileInfo;

pub type BuildInfoFileId = i32;
pub type BuildInfoFileIdListId = i32;

// buildInfoRoot is
// - for incremental program buildinfo
//   - start and end of FileId for consecutive fileIds to be included as root
//   - start - single fileId that is root
//
// - for non incremental program buildinfo
//   - string that is the root file name
#[derive(Clone, Default)]
pub struct BuildInfoRoot {
    pub start: BuildInfoFileId,
    pub end: BuildInfoFileId,
    pub non_incremental: String, // Root of a non incremental program
}

impl BuildInfoRoot {
    pub fn marshal_json(&self) -> serde_json::Result<Vec<u8>> {
        if self.start != 0 {
            if self.end != 0 {
                serde_json::to_vec(&[self.start, self.end])
            } else {
                serde_json::to_vec(&self.start)
            }
        } else {
            serde_json::to_vec(&self.non_incremental)
        }
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        if let Ok(start_and_end) = serde_json::from_slice::<[i32; 2]>(data) {
            *self = BuildInfoRoot {
                start: start_and_end[0],
                end: start_and_end[1],
                non_incremental: String::new(),
            };
            return Ok(());
        }
        if let Ok(start) = serde_json::from_slice::<i32>(data) {
            *self = BuildInfoRoot {
                start,
                end: 0,
                non_incremental: String::new(),
            };
            return Ok(());
        }
        if let Ok(name) = serde_json::from_slice::<String>(data) {
            *self = BuildInfoRoot {
                start: 0,
                end: 0,
                non_incremental: name,
            };
            return Ok(());
        }
        Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid BuildInfoRoot: {}", String::from_utf8_lossy(data)),
        )))
    }
}

impl Serialize for BuildInfoRoot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.start != 0 {
            if self.end != 0 {
                [self.start, self.end].serialize(serializer)
            } else {
                self.start.serialize(serializer)
            }
        } else {
            self.non_incremental.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for BuildInfoRoot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if let Ok(start_and_end) = serde_json::from_value::<[i32; 2]>(value.clone()) {
            return Ok(BuildInfoRoot {
                start: start_and_end[0],
                end: start_and_end[1],
                non_incremental: String::new(),
            });
        }
        if let Ok(start) = serde_json::from_value::<i32>(value.clone()) {
            return Ok(BuildInfoRoot {
                start,
                end: 0,
                non_incremental: String::new(),
            });
        }
        if let Ok(name) = serde_json::from_value::<String>(value.clone()) {
            return Ok(BuildInfoRoot {
                start: 0,
                end: 0,
                non_incremental: name,
            });
        }
        Err(D::Error::custom(format!("invalid BuildInfoRoot: {value}")))
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct BuildInfoFileInfoNoSignature {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(
        default,
        rename = "noSignature",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub no_signature: bool,
    #[serde(
        default,
        rename = "affectsGlobalScope",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub affects_global_scope: bool,
    #[serde(
        default,
        rename = "impliedNodeFormat",
        skip_serializing_if = "is_none_resolution_mode"
    )]
    pub implied_node_format: core::ResolutionMode,
}

//   Signature is
//     - undefined if FileInfo.version === FileInfo.signature
//     - string actual signature
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct BuildInfoFileInfoWithSignature {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
    #[serde(
        default,
        rename = "affectsGlobalScope",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub affects_global_scope: bool,
    #[serde(
        default,
        rename = "impliedNodeFormat",
        skip_serializing_if = "is_none_resolution_mode"
    )]
    pub implied_node_format: core::ResolutionMode,
}

#[derive(Clone, Default)]
pub struct BuildInfoFileInfo {
    pub signature: String,
    pub no_signature: Option<BuildInfoFileInfoNoSignature>,
    pub file_info: Option<BuildInfoFileInfoWithSignature>,
}

pub fn new_build_info_file_info(file_info: &FileInfo) -> BuildInfoFileInfo {
    if file_info.version == file_info.signature {
        if !file_info.affects_global_scope
            && file_info.implied_node_format == core::ResolutionMode::CommonJS
        {
            return BuildInfoFileInfo {
                signature: file_info.signature.clone(),
                ..Default::default()
            };
        }
    } else if file_info.signature.is_empty() {
        return BuildInfoFileInfo {
            no_signature: Some(BuildInfoFileInfoNoSignature {
                version: file_info.version.clone(),
                no_signature: true,
                affects_global_scope: file_info.affects_global_scope,
                implied_node_format: file_info.implied_node_format,
            }),
            ..Default::default()
        };
    }
    BuildInfoFileInfo {
        file_info: Some(BuildInfoFileInfoWithSignature {
            version: file_info.version.clone(),
            signature: core::if_else(
                file_info.signature == file_info.version,
                String::new(),
                file_info.signature.clone(),
            ),
            affects_global_scope: file_info.affects_global_scope,
            implied_node_format: file_info.implied_node_format,
        }),
        ..Default::default()
    }
}

impl BuildInfoFileInfo {
    pub fn get_file_info(&self) -> Option<FileInfo> {
        if !self.signature.is_empty() {
            return Some(FileInfo {
                version: self.signature.clone(),
                signature: self.signature.clone(),
                implied_node_format: core::ResolutionMode::CommonJS,
                ..Default::default()
            });
        }
        if let Some(no_signature) = &self.no_signature {
            return Some(FileInfo {
                version: no_signature.version.clone(),
                affects_global_scope: no_signature.affects_global_scope,
                implied_node_format: no_signature.implied_node_format,
                ..Default::default()
            });
        }
        self.file_info.as_ref().map(|file_info| FileInfo {
            version: file_info.version.clone(),
            signature: core::if_else(
                file_info.signature.is_empty(),
                file_info.version.clone(),
                file_info.signature.clone(),
            ),
            affects_global_scope: file_info.affects_global_scope,
            implied_node_format: file_info.implied_node_format,
        })
    }

    pub fn has_signature(&self) -> bool {
        !self.signature.is_empty()
    }

    pub fn marshal_json(&self) -> serde_json::Result<Vec<u8>> {
        if !self.signature.is_empty() {
            return serde_json::to_vec(&self.signature);
        }
        if let Some(no_signature) = &self.no_signature {
            return serde_json::to_vec(no_signature);
        }
        serde_json::to_vec(&self.file_info)
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        if matches!(serde_json::from_slice::<Value>(data), Ok(Value::Null)) {
            *self = BuildInfoFileInfo::default();
            return Ok(());
        }
        if let Ok(v_signature) = serde_json::from_slice::<String>(data) {
            *self = BuildInfoFileInfo {
                signature: v_signature,
                ..Default::default()
            };
            return Ok(());
        }
        if let Ok(no_signature) = serde_json::from_slice::<BuildInfoFileInfoNoSignature>(data) {
            if no_signature.no_signature {
                *self = BuildInfoFileInfo {
                    no_signature: Some(no_signature),
                    ..Default::default()
                };
                return Ok(());
            }
        }
        let file_info = serde_json::from_slice::<BuildInfoFileInfoWithSignature>(data)?;
        *self = BuildInfoFileInfo {
            file_info: Some(file_info),
            ..Default::default()
        };
        Ok(())
    }
}

impl Serialize for BuildInfoFileInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if !self.signature.is_empty() {
            return self.signature.serialize(serializer);
        }
        if let Some(no_signature) = &self.no_signature {
            return no_signature.serialize(serializer);
        }
        self.file_info.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BuildInfoFileInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(BuildInfoFileInfo::default());
        }
        if let Ok(v_signature) = serde_json::from_value::<String>(value.clone()) {
            return Ok(BuildInfoFileInfo {
                signature: v_signature,
                ..Default::default()
            });
        }
        if let Ok(no_signature) =
            serde_json::from_value::<BuildInfoFileInfoNoSignature>(value.clone())
        {
            if no_signature.no_signature {
                return Ok(BuildInfoFileInfo {
                    no_signature: Some(no_signature),
                    ..Default::default()
                });
            }
        }
        let file_info = serde_json::from_value::<BuildInfoFileInfoWithSignature>(value.clone())
            .map_err(|_| D::Error::custom(format!("invalid BuildInfoFileInfo: {value}")))?;
        Ok(BuildInfoFileInfo {
            file_info: Some(file_info),
            ..Default::default()
        })
    }
}

#[derive(Clone, Default)]
pub struct BuildInfoReferenceMapEntry {
    pub file_id: BuildInfoFileId,
    pub file_id_list_id: BuildInfoFileIdListId,
}

impl BuildInfoReferenceMapEntry {
    pub fn marshal_json(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(&[self.file_id, self.file_id_list_id])
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        let v = serde_json::from_slice::<[i32; 2]>(data)?;
        *self = BuildInfoReferenceMapEntry {
            file_id: v[0],
            file_id_list_id: v[1],
        };
        Ok(())
    }
}

impl Serialize for BuildInfoReferenceMapEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        [self.file_id, self.file_id_list_id].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BuildInfoReferenceMapEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = <[i32; 2]>::deserialize(deserializer)?;
        Ok(BuildInfoReferenceMapEntry {
            file_id: v[0],
            file_id_list_id: v[1],
        })
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct BuildInfoDiagnostic {
    // BuildInfoFileId if it is for a File thats other than its stored for
    #[serde(default, skip_serializing_if = "is_default")]
    pub file: BuildInfoFileId,
    #[serde(default, rename = "noFile", skip_serializing_if = "std::ops::Not::not")]
    pub no_file: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub pos: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub end: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub code: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub category: diagnostics::Category,
    #[serde(
        default,
        rename = "messageKey",
        skip_serializing_if = "String::is_empty"
    )]
    pub message_key: diagnostics::Key,
    #[serde(default, rename = "messageArgs", skip_serializing_if = "Vec::is_empty")]
    pub message_args: Vec<String>,
    #[serde(
        default,
        rename = "messageChain",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub message_chain: Vec<BuildInfoDiagnostic>,
    #[serde(
        default,
        rename = "relatedInformation",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub related_information: Vec<BuildInfoDiagnostic>,
    #[serde(
        default,
        rename = "reportsUnnecessary",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub reports_unnecessary: bool,
    #[serde(
        default,
        rename = "reportsDeprecated",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub reports_deprecated: bool,
    #[serde(
        default,
        rename = "skippedOnNoEmit",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub skipped_on_no_emit: bool,
    #[serde(
        default,
        rename = "repopulateInfo",
        skip_serializing_if = "Option::is_none"
    )]
    pub repopulate_info: Option<BuildInfoRepopulateInfo>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct BuildInfoRepopulateInfo {
    pub kind: ast::RepopulateDiagnosticKind,
    #[serde(
        default,
        rename = "moduleReference",
        skip_serializing_if = "String::is_empty"
    )]
    pub module_reference: String,
    #[serde(default, skip_serializing_if = "is_none_resolution_mode")]
    pub mode: core::ResolutionMode,
    #[serde(
        default,
        rename = "packageName",
        skip_serializing_if = "String::is_empty"
    )]
    pub package_name: String,
}

#[derive(Clone, Default)]
pub struct BuildInfoDiagnosticsOfFile {
    pub file_id: BuildInfoFileId,
    pub diagnostics: Vec<BuildInfoDiagnostic>,
}

impl BuildInfoDiagnosticsOfFile {
    pub fn marshal_json(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(&serde_json::json!([self.file_id, self.diagnostics]))
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        let file_id_and_diagnostics = serde_json::from_slice::<Vec<Value>>(data)?;
        if file_id_and_diagnostics.len() != 2 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "invalid BuildInfoDiagnosticsOfFile: expected 2 elements, got {}",
                    file_id_and_diagnostics.len()
                ),
            )));
        }
        let file_id = serde_json::from_value(file_id_and_diagnostics[0].clone())?;
        let diagnostics = serde_json::from_value(file_id_and_diagnostics[1].clone())?;
        *self = BuildInfoDiagnosticsOfFile {
            file_id,
            diagnostics,
        };
        Ok(())
    }
}

impl Serialize for BuildInfoDiagnosticsOfFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (&self.file_id, &self.diagnostics).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BuildInfoDiagnosticsOfFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Some(file_id_and_diagnostics) = value.as_array() else {
            return Err(D::Error::custom(format!(
                "invalid BuildInfoDiagnosticsOfFile: {value}"
            )));
        };
        if file_id_and_diagnostics.len() != 2 {
            return Err(D::Error::custom(format!(
                "invalid BuildInfoDiagnosticsOfFile: expected 2 elements, got {}",
                file_id_and_diagnostics.len()
            )));
        }
        let file_id =
            serde_json::from_value(file_id_and_diagnostics[0].clone()).map_err(|err| {
                D::Error::custom(format!(
                    "invalid fileId in BuildInfoDiagnosticsOfFile: {err}"
                ))
            })?;
        let diagnostics =
            serde_json::from_value(file_id_and_diagnostics[1].clone()).map_err(|err| {
                D::Error::custom(format!(
                    "invalid diagnostics in BuildInfoDiagnosticsOfFile: {err}"
                ))
            })?;
        Ok(BuildInfoDiagnosticsOfFile {
            file_id,
            diagnostics,
        })
    }
}

#[derive(Clone, Default)]
pub struct BuildInfoSemanticDiagnostic {
    pub file_id: BuildInfoFileId, // File is not in changedSet and still doesnt have cached diagnostics
    pub diagnostics: Option<BuildInfoDiagnosticsOfFile>, // Diagnostics for file
}

impl BuildInfoSemanticDiagnostic {
    pub fn marshal_json(&self) -> serde_json::Result<Vec<u8>> {
        if self.file_id != 0 {
            return serde_json::to_vec(&self.file_id);
        }
        serde_json::to_vec(&self.diagnostics)
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        if matches!(serde_json::from_slice::<Value>(data), Ok(Value::Null)) {
            *self = BuildInfoSemanticDiagnostic::default();
            return Ok(());
        }
        if let Ok(file_id) = serde_json::from_slice::<BuildInfoFileId>(data) {
            *self = BuildInfoSemanticDiagnostic {
                file_id,
                diagnostics: None,
            };
            return Ok(());
        }
        let diagnostics = serde_json::from_slice::<BuildInfoDiagnosticsOfFile>(data)?;
        *self = BuildInfoSemanticDiagnostic {
            file_id: 0,
            diagnostics: Some(diagnostics),
        };
        Ok(())
    }
}

impl Serialize for BuildInfoSemanticDiagnostic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.file_id != 0 {
            return self.file_id.serialize(serializer);
        }
        self.diagnostics.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BuildInfoSemanticDiagnostic {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(BuildInfoSemanticDiagnostic::default());
        }
        if let Ok(file_id) = serde_json::from_value::<BuildInfoFileId>(value.clone()) {
            return Ok(BuildInfoSemanticDiagnostic {
                file_id,
                diagnostics: None,
            });
        }
        let diagnostics = serde_json::from_value::<BuildInfoDiagnosticsOfFile>(value.clone())
            .map_err(|_| {
                D::Error::custom(format!("invalid BuildInfoSemanticDiagnostic: {value}"))
            })?;
        Ok(BuildInfoSemanticDiagnostic {
            file_id: 0,
            diagnostics: Some(diagnostics),
        })
    }
}

// fileId if pending emit is same as what compilerOptions suggest
// [fileId] if pending emit is only dts file emit
// [fileId, emitKind] if any other type emit is pending
#[derive(Clone, Default)]
pub struct BuildInfoFilePendingEmit {
    pub file_id: BuildInfoFileId,
    pub emit_kind: FileEmitKind,
}

impl BuildInfoFilePendingEmit {
    pub fn marshal_json(&self) -> serde_json::Result<Vec<u8>> {
        if self.emit_kind == 0 {
            return serde_json::to_vec(&self.file_id);
        }
        if self.emit_kind == FILE_EMIT_KIND_DTS {
            return serde_json::to_vec(&vec![self.file_id]);
        }
        serde_json::to_vec(&vec![self.file_id, self.emit_kind as i32])
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        if matches!(serde_json::from_slice::<Value>(data), Ok(Value::Null)) {
            *self = BuildInfoFilePendingEmit::default();
            return Ok(());
        }
        if let Ok(file_id) = serde_json::from_slice::<BuildInfoFileId>(data) {
            *self = BuildInfoFilePendingEmit {
                file_id,
                emit_kind: 0,
            };
            return Ok(());
        }
        let int_tuple = serde_json::from_slice::<Vec<i32>>(data)?;
        match int_tuple.len() {
            1 => {
                *self = BuildInfoFilePendingEmit {
                    file_id: int_tuple[0],
                    emit_kind: FILE_EMIT_KIND_DTS,
                };
                Ok(())
            }
            2 => {
                *self = BuildInfoFilePendingEmit {
                    file_id: int_tuple[0],
                    emit_kind: int_tuple[1] as FileEmitKind,
                };
                Ok(())
            }
            _ => Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "invalid BuildInfoFilePendingEmit: expected 1 or 2 integers, got {}",
                    int_tuple.len()
                ),
            ))),
        }
    }
}

impl Serialize for BuildInfoFilePendingEmit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.emit_kind == 0 {
            return self.file_id.serialize(serializer);
        }
        if self.emit_kind == FILE_EMIT_KIND_DTS {
            return [self.file_id].serialize(serializer);
        }
        [self.file_id, self.emit_kind as i32].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BuildInfoFilePendingEmit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(BuildInfoFilePendingEmit::default());
        }
        if let Ok(file_id) = serde_json::from_value::<BuildInfoFileId>(value.clone()) {
            return Ok(BuildInfoFilePendingEmit {
                file_id,
                emit_kind: 0,
            });
        }
        let int_tuple = serde_json::from_value::<Vec<i32>>(value.clone())
            .map_err(|_| D::Error::custom(format!("invalid BuildInfoFilePendingEmit: {value}")))?;
        match int_tuple.len() {
            1 => Ok(BuildInfoFilePendingEmit {
                file_id: int_tuple[0],
                emit_kind: FILE_EMIT_KIND_DTS,
            }),
            2 => Ok(BuildInfoFilePendingEmit {
                file_id: int_tuple[0],
                emit_kind: int_tuple[1] as FileEmitKind,
            }),
            _ => Err(D::Error::custom(format!(
                "invalid BuildInfoFilePendingEmit: expected 1 or 2 integers, got {}",
                int_tuple.len()
            ))),
        }
    }
}

// [fileId, signature] if different from file's signature
// fileId if file wasnt emitted
#[derive(Clone, Default)]
pub struct BuildInfoEmitSignature {
    pub file_id: BuildInfoFileId,
    pub signature: String, // Signature if it is different from file's Signature
    pub differs_only_in_dts_map: bool, // true if signature is different only in dtsMap value
    pub differs_in_options: bool, // true if signature is different in options used to emit file
}

impl BuildInfoEmitSignature {
    pub fn no_emit_signature(&self) -> bool {
        self.signature.is_empty() && !self.differs_only_in_dts_map && !self.differs_in_options
    }

    pub fn to_emit_signature(
        &self,
        path: tspath::Path,
        emit_signatures: &HashMap<tspath::Path, EmitSignature>,
    ) -> EmitSignature {
        let mut signature = String::new();
        let mut signature_with_different_options = Vec::new();
        if self.differs_only_in_dts_map {
            let info = emit_signatures
                .get(&path)
                .expect("emit signature must exist when differs_only_in_dts_map is set");
            signature_with_different_options.push(info.signature.clone());
        } else if self.differs_in_options {
            signature_with_different_options.push(self.signature.clone());
        } else {
            signature = self.signature.clone();
        }
        EmitSignature {
            signature,
            signature_with_different_options,
        }
    }

    pub fn marshal_json(&self) -> serde_json::Result<Vec<u8>> {
        if self.no_emit_signature() {
            return serde_json::to_vec(&self.file_id);
        }
        let signature = if self.differs_only_in_dts_map {
            Value::Array(Vec::new())
        } else if self.differs_in_options {
            Value::Array(vec![Value::String(self.signature.clone())])
        } else {
            Value::String(self.signature.clone())
        };
        serde_json::to_vec(&Value::Array(vec![
            serde_json::json!(self.file_id),
            signature,
        ]))
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        if matches!(serde_json::from_slice::<Value>(data), Ok(Value::Null)) {
            *self = BuildInfoEmitSignature::default();
            return Ok(());
        }
        if let Ok(file_id) = serde_json::from_slice::<BuildInfoFileId>(data) {
            *self = BuildInfoEmitSignature {
                file_id,
                ..Default::default()
            };
            return Ok(());
        }
        let file_id_and_signature = serde_json::from_slice::<Vec<Value>>(data)?;
        if file_id_and_signature.len() != 2 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "invalid BuildInfoEmitSignature: expected 2 elements, got {}",
                    file_id_and_signature.len()
                ),
            )));
        }
        let file_id = file_id_and_signature[0].as_f64().ok_or_else(|| {
            serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid fileId in BuildInfoEmitSignature: expected float64",
            ))
        })? as BuildInfoFileId;
        let mut signature = String::new();
        let mut differs_only_in_dts_map = false;
        let mut differs_in_options = false;
        match &file_id_and_signature[1] {
            Value::String(value) => signature = value.clone(),
            Value::Array(signature_list) => match signature_list.len() {
                0 => differs_only_in_dts_map = true,
                1 => {
                    let Some(sig) = signature_list[0].as_str() else {
                        return Err(serde_json::Error::io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "invalid signature in BuildInfoEmitSignature: expected string, got {:?}",
                                signature_list[0]
                            ),
                        )));
                    };
                    signature = sig.to_owned();
                    differs_in_options = true;
                }
                _ => {
                    return Err(serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "invalid signature in BuildInfoEmitSignature: expected string or []string with 0 or 1 element, got {} elements",
                            signature_list.len()
                        ),
                    )));
                }
            },
            other => {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "invalid signature in BuildInfoEmitSignature: expected string or []string, got {other:?}"
                    ),
                )));
            }
        }
        *self = BuildInfoEmitSignature {
            file_id,
            signature,
            differs_only_in_dts_map,
            differs_in_options,
        };
        Ok(())
    }
}

impl Serialize for BuildInfoEmitSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.no_emit_signature() {
            return self.file_id.serialize(serializer);
        }
        let signature = if self.differs_only_in_dts_map {
            Value::Array(Vec::new())
        } else if self.differs_in_options {
            Value::Array(vec![Value::String(self.signature.clone())])
        } else {
            Value::String(self.signature.clone())
        };
        (self.file_id, signature).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BuildInfoEmitSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(BuildInfoEmitSignature::default());
        }
        if let Ok(file_id) = serde_json::from_value::<BuildInfoFileId>(value.clone()) {
            return Ok(BuildInfoEmitSignature {
                file_id,
                ..Default::default()
            });
        }
        let Some(file_id_and_signature) = value.as_array() else {
            return Err(D::Error::custom(format!(
                "invalid BuildInfoEmitSignature: {value}"
            )));
        };
        if file_id_and_signature.len() != 2 {
            return Err(D::Error::custom(format!(
                "invalid BuildInfoEmitSignature: expected 2 elements, got {}",
                file_id_and_signature.len()
            )));
        }
        let file_id = file_id_and_signature[0].as_f64().ok_or_else(|| {
            D::Error::custom(format!(
                "invalid fileId in BuildInfoEmitSignature: expected float64, got {:?}",
                file_id_and_signature[0]
            ))
        })? as BuildInfoFileId;
        let mut signature = String::new();
        let mut differs_only_in_dts_map = false;
        let mut differs_in_options = false;
        match &file_id_and_signature[1] {
            Value::String(value) => signature = value.clone(),
            Value::Array(signature_list) => match signature_list.len() {
                0 => differs_only_in_dts_map = true,
                1 => {
                    let Some(sig) = signature_list[0].as_str() else {
                        return Err(D::Error::custom(format!(
                            "invalid signature in BuildInfoEmitSignature: expected string, got {:?}",
                            signature_list[0]
                        )));
                    };
                    signature = sig.to_owned();
                    differs_in_options = true;
                }
                _ => {
                    return Err(D::Error::custom(format!(
                        "invalid signature in BuildInfoEmitSignature: expected string or []string with 0 or 1 element, got {} elements",
                        signature_list.len()
                    )));
                }
            },
            other => {
                return Err(D::Error::custom(format!(
                    "invalid signature in BuildInfoEmitSignature: expected string or []string, got {other:?}"
                )));
            }
        }
        Ok(BuildInfoEmitSignature {
            file_id,
            signature,
            differs_only_in_dts_map,
            differs_in_options,
        })
    }
}

#[derive(Clone, Default)]
pub struct BuildInfoResolvedRoot {
    pub resolved: BuildInfoFileId,
    pub root: BuildInfoFileId,
}

impl BuildInfoResolvedRoot {
    pub fn marshal_json(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(&[self.resolved, self.root])
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        if matches!(serde_json::from_slice::<Value>(data), Ok(Value::Null)) {
            *self = BuildInfoResolvedRoot::default();
            return Ok(());
        }
        let resolved_and_root = serde_json::from_slice::<[i32; 2]>(data)?;
        *self = BuildInfoResolvedRoot {
            resolved: resolved_and_root[0],
            root: resolved_and_root[1],
        };
        Ok(())
    }
}

impl Serialize for BuildInfoResolvedRoot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        [self.resolved, self.root].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BuildInfoResolvedRoot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(BuildInfoResolvedRoot::default());
        }
        let resolved_and_root = serde_json::from_value::<[i32; 2]>(value)
            .map_err(|_| D::Error::custom("invalid BuildInfoResolvedRoot"))?;
        Ok(BuildInfoResolvedRoot {
            resolved: resolved_and_root[0],
            root: resolved_and_root[1],
        })
    }
}

#[derive(Clone, Default)]
pub struct BuildInfo {
    pub version: String,

    // Common between incremental and tsc -b buildinfo for non incremental programs
    pub errors: bool,
    pub check_pending: bool,
    pub root: Vec<BuildInfoRoot>,

    // IncrementalProgram info
    pub file_names: Vec<String>,
    pub file_infos: Vec<BuildInfoFileInfo>,
    pub file_infos_present: bool,
    pub file_ids_list: Vec<Vec<BuildInfoFileId>>,
    pub options: OrderedMap<String, Value>,
    pub referenced_map: Vec<BuildInfoReferenceMapEntry>,
    pub semantic_diagnostics_per_file: Vec<BuildInfoSemanticDiagnostic>,
    pub emit_diagnostics_per_file: Vec<BuildInfoDiagnosticsOfFile>,
    pub change_file_set: Vec<BuildInfoFileId>,
    pub affected_files_pending_emit: Vec<BuildInfoFilePendingEmit>,
    pub latest_changed_dts_file: String, // Because this is only output file in the program, we dont need fileId to deduplicate name
    pub emit_signatures: Vec<BuildInfoEmitSignature>,
    pub resolved_root: Vec<BuildInfoResolvedRoot>,

    // NonIncrementalProgram info
    pub semantic_errors: bool,
}

impl Serialize for BuildInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("BuildInfo", 17)?;
        if !self.version.is_empty() {
            state.serialize_field("version", &self.version)?;
        }
        if self.errors {
            state.serialize_field("errors", &self.errors)?;
        }
        if self.check_pending {
            state.serialize_field("checkPending", &self.check_pending)?;
        }
        if !self.root.is_empty() {
            state.serialize_field("root", &self.root)?;
        }
        if !self.file_names.is_empty() {
            state.serialize_field("fileNames", &self.file_names)?;
        }
        if self.file_infos_present || !self.file_infos.is_empty() {
            state.serialize_field("fileInfos", &self.file_infos)?;
        }
        if !self.file_ids_list.is_empty() {
            state.serialize_field("fileIdsList", &self.file_ids_list)?;
        }
        if self.options.size() != 0 {
            state.serialize_field("options", &self.options)?;
        }
        if !self.referenced_map.is_empty() {
            state.serialize_field("referencedMap", &self.referenced_map)?;
        }
        if !self.semantic_diagnostics_per_file.is_empty() {
            state.serialize_field(
                "semanticDiagnosticsPerFile",
                &self.semantic_diagnostics_per_file,
            )?;
        }
        if !self.emit_diagnostics_per_file.is_empty() {
            state.serialize_field("emitDiagnosticsPerFile", &self.emit_diagnostics_per_file)?;
        }
        if !self.change_file_set.is_empty() {
            state.serialize_field("changeFileSet", &self.change_file_set)?;
        }
        if !self.affected_files_pending_emit.is_empty() {
            state.serialize_field(
                "affectedFilesPendingEmit",
                &self.affected_files_pending_emit,
            )?;
        }
        if !self.latest_changed_dts_file.is_empty() {
            state.serialize_field("latestChangedDtsFile", &self.latest_changed_dts_file)?;
        }
        if !self.emit_signatures.is_empty() {
            state.serialize_field("emitSignatures", &self.emit_signatures)?;
        }
        if !self.resolved_root.is_empty() {
            state.serialize_field("resolvedRoot", &self.resolved_root)?;
        }
        if self.semantic_errors {
            state.serialize_field("semanticErrors", &self.semantic_errors)?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for BuildInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        struct BuildInfoWire {
            #[serde(default)]
            version: String,
            #[serde(default)]
            errors: bool,
            #[serde(default, rename = "checkPending")]
            check_pending: bool,
            #[serde(default)]
            root: Vec<BuildInfoRoot>,
            #[serde(default, rename = "fileNames")]
            file_names: Vec<String>,
            #[serde(default, rename = "fileInfos")]
            file_infos: Option<Vec<BuildInfoFileInfo>>,
            #[serde(default, rename = "fileIdsList")]
            file_ids_list: Vec<Vec<BuildInfoFileId>>,
            #[serde(default)]
            options: OrderedMap<String, Value>,
            #[serde(default, rename = "referencedMap")]
            referenced_map: Vec<BuildInfoReferenceMapEntry>,
            #[serde(default, rename = "semanticDiagnosticsPerFile")]
            semantic_diagnostics_per_file: Vec<BuildInfoSemanticDiagnostic>,
            #[serde(default, rename = "emitDiagnosticsPerFile")]
            emit_diagnostics_per_file: Vec<BuildInfoDiagnosticsOfFile>,
            #[serde(default, rename = "changeFileSet")]
            change_file_set: Vec<BuildInfoFileId>,
            #[serde(default, rename = "affectedFilesPendingEmit")]
            affected_files_pending_emit: Vec<BuildInfoFilePendingEmit>,
            #[serde(default, rename = "latestChangedDtsFile")]
            latest_changed_dts_file: String,
            #[serde(default, rename = "emitSignatures")]
            emit_signatures: Vec<BuildInfoEmitSignature>,
            #[serde(default, rename = "resolvedRoot")]
            resolved_root: Vec<BuildInfoResolvedRoot>,
            #[serde(default, rename = "semanticErrors")]
            semantic_errors: bool,
        }

        let wire = BuildInfoWire::deserialize(deserializer)?;
        let file_infos_present = wire.file_infos.is_some();
        Ok(BuildInfo {
            version: wire.version,
            errors: wire.errors,
            check_pending: wire.check_pending,
            root: wire.root,
            file_names: wire.file_names,
            file_infos: wire.file_infos.unwrap_or_default(),
            file_infos_present,
            file_ids_list: wire.file_ids_list,
            options: wire.options,
            referenced_map: wire.referenced_map,
            semantic_diagnostics_per_file: wire.semantic_diagnostics_per_file,
            emit_diagnostics_per_file: wire.emit_diagnostics_per_file,
            change_file_set: wire.change_file_set,
            affected_files_pending_emit: wire.affected_files_pending_emit,
            latest_changed_dts_file: wire.latest_changed_dts_file,
            emit_signatures: wire.emit_signatures,
            resolved_root: wire.resolved_root,
            semantic_errors: wire.semantic_errors,
        })
    }
}

impl BuildInfo {
    pub fn is_valid_version(&self) -> bool {
        self.version == core::version()
    }

    pub fn is_incremental(&self) -> bool {
        !self.file_names.is_empty()
    }

    pub fn file_name(&self, file_id: BuildInfoFileId) -> String {
        self.file_names[(file_id - 1) as usize].clone()
    }

    pub fn file_info(&self, file_id: BuildInfoFileId) -> BuildInfoFileInfo {
        self.file_infos[(file_id - 1) as usize].clone()
    }

    pub fn get_compiler_options(&self, build_info_directory: &str) -> core::CompilerOptions {
        let mut options = core::CompilerOptions::default();
        for (option, value) in self.options.entries() {
            if !build_info_directory.is_empty() {
                let (result, ok) = tsoptions::convert_option_to_absolute_path(
                    option,
                    value.clone(),
                    &tsoptions::COMMAND_LINE_COMPILER_OPTIONS_MAP,
                    build_info_directory,
                );
                if ok {
                    tsoptions::parse_compiler_options(option, result, &mut options);
                    continue;
                }
            }
            tsoptions::parse_compiler_options(option, value.clone(), &mut options);
        }
        options
    }

    pub fn is_emit_pending(
        &self,
        resolved: &tsoptions::ParsedCommandLine,
        build_info_directory: &str,
    ) -> bool {
        // Some of the emit files like source map or dts etc are not yet done
        if !resolved.compiler_options().no_emit.is_true()
            || resolved.compiler_options().get_emit_declarations()
        {
            let mut pending_emit = get_pending_emit_kind_with_options(
                resolved.compiler_options(),
                self.get_compiler_options(build_info_directory),
            );
            if resolved.compiler_options().no_emit.is_true() {
                pending_emit &= FILE_EMIT_KIND_DTS_ERRORS;
            }
            return pending_emit != 0;
        }
        false
    }

    pub fn get_build_info_root_info_reader(
        &self,
        build_info_directory: &str,
        compare_path_options: tspath::ComparePathsOptions,
    ) -> BuildInfoRootInfoReader {
        let mut resolved_root_file_infos = HashMap::with_capacity(self.file_names.len());
        // Roots of the File
        let mut root_to_resolved = OrderedMap::with_size_hint(self.file_names.len());
        let mut resolved_to_root = HashMap::with_capacity(self.resolved_root.len());
        let to_path = |file_name: String| -> tspath::Path {
            tspath::to_path(
                &file_name,
                build_info_directory,
                compare_path_options.use_case_sensitive_file_names,
            )
        };

        // Create map from resolvedRoot to Root
        for resolved in &self.resolved_root {
            resolved_to_root.insert(
                to_path(self.file_name(resolved.resolved)),
                to_path(self.file_name(resolved.root)),
            );
        }

        let mut add_root = |resolved_root: String, file_info: Option<BuildInfoFileInfo>| {
            let resolved_root_path = to_path(resolved_root);
            if let Some(root_path) = resolved_to_root.get(&resolved_root_path) {
                root_to_resolved.set(root_path.clone(), resolved_root_path.clone());
            } else {
                root_to_resolved.set(resolved_root_path.clone(), resolved_root_path.clone());
            }
            if let Some(file_info) = file_info {
                resolved_root_file_infos.insert(resolved_root_path, file_info);
            }
        };

        for root in &self.root {
            if !root.non_incremental.is_empty() {
                add_root(root.non_incremental.clone(), None);
            } else if root.end == 0 {
                add_root(self.file_name(root.start), Some(self.file_info(root.start)));
            } else {
                for i in root.start..=root.end {
                    add_root(self.file_name(i), Some(self.file_info(i)));
                }
            }
        }

        BuildInfoRootInfoReader {
            resolved_root_file_infos,
            root_to_resolved,
        }
    }
}

pub fn is_none_resolution_mode(mode: &core::ResolutionMode) -> bool {
    *mode == core::ResolutionMode::None
}

pub fn is_default<T>(value: &T) -> bool
where
    T: Default + PartialEq,
{
    value == &T::default()
}

pub fn ordered_map_is_empty(map: &OrderedMap<String, Value>) -> bool {
    map.size() == 0
}

fn get_pending_emit_kind_with_options(
    resolved: core::CompilerOptions,
    build_info: core::CompilerOptions,
) -> FileEmitKind {
    snapshot::get_pending_emit_kind_with_options(resolved, build_info)
}

pub struct BuildInfoRootInfoReader {
    pub resolved_root_file_infos: HashMap<tspath::Path, BuildInfoFileInfo>,
    pub root_to_resolved: OrderedMap<tspath::Path, tspath::Path>,
}

impl BuildInfoRootInfoReader {
    pub fn get_build_info_file_info(
        &self,
        input_file_path: tspath::Path,
    ) -> (Option<BuildInfoFileInfo>, tspath::Path) {
        if let Some(info) = self.resolved_root_file_infos.get(&input_file_path) {
            return (Some(info.clone()), input_file_path);
        }
        if let Some(resolved) = self.root_to_resolved.get(&input_file_path) {
            return (
                self.resolved_root_file_infos.get(resolved).cloned(),
                resolved.clone(),
            );
        }
        (None, String::new())
    }

    pub fn roots(&self) -> impl Iterator<Item = tspath::Path> + '_ {
        self.root_to_resolved.keys().cloned()
    }
}
