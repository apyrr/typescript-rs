#![allow(non_upper_case_globals)]

use std::{
    collections::HashMap,
    fmt,
    marker::PhantomData,
    sync::{LazyLock, Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use ts_bundled as bundled;
use ts_core as core;
use ts_json as json;
use ts_jsonrpc as jsonrpc;
use ts_tspath as tspath;

use super::{FormattingOptions, Position, Range};

macro_rules! object_or_null_wrapper {
    ($name:ident, $field:ident, $ty:ty) => {
        #[derive(Clone, Debug, Default)]
        pub struct $name {
            pub $field: Option<$ty>,
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                if let Some(value) = &self.$field {
                    return value.serialize(serializer);
                }
                serializer.serialize_none()
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = serde_json::Value::deserialize(deserializer)?;
                match value {
                    serde_json::Value::Null => Ok(Self::default()),
                    serde_json::Value::Object(_) => {
                        let value =
                            serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                        Ok(Self {
                            $field: Some(value),
                        })
                    }
                    other => Err(serde::de::Error::custom(format!(
                        "invalid {}: got {other}",
                        stringify!($name)
                    ))),
                }
            }
        }
    };
}

pub type DocumentUri = String;
pub type LanguageKind = String;
pub type LSPAny = serde_json::Value;
pub type Uri = String;
pub type Method = String;
pub type ErrorCode = i32;
pub type ShowDocumentResult = lsp_types_full::ShowDocumentResult;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct InsertTextFormat(pub i32);

impl InsertTextFormat {
    pub const PlainText: Self = Self(1);
    pub const Snippet: Self = Self(2);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CodeLensKind {
    #[serde(rename = "references")]
    References,
    #[serde(rename = "implementations")]
    Implementations,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UintegerOrNull {
    pub uinteger: Option<u32>,
}

impl Serialize for UintegerOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(uinteger) = self.uinteger {
            return uinteger.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for UintegerOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(Self::default());
        }
        let uinteger = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            uinteger: Some(uinteger),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StringOrTuple {
    pub string: Option<String>,
    pub tuple: Option<[u32; 2]>,
}

impl StringOrTuple {
    pub fn from_string(value: String) -> Self {
        Self {
            string: Some(value),
            tuple: None,
        }
    }

    pub fn from_tuple(value: [u32; 2]) -> Self {
        Self {
            string: None,
            tuple: Some(value),
        }
    }
}

impl Serialize for StringOrTuple {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of StringOrTuple should be set",
            bool_to_int(self.string.is_some()) + bool_to_int(self.tuple.is_some()),
        );
        if let Some(string) = &self.string {
            return string.serialize(serializer);
        }
        self.tuple
            .expect("StringOrTuple has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StringOrTuple {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(string) => Ok(Self {
                string: Some(string),
                tuple: None,
            }),
            serde_json::Value::Array(_) => Ok(Self {
                string: None,
                tuple: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
            }),
            other => Err(serde::de::Error::custom(format!(
                "invalid StringOrTuple: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StringOrMarkupContent {
    pub string: Option<String>,
    pub markup_content: Option<MarkupContent>,
}

impl Serialize for StringOrMarkupContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of StringOrMarkupContent should be set",
            bool_to_int(self.string.is_some()) + bool_to_int(self.markup_content.is_some()),
        );
        if let Some(string) = &self.string {
            return string.serialize(serializer);
        }
        self.markup_content
            .as_ref()
            .expect("StringOrMarkupContent has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StringOrMarkupContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(string) => Ok(Self {
                string: Some(string),
                markup_content: None,
            }),
            serde_json::Value::Object(_) => Ok(Self {
                string: None,
                markup_content: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
            }),
            other => Err(serde::de::Error::custom(format!(
                "invalid StringOrMarkupContent: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyWorkspaceEditResult {
    pub applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_change: Option<u32>,
}

impl<'de> Deserialize<'de> for ApplyWorkspaceEditResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["applied"])?;

        let applied = take_non_null_required_field(&mut map, "applied")?;
        let failure_reason = take_non_null_optional_field(&mut map, "failureReason")?;
        let failed_change = take_non_null_optional_field(&mut map, "failedChange")?;

        Ok(Self {
            applied,
            failure_reason,
            failed_change,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShowDocumentResultOrNull {
    pub show_document_result: Option<ShowDocumentResult>,
}

impl Serialize for ShowDocumentResultOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(show_document_result) = &self.show_document_result {
            return show_document_result.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for ShowDocumentResultOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let show_document_result =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    show_document_result: Some(show_document_result),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid ShowDocumentResultOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ApplyWorkspaceEditResultOrNull {
    pub apply_workspace_edit_result: Option<ApplyWorkspaceEditResult>,
}

impl Serialize for ApplyWorkspaceEditResultOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(apply_workspace_edit_result) = &self.apply_workspace_edit_result {
            return apply_workspace_edit_result.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for ApplyWorkspaceEditResultOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let apply_workspace_edit_result =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    apply_workspace_edit_result: Some(apply_workspace_edit_result),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid ApplyWorkspaceEditResultOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LSPAnyOrNull {
    pub lsp_any: Option<LSPAny>,
}

impl Serialize for LSPAnyOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(lsp_any) = &self.lsp_any {
            return lsp_any.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for LSPAnyOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(Self::default());
        }
        Ok(Self {
            lsp_any: Some(value),
        })
    }
}

pub type MarkupKind = lsp_types_full::MarkupKind;
pub type Documentation = lsp_types_full::Documentation;
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum PositionEncodingKind {
    #[serde(rename = "utf-8")]
    Utf8,
    #[default]
    #[serde(rename = "utf-16")]
    Utf16,
    #[serde(rename = "utf-32")]
    Utf32,
}

#[allow(non_upper_case_globals)]
impl PositionEncodingKind {
    pub const UTF8: Self = Self::Utf8;
    pub const UTF16: Self = Self::Utf16;
    pub const UTF32: Self = Self::Utf32;
}

pub const PositionEncodingKindUTF8: PositionEncodingKind = PositionEncodingKind::UTF8;
pub const PositionEncodingKindUTF16: PositionEncodingKind = PositionEncodingKind::UTF16;
pub const PositionEncodingKindUTF32: PositionEncodingKind = PositionEncodingKind::UTF32;
pub type ResourceOperationKind = lsp_types_full::ResourceOperationKind;
pub type FileSystemWatcher = lsp_types_full::FileSystemWatcher;
pub type FileOperationFilter = lsp_types_full::FileOperationFilter;
pub type FileOperationPattern = lsp_types_full::FileOperationPattern;
pub type FileOperationOptions = lsp_types_full::WorkspaceFileOperationsServerCapabilities;
pub type FileOperationRegistrationOptions = lsp_types_full::FileOperationRegistrationOptions;
pub type GlobPattern = lsp_types_full::GlobPattern;
pub type OneOf<A, B> = lsp_types_full::OneOf<A, B>;
pub type Pattern = lsp_types_full::Pattern;
pub type RelativePattern = lsp_types_full::RelativePattern;
pub type WatchKind = lsp_types_full::WatchKind;
pub type WorkspaceFolder = lsp_types_full::WorkspaceFolder;
pub type Color = lsp_types_full::Color;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CancelParams {
    pub id: IntegerOrString,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenTextDocumentParams {
    pub text_document: TextDocumentItem,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
    pub uri: DocumentUri,
    pub language_id: LanguageKind,
    pub version: i32,
    pub text: String,
}
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionedTextDocumentIdentifier {
    pub uri: DocumentUri,
    pub version: i32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeTextDocumentParams {
    pub text_document: VersionedTextDocumentIdentifier,
    pub content_changes: Vec<TextDocumentContentChangePartialOrWholeDocument>,
}
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidCloseTextDocumentParams {
    pub text_document: TextDocumentIdentifier,
}
pub type WillSaveTextDocumentParams = lsp_types_full::WillSaveTextDocumentParams;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileChangeType(pub u32);

impl FileChangeType {
    pub const Created: Self = Self(1);
    pub const Changed: Self = Self(2);
    pub const Deleted: Self = Self(3);
    pub const CREATED: Self = Self(1);
    pub const CHANGED: Self = Self(2);
    pub const DELETED: Self = Self(3);
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileEvent {
    pub uri: DocumentUri,
    #[serde(rename = "type")]
    pub typ: FileChangeType,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DidChangeWatchedFilesParams {
    pub changes: Vec<FileEvent>,
}
pub type WindowClientCapabilities = lsp_types_full::WindowClientCapabilities;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<CompletionContext>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<SignatureHelpContext>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    pub context: ReferenceContext,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHighlightParams {
    pub text_document_position_params: TextDocumentPositionParams,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
}
pub type DocumentSymbolParams = lsp_types_full::DocumentSymbolParams;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    pub text_document: TextDocumentIdentifier,
    pub range: Range,
    pub context: CodeActionContext,
}

impl CodeActionParams {
    pub fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionContext {
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<CodeActionKind>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_kind: Option<CodeActionTriggerKind>,
}

#[allow(non_upper_case_globals)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CodeActionTriggerKind(pub u32);

#[allow(non_upper_case_globals)]
impl CodeActionTriggerKind {
    pub const Invoked: Self = Self(1);
    pub const Automatic: Self = Self(2);
}
pub type WorkspaceSymbolParams = lsp_types_full::WorkspaceSymbolParams;
pub type CodeLensParams = lsp_types_full::CodeLensParams;
pub type DocumentLinkParams = lsp_types_full::DocumentLinkParams;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFormattingParams {
    pub text_document: TextDocumentIdentifier,
    pub options: FormattingOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRangeFormattingParams {
    pub text_document: TextDocumentIdentifier,
    pub range: Range,
    pub options: FormattingOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentOnTypeFormattingParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub ch: String,
    pub options: FormattingOptions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    pub new_name: String,
}

pub type ExecuteCommandParams = lsp_types_full::ExecuteCommandParams;
pub type ApplyWorkspaceEditParams = lsp_types_full::ApplyWorkspaceEditParams;
pub type SelectionRangeParams = lsp_types_full::SelectionRangeParams;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyPrepareParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensParams {
    pub text_document: TextDocumentIdentifier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
}
pub type SemanticTokensDeltaParams = lsp_types_full::SemanticTokensDeltaParams;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensRangeParams {
    pub text_document: TextDocumentIdentifier,
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
}
pub type ShowDocumentParams = lsp_types_full::ShowDocumentParams;
pub type LinkedEditingRangeParams = lsp_types_full::LinkedEditingRangeParams;
pub type MonikerParams = lsp_types_full::MonikerParams;
pub type TypeHierarchyPrepareParams = lsp_types_full::TypeHierarchyPrepareParams;
pub type InlineValueParams = lsp_types_full::InlineValueParams;
pub type InlayHintParams = lsp_types_full::InlayHintParams;
pub type DocumentDiagnosticParams = lsp_types_full::DocumentDiagnosticParams;
pub type WorkspaceDiagnosticParams = lsp_types_full::WorkspaceDiagnosticParams;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationParams {
    pub registrations: Vec<Registration>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub register_options: Option<RegisterOptions>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_did_change_watched_files: Option<DidChangeWatchedFilesRegistrationOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_did_change_configuration: Option<DidChangeConfigurationRegistrationOptions>,
}

pub type DidChangeWatchedFilesRegistrationOptions =
    lsp_types_full::DidChangeWatchedFilesRegistrationOptions;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeConfigurationRegistrationOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<StringOrStrings>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StringOrStrings {
    String(String),
    Strings(Vec<String>),
}

pub type UnregistrationParams = lsp_types_full::UnregistrationParams;
pub type Unregistration = lsp_types_full::Unregistration;
pub type CodeActionResolveParams = CodeAction;
pub type WorkspaceSymbolResolveParams = WorkspaceSymbol;
pub type CodeLensResolveParams = CodeLens;
pub type DocumentLinkResolveParams = DocumentLink;
pub type ImplementationResponse = LocationOrLocationsOrDefinitionLinksOrNull;
pub type TypeDefinitionResponse = LocationOrLocationsOrDefinitionLinksOrNull;
pub type WorkspaceFoldersResponse = WorkspaceFoldersOrNull;
pub type ConfigurationResponse = Vec<LSPAny>;
pub type DocumentColorResponse = Vec<Option<ColorInformation>>;
pub type ColorPresentationResponse = Vec<Option<ColorPresentation>>;
pub type FoldingRangeResponse = FoldingRangesOrNull;
pub type FoldingRangeRefreshResponse = Null;
pub type DeclarationResponse = LocationOrLocationsOrDeclarationLinksOrNull;
pub type SelectionRangeResponse = SelectionRangesOrNull;
pub type CallHierarchyPrepareResponse = CallHierarchyItemsOrNull;
pub type CallHierarchyIncomingCallsResponse = CallHierarchyIncomingCallsOrNull;
pub type CallHierarchyOutgoingCallsResponse = CallHierarchyOutgoingCallsOrNull;
pub type SemanticTokensResponse = SemanticTokensOrNull;
pub type SemanticTokensDeltaResponse = SemanticTokensOrSemanticTokensDeltaOrNull;
pub type SemanticTokensRangeResponse = SemanticTokensOrNull;
pub type SemanticTokensRefreshResponse = Null;
pub type ShowDocumentResponse = ShowDocumentResultOrNull;
pub type LinkedEditingRangeResponse = LinkedEditingRangesOrNull;
pub type WillCreateFilesResponse = WorkspaceEditOrNull;
pub type WillRenameFilesResponse = WorkspaceEditOrNull;
pub type WillDeleteFilesResponse = WorkspaceEditOrNull;
pub type MonikerResponse = MonikersOrNull;
pub type TypeHierarchyPrepareResponse = TypeHierarchyItemsOrNull;
pub type TypeHierarchySupertypesResponse = TypeHierarchyItemsOrNull;
pub type TypeHierarchySubtypesResponse = TypeHierarchyItemsOrNull;
pub type InlineValueResponse = InlineValuesOrNull;
pub type InlineValueRefreshResponse = Null;
pub type InlayHintResponse = InlayHintsOrNull;
pub type InlayHintResolveResponse = InlayHintOrNull;
pub type InlayHintRefreshResponse = Null;
pub type DocumentDiagnosticResponse =
    RelatedFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport;
pub type WorkspaceDiagnosticResponse = WorkspaceDiagnosticReportOrNull;
pub type DiagnosticRefreshResponse = Null;
pub type InlineCompletionResponse = InlineCompletionListOrItemsOrNull;
pub type TextDocumentContentResponse = TextDocumentContentResultOrNull;
pub type TextDocumentContentRefreshResponse = Null;
pub type RegistrationResponse = Null;
pub type UnregistrationResponse = Null;
pub type InitializeResponse = InitializeResultOrNull;
pub type TextDocumentSyncOptions = lsp_types_full::TextDocumentSyncOptions;
pub type TextDocumentSyncOptionsOrKind = lsp_types_full::TextDocumentSyncCapability;
pub type TextDocumentSyncKind = lsp_types_full::TextDocumentSyncKind;
pub type BooleanOrSaveOptions = lsp_types_full::TextDocumentSyncSaveOptions;
pub type BooleanOrHoverOptions = lsp_types_full::HoverProviderCapability;
pub type BooleanOrDefinitionOptions =
    lsp_types_full::OneOf<bool, lsp_types_full::DefinitionOptions>;
pub type BooleanOrReferenceOptions = lsp_types_full::OneOf<bool, lsp_types_full::ReferencesOptions>;
pub type WorkspaceOptions = lsp_types_full::WorkspaceServerCapabilities;
pub type ShutdownResponse = Null;
pub type ShowMessageResponse = MessageActionItemOrNull;
pub type WillSaveTextDocumentWaitUntilResponse = TextEditsOrNull;
pub type CompletionResponse = CompletionItemsOrListOrNull;
pub type CompletionResolveResponse = CompletionItemOrNull;
pub type HoverResponse = HoverOrNull;
pub type SignatureHelpResponse = SignatureHelpOrNull;
pub type DefinitionResponse = LocationOrLocationsOrDefinitionLinksOrNull;
pub type ReferencesResponse = LocationsOrNull;
pub type DocumentHighlightResponse = DocumentHighlightsOrNull;
pub type DocumentSymbolResponse = SymbolInformationsOrDocumentSymbolsOrNull;
pub type CodeActionResponse = CommandOrCodeActionArrayOrNull;
pub type CodeActionResolveResponse = CodeActionOrNull;
pub type WorkspaceSymbolResponse = SymbolInformationsOrWorkspaceSymbolsOrNull;
pub type WorkspaceSymbolResolveResponse = WorkspaceSymbolOrNull;
pub type CodeLensResponse = CodeLensesOrNull;
pub type CodeLensResolveResponse = CodeLensOrNull;
pub type CodeLensRefreshResponse = Null;
pub type DocumentLinkResponse = DocumentLinksOrNull;
pub type DocumentLinkResolveResponse = DocumentLinkOrNull;
pub type DocumentFormattingResponse = TextEditsOrNull;
pub type DocumentRangeFormattingResponse = TextEditsOrNull;
pub type DocumentRangesFormattingResponse = TextEditsOrNull;
pub type DocumentOnTypeFormattingResponse = TextEditsOrNull;
pub type RenameResponse = WorkspaceEditOrNull;
pub type PrepareRenameResponse =
    RangeOrPrepareRenamePlaceholderOrPrepareRenameDefaultBehaviorOrNull;
pub type ExecuteCommandResponse = LSPAnyOrNull;
pub type ApplyWorkspaceEditResponse = ApplyWorkspaceEditResultOrNull;
pub type CustomInitializeAPISessionResponse = InitializeAPISessionResultOrNull;
pub type CustomProjectInfoResponse = ProjectInfoResultOrNull;
pub type CustomTextDocumentSourceDefinitionResponse = LocationOrLocationsOrDefinitionLinksOrNull;
pub type CustomMultiDocumentHighlightResponse = MultiDocumentHighlightsOrNull;
pub type VsOnAutoInsertResponse = VsOnAutoInsertResponseItemOrNull;

object_or_null_wrapper!(InlayHintOrNull, inlay_hint, InlayHint);
object_or_null_wrapper!(
    WorkspaceDiagnosticReportOrNull,
    workspace_diagnostic_report,
    WorkspaceDiagnosticReport
);
object_or_null_wrapper!(
    TextDocumentContentResultOrNull,
    text_document_content_result,
    TextDocumentContentResult
);
object_or_null_wrapper!(
    InitializeAPISessionResultOrNull,
    initialize_api_session_result,
    InitializeAPISessionResult
);
object_or_null_wrapper!(
    ProjectInfoResultOrNull,
    project_info_result,
    ProjectInfoResult
);
object_or_null_wrapper!(CompletionItemOrNull, completion_item, CompletionItem);
object_or_null_wrapper!(CodeActionOrNull, code_action, CodeAction);
object_or_null_wrapper!(WorkspaceSymbolOrNull, workspace_symbol, WorkspaceSymbol);
object_or_null_wrapper!(CodeLensOrNull, code_lens, CodeLens);
object_or_null_wrapper!(DocumentLinkOrNull, document_link, DocumentLink);

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConfigurationParams {
    pub items: Vec<ConfigurationItem>,
}

impl<'de> Deserialize<'de> for ConfigurationParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("items") {
            missing.push("items".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let items = take_non_null_required_field(&mut map, "items")?;
        Ok(Self { items })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ConfigurationItem {
    #[serde(rename = "scopeUri", skip_serializing_if = "Option::is_none")]
    pub scope_uri: Option<DocumentUri>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

impl<'de> Deserialize<'de> for ConfigurationItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let scope_uri = take_non_null_optional_field(&mut map, "scopeUri")?;
        let section = take_non_null_optional_field(&mut map, "section")?;
        Ok(Self { scope_uri, section })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DocumentColorParams {
    #[serde(rename = "workDoneToken", skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(rename = "partialResultToken", skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    #[serde(rename = "textDocument")]
    pub text_document: lsp_types_full::TextDocumentIdentifier,
}

impl<'de> Deserialize<'de> for DocumentColorParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("textDocument") {
            missing.push("textDocument".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        let partial_result_token = take_non_null_optional_field(&mut map, "partialResultToken")?;
        let text_document = take_non_null_required_field(&mut map, "textDocument")?;
        Ok(Self {
            work_done_token,
            partial_result_token,
            text_document,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ColorPresentationParams {
    #[serde(rename = "workDoneToken", skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(rename = "partialResultToken", skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    #[serde(rename = "textDocument")]
    pub text_document: lsp_types_full::TextDocumentIdentifier,
    pub color: lsp_types_full::Color,
    pub range: lsp_types_full::Range,
}

impl<'de> Deserialize<'de> for ColorPresentationParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("textDocument") {
            missing.push("textDocument".to_string());
        }
        if !map.contains_key("color") {
            missing.push("color".to_string());
        }
        if !map.contains_key("range") {
            missing.push("range".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        let partial_result_token = take_non_null_optional_field(&mut map, "partialResultToken")?;
        let text_document = take_non_null_required_field(&mut map, "textDocument")?;
        let color = take_non_null_required_field(&mut map, "color")?;
        let range = take_non_null_required_field(&mut map, "range")?;
        Ok(Self {
            work_done_token,
            partial_result_token,
            text_document,
            color,
            range,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorInformation {
    pub range: Range,
    pub color: Color,
}

impl<'de> Deserialize<'de> for ColorInformation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["range", "color"])?;

        let range = take_non_null_required_field(&mut map, "range")?;
        let color = take_non_null_required_field(&mut map, "color")?;

        Ok(Self { range, color })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorPresentation {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_edit: Option<TextEdit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_text_edits: Option<Vec<TextEdit>>,
}

impl<'de> Deserialize<'de> for ColorPresentation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["label"])?;

        let label = take_non_null_required_field(&mut map, "label")?;
        let text_edit = take_non_null_optional_field(&mut map, "textEdit")?;
        let additional_text_edits = take_non_null_optional_field(&mut map, "additionalTextEdits")?;

        Ok(Self {
            label,
            text_edit,
            additional_text_edits,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FoldingRangeParams {
    #[serde(rename = "workDoneToken", skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(rename = "partialResultToken", skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    #[serde(rename = "textDocument")]
    pub text_document: lsp_types_full::TextDocumentIdentifier,
}

impl<'de> Deserialize<'de> for FoldingRangeParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("textDocument") {
            missing.push("textDocument".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        let partial_result_token = take_non_null_optional_field(&mut map, "partialResultToken")?;
        let text_document = take_non_null_required_field(&mut map, "textDocument")?;
        Ok(Self {
            work_done_token,
            partial_result_token,
            text_document,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ShowMessageRequestParams {
    #[serde(rename = "type")]
    pub typ: MessageType,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<lsp_types_full::MessageActionItem>>,
}

impl<'de> Deserialize<'de> for ShowMessageRequestParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("type") {
            missing.push("type".to_string());
        }
        if !map.contains_key("message") {
            missing.push("message".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let typ = take_non_null_required_field(&mut map, "type")?;
        let message = take_non_null_required_field(&mut map, "message")?;
        let actions = take_non_null_optional_field(&mut map, "actions")?;
        Ok(Self {
            typ,
            message,
            actions,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverParams {
    pub text_document: lsp_types_full::TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbosity_level: Option<i32>,
}

impl<'de> Deserialize<'de> for HoverParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["textDocument", "position"])?;

        let text_document = take_non_null_required_field(&mut map, "textDocument")?;
        let position = take_non_null_required_field(&mut map, "position")?;
        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        let verbosity_level = take_non_null_optional_field(&mut map, "verbosityLevel")?;
        Ok(Self {
            text_document,
            position,
            work_done_token,
            verbosity_level,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareRenameParams {
    pub text_document: lsp_types_full::TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
}

impl<'de> Deserialize<'de> for PrepareRenameParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["textDocument", "position"])?;

        let text_document = take_non_null_required_field(&mut map, "textDocument")?;
        let position = take_non_null_required_field(&mut map, "position")?;
        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        Ok(Self {
            text_document,
            position,
            work_done_token,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationParams {
    pub text_document: lsp_types_full::TextDocumentIdentifier,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
}

impl<'de> Deserialize<'de> for ImplementationParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_text_document_position_progress_params(deserializer)
    }
}

pub type TypeDefinitionParams = ImplementationParams;
pub type DeclarationParams = ImplementationParams;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DidChangeConfigurationParams {
    pub settings: LSPAny,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct InitializedParams {}

impl<'de> Deserialize<'de> for InitializedParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(_) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };
        Ok(Self {})
    }
}

pub const MarkupKindPlainText: MarkupKind = MarkupKind::PlainText;
pub const ErrorCodeParseError: ErrorCode = -32700;
pub const ErrorCodeInvalidRequest: ErrorCode = -32600;
pub const ErrorCodeMethodNotFound: ErrorCode = -32601;
pub const ErrorCodeInvalidParams: ErrorCode = -32602;
pub const ErrorCodeInternalError: ErrorCode = -32603;
pub const ErrorCodeServerNotInitialized: ErrorCode = -32002;
pub const ErrorCodeUnknownErrorCode: ErrorCode = -32001;
pub const ErrorCodeRequestFailed: ErrorCode = -32803;
pub const ErrorCodeServerCancelled: ErrorCode = -32802;
pub const ErrorCodeContentModified: ErrorCode = -32801;
pub const ErrorCodeRequestCancelled: ErrorCode = -32800;
pub const MethodWindowShowMessage: &str = "window/showMessage";
pub const MethodWindowLogMessage: &str = "window/logMessage";
pub const METHOD_WINDOW_LOG_MESSAGE: &str = "window/logMessage";
pub const MethodInitialize: &str = "initialize";
pub const MethodInitialized: &str = "initialized";
pub const MethodShutdown: &str = "shutdown";
pub const MethodExit: &str = "exit";
pub const MethodWorkspaceWorkspaceFolders: &str = "workspace/workspaceFolders";
pub const MethodWorkspaceConfiguration: &str = "workspace/configuration";
pub const MethodWorkspaceDidChangeConfiguration: &str = "workspace/didChangeConfiguration";
pub const MethodCancelRequest: &str = "$/cancelRequest";
pub const MethodProgress: &str = "$/progress";
pub const MethodSetTrace: &str = "$/setTrace";
pub const MethodLogTrace: &str = "$/logTrace";
pub const MethodCustomRunGC: &str = "custom/runGC";
pub const MethodCustomSaveHeapProfile: &str = "custom/saveHeapProfile";
pub const MethodCustomSaveAllocProfile: &str = "custom/saveAllocProfile";
pub const MethodCustomStartCPUProfile: &str = "custom/startCPUProfile";
pub const MethodCustomStopCPUProfile: &str = "custom/stopCPUProfile";
pub const MethodCustomInitializeAPISession: &str = "custom/initializeAPISession";
pub const MethodCustomProjectInfo: &str = "custom/projectInfo";
pub const MethodCustomTextDocumentSourceDefinition: &str = "custom/textDocument/sourceDefinition";
pub const MethodCustomTextDocumentMultiDocumentHighlight: &str =
    "custom/textDocument/multiDocumentHighlight";
pub const MethodTelemetryEvent: &str = "telemetry/event";
pub const MethodTextDocumentDidOpen: &str = "textDocument/didOpen";
pub const MethodTextDocumentDidChange: &str = "textDocument/didChange";
pub const MethodTextDocumentDidClose: &str = "textDocument/didClose";
pub const MethodTextDocumentDidSave: &str = "textDocument/didSave";
pub const MethodTextDocumentWillSave: &str = "textDocument/willSave";
pub const MethodWorkspaceDidChangeWatchedFiles: &str = "workspace/didChangeWatchedFiles";
pub const MethodTextDocumentPublishDiagnostics: &str = "textDocument/publishDiagnostics";
pub const MethodTextDocumentImplementation: &str = "textDocument/implementation";
pub const MethodTextDocumentTypeDefinition: &str = "textDocument/typeDefinition";
pub const MethodTextDocumentDocumentColor: &str = "textDocument/documentColor";
pub const MethodTextDocumentColorPresentation: &str = "textDocument/colorPresentation";
pub const MethodTextDocumentFoldingRange: &str = "textDocument/foldingRange";
pub const MethodWorkspaceFoldingRangeRefresh: &str = "workspace/foldingRange/refresh";
pub const MethodTextDocumentDeclaration: &str = "textDocument/declaration";
pub const MethodTextDocumentSelectionRange: &str = "textDocument/selectionRange";
pub const MethodTextDocumentPrepareCallHierarchy: &str = "textDocument/prepareCallHierarchy";
pub const MethodCallHierarchyIncomingCalls: &str = "callHierarchy/incomingCalls";
pub const MethodCallHierarchyOutgoingCalls: &str = "callHierarchy/outgoingCalls";
pub const MethodTextDocumentSemanticTokensFull: &str = "textDocument/semanticTokens/full";
pub const MethodTextDocumentSemanticTokensFullDelta: &str =
    "textDocument/semanticTokens/full/delta";
pub const MethodTextDocumentSemanticTokensRange: &str = "textDocument/semanticTokens/range";
pub const MethodTextDocumentWillSaveWaitUntil: &str = "textDocument/willSaveWaitUntil";
pub const MethodTextDocumentCompletion: &str = "textDocument/completion";
pub const MethodTextDocumentHover: &str = "textDocument/hover";
pub const MethodTextDocumentSignatureHelp: &str = "textDocument/signatureHelp";
pub const MethodTextDocumentDefinition: &str = "textDocument/definition";
pub const MethodTextDocumentReferences: &str = "textDocument/references";
pub const MethodTextDocumentDocumentHighlight: &str = "textDocument/documentHighlight";
pub const MethodTextDocumentDocumentSymbol: &str = "textDocument/documentSymbol";
pub const MethodTextDocumentCodeAction: &str = "textDocument/codeAction";
pub const MethodWorkspaceSymbol: &str = "workspace/symbol";
pub const MethodTextDocumentCodeLens: &str = "textDocument/codeLens";
pub const MethodTextDocumentDocumentLink: &str = "textDocument/documentLink";
pub const MethodTextDocumentFormatting: &str = "textDocument/formatting";
pub const MethodTextDocumentRangeFormatting: &str = "textDocument/rangeFormatting";
pub const MethodTextDocumentRangesFormatting: &str = "textDocument/rangesFormatting";
pub const MethodTextDocumentOnTypeFormatting: &str = "textDocument/onTypeFormatting";
pub const MethodTextDocumentRename: &str = "textDocument/rename";
pub const MethodTextDocumentPrepareRename: &str = "textDocument/prepareRename";
pub const MethodWorkspaceExecuteCommand: &str = "workspace/executeCommand";
pub const MethodWorkspaceApplyEdit: &str = "workspace/applyEdit";
pub const MethodWorkspaceSemanticTokensRefresh: &str = "workspace/semanticTokens/refresh";
pub const MethodWindowShowDocument: &str = "window/showDocument";
pub const MethodTextDocumentLinkedEditingRange: &str = "textDocument/linkedEditingRange";
pub const MethodTextDocumentMoniker: &str = "textDocument/moniker";
pub const MethodTextDocumentPrepareTypeHierarchy: &str = "textDocument/prepareTypeHierarchy";
pub const MethodTypeHierarchySupertypes: &str = "typeHierarchy/supertypes";
pub const MethodTypeHierarchySubtypes: &str = "typeHierarchy/subtypes";
pub const MethodWorkspaceInlineValueRefresh: &str = "workspace/inlineValue/refresh";
pub const MethodTextDocumentInlineValue: &str = "textDocument/inlineValue";
pub const MethodTextDocumentInlayHint: &str = "textDocument/inlayHint";
pub const MethodWorkspaceInlayHintRefresh: &str = "workspace/inlayHint/refresh";
pub const MethodTextDocumentDiagnostic: &str = "textDocument/diagnostic";
pub const MethodWorkspaceDiagnostic: &str = "workspace/diagnostic";
pub const MethodWorkspaceDiagnosticRefresh: &str = "workspace/diagnostic/refresh";
pub const MethodTextDocumentInlineCompletion: &str = "textDocument/inlineCompletion";
pub const MethodClientRegisterCapability: &str = "client/registerCapability";
pub const MethodClientUnregisterCapability: &str = "client/unregisterCapability";
pub const MethodWorkspaceTextDocumentContent: &str = "workspace/textDocumentContent";
pub const MethodWorkspaceTextDocumentContentRefresh: &str = "workspace/textDocumentContent/refresh";
pub const MethodWorkspaceCodeLensRefresh: &str = "workspace/codeLens/refresh";
pub const MethodWindowWorkDoneProgressCreate: &str = "window/workDoneProgress/create";
pub const MethodWindowShowMessageRequest: &str = "window/showMessageRequest";
pub const MethodCompletionItemResolve: &str = "completionItem/resolve";
pub const MethodCodeActionResolve: &str = "codeAction/resolve";
pub const MethodWorkspaceSymbolResolve: &str = "workspaceSymbol/resolve";
pub const MethodCodeLensResolve: &str = "codeLens/resolve";
pub const MethodDocumentLinkResolve: &str = "documentLink/resolve";
pub const MethodInlayHintResolve: &str = "inlayHint/resolve";
pub const MethodWorkspaceWillCreateFiles: &str = "workspace/willCreateFiles";
pub const MethodWorkspaceWillRenameFiles: &str = "workspace/willRenameFiles";
pub const MethodWorkspaceWillDeleteFiles: &str = "workspace/willDeleteFiles";
pub const MethodWorkspaceDidChangeWorkspaceFolders: &str = "workspace/didChangeWorkspaceFolders";
pub const MethodWindowWorkDoneProgressCancel: &str = "window/workDoneProgress/cancel";
pub const MethodTextDocumentVSOnAutoInsert: &str = "textDocument/_vs_onAutoInsert";
pub const MethodWorkspaceDidCreateFiles: &str = "workspace/didCreateFiles";
pub const MethodWorkspaceDidRenameFiles: &str = "workspace/didRenameFiles";
pub const MethodWorkspaceDidDeleteFiles: &str = "workspace/didDeleteFiles";

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct AutoImportFixKind(pub i32);

#[allow(non_upper_case_globals)]
impl AutoImportFixKind {
    // Augment an existing namespace import.
    pub const UseNamespace: Self = Self(0);
    // Insert into an existing import declaration.
    pub const AddToExisting: Self = Self(1);
    // Create a fresh import statement.
    pub const AddNew: Self = Self(2);
    // Promote a type-only import when necessary.
    pub const PromoteTypeOnly: Self = Self(3);
}

impl Serialize for AutoImportFixKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.0)
    }
}

impl<'de> Deserialize<'de> for AutoImportFixKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(i32::deserialize(deserializer)?))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ImportKind(pub i32);

#[allow(non_upper_case_globals)]
impl ImportKind {
    // Adds a named import.
    pub const Named: Self = Self(0);
    // Adds a default import.
    pub const Default: Self = Self(1);
    // Adds a namespace import.
    pub const Namespace: Self = Self(2);
    // Adds a CommonJS import assignment.
    pub const CommonJS: Self = Self(3);
}

impl Serialize for ImportKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.0)
    }
}

impl<'de> Deserialize<'de> for ImportKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(i32::deserialize(deserializer)?))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct AddAsTypeOnly(pub i32);

#[allow(non_upper_case_globals)]
impl AddAsTypeOnly {
    // Import may be marked type-only if needed.
    pub const Allowed: Self = Self(1);
    // Import must be marked type-only.
    pub const Required: Self = Self(2);
    // Import cannot be marked type-only.
    pub const NotAllowed: Self = Self(4);
}

impl Serialize for AddAsTypeOnly {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.0)
    }
}

impl<'de> Deserialize<'de> for AddAsTypeOnly {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(i32::deserialize(deserializer)?))
    }
}

// AutoImportFix contains information about an auto-import suggestion.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoImportFix {
    #[serde(default, skip_serializing_if = "is_default_auto_import_fix_kind")]
    pub kind: AutoImportFixKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub import_kind: ImportKind,
    #[serde(default, skip_serializing_if = "is_false")]
    pub use_require: bool,
    pub add_as_type_only: AddAsTypeOnly,
    // The module specifier for this auto-import.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub module_specifier: String,
    // Index of the import to modify when adding to an existing import declaration.
    pub import_index: i32,
    #[serde(
        default,
        deserialize_with = "deserialize_non_null_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub usage_position: Option<Position>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub namespace_prefix: String,
}

fn is_default_auto_import_fix_kind(value: &AutoImportFixKind) -> bool {
    *value == AutoImportFixKind::UseNamespace
}

// CompletionItemData is preserved on a CompletionItem between CompletionRequest and CompletionResolveRequest.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemData {
    // The file name where the completion was requested.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file_name: String,
    // The position where the completion was requested.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub position: i32,
    // Special source value for disambiguation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    // The name of the completion item.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    // Auto-import data for this completion item.
    #[serde(
        default,
        deserialize_with = "deserialize_non_null_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_import: Option<AutoImportFix>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensData {
    // The kind of the code lens ("references" or "implementations").
    pub kind: CodeLensKind,
    // The document in which the code lens and its range are located.
    pub uri: DocumentUri,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

fn is_zero_f64(value: &f64) -> bool {
    *value == 0.0
}

fn deserialize_non_null_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_null() {
        return Err(serde::de::Error::custom(err_null("usagePosition")));
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IntegerOrString {
    pub integer: Option<i32>,
    pub string: Option<String>,
}

impl From<i32> for IntegerOrString {
    fn from(value: i32) -> Self {
        Self {
            integer: Some(value),
            string: None,
        }
    }
}

impl From<String> for IntegerOrString {
    fn from(value: String) -> Self {
        Self {
            integer: None,
            string: Some(value),
        }
    }
}

impl Serialize for IntegerOrString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of IntegerOrString should be set",
            bool_to_int(self.integer.is_some()) + bool_to_int(self.string.is_some()),
        );
        if let Some(value) = self.integer {
            return serializer.serialize_i32(value);
        }
        if let Some(value) = &self.string {
            return serializer.serialize_str(value);
        }
        unreachable!()
    }
}

impl<'de> Deserialize<'de> for IntegerOrString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = IntegerOrString;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("integer or string")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(IntegerOrString::from(value as i32))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(IntegerOrString::from(value as i32))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(IntegerOrString::from(value.to_string()))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

pub fn unmarshal_params(method: &Method, params: json::Value) -> Result<serde_json::Value, String> {
    match method.as_str() {
        MethodWindowWorkDoneProgressCreate => {
            validate_value::<WorkDoneProgressCreateParams>(&params)?
        }
        MethodInitialized => validate_value::<InitializedParams>(&params)?,
        MethodWorkspaceDidChangeConfiguration => {
            validate_value::<DidChangeConfigurationParams>(&params)?
        }
        MethodWorkspaceWorkspaceFolders => validate_null(method, &params)?,
        MethodWorkspaceConfiguration => validate_value::<ConfigurationParams>(&params)?,
        MethodShutdown | MethodExit => validate_null(method, &params)?,
        MethodCustomRunGC => validate_null(method, &params)?,
        MethodCustomSaveHeapProfile
        | MethodCustomSaveAllocProfile
        | MethodCustomStartCPUProfile => validate_value::<ProfileParams>(&params)?,
        MethodCustomStopCPUProfile => validate_null(method, &params)?,
        MethodCustomInitializeAPISession => validate_value::<InitializeAPISessionParams>(&params)?,
        MethodCustomProjectInfo => validate_value::<ProjectInfoParams>(&params)?,
        MethodCustomTextDocumentSourceDefinition => {
            validate_value::<TextDocumentPositionParams>(&params)?
        }
        MethodCustomTextDocumentMultiDocumentHighlight => {
            validate_value::<MultiDocumentHighlightParams>(&params)?
        }
        MethodWindowShowMessage => validate_value::<ShowMessageParams>(&params)?,
        MethodWindowLogMessage => validate_value::<LogMessageParams>(&params)?,
        MethodTelemetryEvent => validate_value::<TelemetryEvent>(&params)?,
        MethodTextDocumentDidOpen => validate_value::<DidOpenTextDocumentParams>(&params)?,
        MethodTextDocumentDidChange => validate_value::<DidChangeTextDocumentParams>(&params)?,
        MethodTextDocumentDidClose => validate_value::<DidCloseTextDocumentParams>(&params)?,
        MethodTextDocumentDidSave => validate_value::<DidSaveTextDocumentParams>(&params)?,
        MethodTextDocumentWillSave => validate_value::<WillSaveTextDocumentParams>(&params)?,
        MethodWorkspaceDidChangeWatchedFiles => {
            validate_value::<DidChangeWatchedFilesParams>(&params)?
        }
        MethodTextDocumentPublishDiagnostics => {
            validate_value::<PublishDiagnosticsParams>(&params)?
        }
        MethodTextDocumentImplementation => validate_value::<ImplementationParams>(&params)?,
        MethodTextDocumentTypeDefinition => validate_value::<TypeDefinitionParams>(&params)?,
        MethodTextDocumentDocumentColor => validate_value::<DocumentColorParams>(&params)?,
        MethodTextDocumentColorPresentation => validate_value::<ColorPresentationParams>(&params)?,
        MethodTextDocumentFoldingRange => validate_value::<FoldingRangeParams>(&params)?,
        MethodTextDocumentDeclaration => validate_value::<DeclarationParams>(&params)?,
        MethodTextDocumentSelectionRange => validate_value_rejecting_nulls::<SelectionRangeParams>(
            &params,
            &["workDoneToken", "partialResultToken", "positions"],
        )?,
        MethodTextDocumentPrepareCallHierarchy => validate_value_rejecting_nulls::<
            CallHierarchyPrepareParams,
        >(&params, &["workDoneToken"])?,
        MethodCallHierarchyIncomingCalls => {
            validate_value::<CallHierarchyIncomingCallsParams>(&params)?
        }
        MethodCallHierarchyOutgoingCalls => {
            validate_value::<CallHierarchyOutgoingCallsParams>(&params)?
        }
        MethodTextDocumentSemanticTokensFull => {
            validate_value_rejecting_nulls::<SemanticTokensParams>(
                &params,
                &["workDoneToken", "partialResultToken"],
            )?
        }
        MethodTextDocumentSemanticTokensFullDelta => {
            validate_value_rejecting_nulls::<SemanticTokensDeltaParams>(
                &params,
                &["workDoneToken", "partialResultToken"],
            )?
        }
        MethodTextDocumentSemanticTokensRange => {
            validate_value_rejecting_nulls::<SemanticTokensRangeParams>(
                &params,
                &["workDoneToken", "partialResultToken"],
            )?
        }
        MethodWorkspaceFoldingRangeRefresh
        | MethodWorkspaceSemanticTokensRefresh
        | MethodWorkspaceInlineValueRefresh
        | MethodWorkspaceInlayHintRefresh
        | MethodWorkspaceDiagnosticRefresh
        | MethodWorkspaceCodeLensRefresh => validate_null(method, &params)?,
        MethodWindowShowDocument => validate_value::<ShowDocumentParams>(&params)?,
        MethodTextDocumentLinkedEditingRange => {
            validate_value_rejecting_nulls::<LinkedEditingRangeParams>(&params, &["workDoneToken"])?
        }
        MethodTextDocumentMoniker => validate_value_rejecting_nulls::<MonikerParams>(
            &params,
            &["workDoneToken", "partialResultToken"],
        )?,
        MethodTextDocumentPrepareTypeHierarchy => validate_value_rejecting_nulls::<
            TypeHierarchyPrepareParams,
        >(&params, &["workDoneToken"])?,
        MethodTypeHierarchySupertypes => validate_value::<TypeHierarchySupertypesParams>(&params)?,
        MethodTypeHierarchySubtypes => validate_value::<TypeHierarchySubtypesParams>(&params)?,
        MethodTextDocumentInlineValue => validate_value_rejecting_nulls::<InlineValueParams>(
            &params,
            &["workDoneToken", "context"],
        )?,
        MethodTextDocumentInlayHint => {
            validate_value_rejecting_nulls::<InlayHintParams>(&params, &["workDoneToken"])?
        }
        MethodInlayHintResolve => validate_value::<InlayHint>(&params)?,
        MethodTextDocumentDiagnostic => validate_value_rejecting_nulls::<DocumentDiagnosticParams>(
            &params,
            &[
                "workDoneToken",
                "partialResultToken",
                "identifier",
                "previousResultId",
            ],
        )?,
        MethodWorkspaceDiagnostic => validate_value::<WorkspaceDiagnosticParams>(&params)?,
        MethodTextDocumentInlineCompletion => validate_value::<InlineCompletionParams>(&params)?,
        MethodClientRegisterCapability => validate_value::<RegistrationParams>(&params)?,
        MethodClientUnregisterCapability => validate_value::<UnregistrationParams>(&params)?,
        MethodWorkspaceTextDocumentContent => validate_value::<TextDocumentContentParams>(&params)?,
        MethodWorkspaceTextDocumentContentRefresh => {
            validate_value::<TextDocumentContentRefreshParams>(&params)?
        }
        MethodTextDocumentWillSaveWaitUntil => {
            validate_value::<WillSaveTextDocumentParams>(&params)?
        }
        MethodInitialize => validate_value::<InitializeParams>(&params)?,
        MethodWindowShowMessageRequest => validate_value::<ShowMessageRequestParams>(&params)?,
        MethodTextDocumentCompletion => validate_value_rejecting_nulls::<CompletionParams>(
            &params,
            &["workDoneToken", "partialResultToken", "context"],
        )?,
        MethodCompletionItemResolve => validate_value::<CompletionItem>(&params)?,
        MethodTextDocumentHover => validate_value::<HoverParams>(&params)?,
        MethodTextDocumentSignatureHelp => validate_value_rejecting_nulls::<SignatureHelpParams>(
            &params,
            &["workDoneToken", "context"],
        )?,
        MethodTextDocumentDefinition => validate_value_rejecting_nulls::<DefinitionParams>(
            &params,
            &["workDoneToken", "partialResultToken"],
        )?,
        MethodTextDocumentReferences => validate_value_rejecting_nulls::<ReferenceParams>(
            &params,
            &["workDoneToken", "partialResultToken", "context"],
        )?,
        MethodTextDocumentDocumentHighlight => validate_value_rejecting_nulls::<
            DocumentHighlightParams,
        >(
            &params, &["workDoneToken", "partialResultToken"]
        )?,
        MethodTextDocumentDocumentSymbol => validate_value_rejecting_nulls::<DocumentSymbolParams>(
            &params,
            &["workDoneToken", "partialResultToken"],
        )?,
        MethodTextDocumentCodeAction => validate_value_rejecting_nulls::<CodeActionParams>(
            &params,
            &["workDoneToken", "partialResultToken", "context"],
        )?,
        MethodCodeActionResolve => validate_value::<CodeActionResolveParams>(&params)?,
        MethodWorkspaceSymbol => validate_value::<WorkspaceSymbolParams>(&params)?,
        MethodWorkspaceSymbolResolve => validate_value::<WorkspaceSymbolResolveParams>(&params)?,
        MethodTextDocumentCodeLens => validate_value_rejecting_nulls::<CodeLensParams>(
            &params,
            &["workDoneToken", "partialResultToken"],
        )?,
        MethodCodeLensResolve => validate_value::<CodeLensResolveParams>(&params)?,
        MethodTextDocumentDocumentLink => validate_value_rejecting_nulls::<DocumentLinkParams>(
            &params,
            &["workDoneToken", "partialResultToken"],
        )?,
        MethodDocumentLinkResolve => validate_value::<DocumentLinkResolveParams>(&params)?,
        MethodTextDocumentFormatting => validate_value_rejecting_nulls::<DocumentFormattingParams>(
            &params,
            &["workDoneToken", "options"],
        )?,
        MethodTextDocumentRangeFormatting => validate_value_rejecting_nulls::<
            DocumentRangeFormattingParams,
        >(&params, &["workDoneToken", "options"])?,
        MethodTextDocumentRangesFormatting => {
            validate_value::<DocumentRangesFormattingParams>(&params)?
        }
        MethodTextDocumentOnTypeFormatting => {
            validate_value_rejecting_nulls::<DocumentOnTypeFormattingParams>(&params, &["options"])?
        }
        MethodTextDocumentRename => {
            validate_value_rejecting_nulls::<RenameParams>(&params, &["workDoneToken"])?
        }
        MethodTextDocumentPrepareRename => validate_value::<PrepareRenameParams>(&params)?,
        MethodWorkspaceExecuteCommand => validate_value::<ExecuteCommandParams>(&params)?,
        MethodWorkspaceApplyEdit => validate_value::<ApplyWorkspaceEditParams>(&params)?,
        MethodWorkspaceWillCreateFiles => validate_value::<CreateFilesParams>(&params)?,
        MethodWorkspaceWillRenameFiles => validate_value::<RenameFilesParams>(&params)?,
        MethodWorkspaceWillDeleteFiles => validate_value::<DeleteFilesParams>(&params)?,
        MethodWorkspaceDidChangeWorkspaceFolders => {
            validate_value::<DidChangeWorkspaceFoldersParams>(&params)?
        }
        MethodWindowWorkDoneProgressCancel => {
            validate_value::<WorkDoneProgressCancelParams>(&params)?
        }
        MethodTextDocumentVSOnAutoInsert => validate_value::<VsOnAutoInsertParams>(&params)?,
        MethodWorkspaceDidCreateFiles => validate_value::<CreateFilesParams>(&params)?,
        MethodWorkspaceDidRenameFiles => validate_value::<RenameFilesParams>(&params)?,
        MethodWorkspaceDidDeleteFiles => validate_value::<DeleteFilesParams>(&params)?,
        MethodCancelRequest => validate_value::<CancelParams>(&params)?,
        MethodSetTrace => validate_value::<SetTraceParams>(&params)?,
        MethodLogTrace => validate_value::<LogTraceParams>(&params)?,
        MethodProgress => validate_value::<ProgressParams>(&params)?,
        _ => {}
    }
    Ok(params)
}

pub fn unmarshal_result(
    method: Method,
    result: serde_json::Value,
) -> Result<serde_json::Value, String> {
    match method.as_str() {
        MethodWindowWorkDoneProgressCreate => {
            validate_value::<WorkDoneProgressCreateResponse>(&result)?
        }
        MethodTextDocumentImplementation => validate_value::<ImplementationResponse>(&result)?,
        MethodTextDocumentTypeDefinition => validate_value::<TypeDefinitionResponse>(&result)?,
        MethodWorkspaceWorkspaceFolders => validate_value::<WorkspaceFoldersResponse>(&result)?,
        MethodWorkspaceConfiguration => validate_value::<ConfigurationResponse>(&result)?,
        MethodTextDocumentDocumentColor => validate_value::<DocumentColorResponse>(&result)?,
        MethodTextDocumentColorPresentation => {
            validate_value::<ColorPresentationResponse>(&result)?
        }
        MethodTextDocumentFoldingRange => validate_value::<FoldingRangeResponse>(&result)?,
        MethodWorkspaceFoldingRangeRefresh => {
            validate_value::<FoldingRangeRefreshResponse>(&result)?
        }
        MethodTextDocumentDeclaration => validate_value::<DeclarationResponse>(&result)?,
        MethodTextDocumentSelectionRange => validate_value::<SelectionRangeResponse>(&result)?,
        MethodTextDocumentPrepareCallHierarchy => {
            validate_value::<CallHierarchyPrepareResponse>(&result)?
        }
        MethodCallHierarchyIncomingCalls => {
            validate_value::<CallHierarchyIncomingCallsResponse>(&result)?
        }
        MethodCallHierarchyOutgoingCalls => {
            validate_value::<CallHierarchyOutgoingCallsResponse>(&result)?
        }
        MethodTextDocumentSemanticTokensFull => validate_value::<SemanticTokensResponse>(&result)?,
        MethodTextDocumentSemanticTokensFullDelta => {
            validate_value::<SemanticTokensDeltaResponse>(&result)?
        }
        MethodTextDocumentSemanticTokensRange => {
            validate_value::<SemanticTokensRangeResponse>(&result)?
        }
        MethodWorkspaceSemanticTokensRefresh => {
            validate_value::<SemanticTokensRefreshResponse>(&result)?
        }
        MethodWindowShowDocument => validate_value::<ShowDocumentResponse>(&result)?,
        MethodTextDocumentLinkedEditingRange => {
            validate_value::<LinkedEditingRangeResponse>(&result)?
        }
        MethodWorkspaceWillCreateFiles => validate_value::<WillCreateFilesResponse>(&result)?,
        MethodWorkspaceWillRenameFiles => validate_value::<WillRenameFilesResponse>(&result)?,
        MethodWorkspaceWillDeleteFiles => validate_value::<WillDeleteFilesResponse>(&result)?,
        MethodTextDocumentMoniker => validate_value::<MonikerResponse>(&result)?,
        MethodTextDocumentPrepareTypeHierarchy => {
            validate_value::<TypeHierarchyPrepareResponse>(&result)?
        }
        MethodTypeHierarchySupertypes => {
            validate_value::<TypeHierarchySupertypesResponse>(&result)?
        }
        MethodTypeHierarchySubtypes => validate_value::<TypeHierarchySubtypesResponse>(&result)?,
        MethodTextDocumentInlineValue => validate_value::<InlineValueResponse>(&result)?,
        MethodWorkspaceInlineValueRefresh => validate_value::<InlineValueRefreshResponse>(&result)?,
        MethodTextDocumentInlayHint => validate_value::<InlayHintResponse>(&result)?,
        MethodInlayHintResolve => validate_value::<InlayHintResolveResponse>(&result)?,
        MethodWorkspaceInlayHintRefresh => validate_value::<InlayHintRefreshResponse>(&result)?,
        MethodTextDocumentDiagnostic => validate_value::<DocumentDiagnosticResponse>(&result)?,
        MethodWorkspaceDiagnostic => validate_value::<WorkspaceDiagnosticResponse>(&result)?,
        MethodWorkspaceDiagnosticRefresh => validate_value::<DiagnosticRefreshResponse>(&result)?,
        MethodTextDocumentInlineCompletion => validate_value::<InlineCompletionResponse>(&result)?,
        MethodWorkspaceTextDocumentContent => {
            validate_value::<TextDocumentContentResponse>(&result)?
        }
        MethodWorkspaceTextDocumentContentRefresh => {
            validate_value::<TextDocumentContentRefreshResponse>(&result)?
        }
        MethodClientRegisterCapability => validate_value::<RegistrationResponse>(&result)?,
        MethodClientUnregisterCapability => validate_value::<UnregistrationResponse>(&result)?,
        MethodInitialize => validate_value::<InitializeResponse>(&result)?,
        MethodCustomRunGC => validate_value::<RunGCResponse>(&result)?,
        MethodShutdown => validate_value::<ShutdownResponse>(&result)?,
        MethodWindowShowMessageRequest => validate_value::<ShowMessageResponse>(&result)?,
        MethodTextDocumentWillSaveWaitUntil => {
            validate_value::<WillSaveTextDocumentWaitUntilResponse>(&result)?
        }
        MethodTextDocumentCompletion => validate_value::<CompletionResponse>(&result)?,
        MethodCompletionItemResolve => validate_value::<CompletionResolveResponse>(&result)?,
        MethodTextDocumentHover => validate_value::<HoverResponse>(&result)?,
        MethodTextDocumentSignatureHelp => validate_value::<SignatureHelpResponse>(&result)?,
        MethodTextDocumentDefinition => validate_value::<DefinitionResponse>(&result)?,
        MethodTextDocumentReferences => validate_value::<ReferencesResponse>(&result)?,
        MethodTextDocumentDocumentHighlight => {
            validate_value::<DocumentHighlightResponse>(&result)?
        }
        MethodTextDocumentDocumentSymbol => validate_value::<DocumentSymbolResponse>(&result)?,
        MethodTextDocumentCodeAction => validate_value::<CodeActionResponse>(&result)?,
        MethodCodeActionResolve => validate_value::<CodeActionResolveResponse>(&result)?,
        MethodWorkspaceSymbol => validate_value::<WorkspaceSymbolResponse>(&result)?,
        MethodWorkspaceSymbolResolve => validate_value::<WorkspaceSymbolResolveResponse>(&result)?,
        MethodTextDocumentCodeLens => validate_value::<CodeLensResponse>(&result)?,
        MethodCodeLensResolve => validate_value::<CodeLensResolveResponse>(&result)?,
        MethodWorkspaceCodeLensRefresh => validate_value::<CodeLensRefreshResponse>(&result)?,
        MethodTextDocumentDocumentLink => validate_value::<DocumentLinkResponse>(&result)?,
        MethodDocumentLinkResolve => validate_value::<DocumentLinkResolveResponse>(&result)?,
        MethodTextDocumentFormatting => validate_value::<DocumentFormattingResponse>(&result)?,
        MethodTextDocumentRangeFormatting => {
            validate_value::<DocumentRangeFormattingResponse>(&result)?
        }
        MethodTextDocumentRangesFormatting => {
            validate_value::<DocumentRangesFormattingResponse>(&result)?
        }
        MethodTextDocumentOnTypeFormatting => {
            validate_value::<DocumentOnTypeFormattingResponse>(&result)?
        }
        MethodTextDocumentRename => validate_value::<RenameResponse>(&result)?,
        MethodTextDocumentPrepareRename => validate_value::<PrepareRenameResponse>(&result)?,
        MethodWorkspaceExecuteCommand => validate_value::<ExecuteCommandResponse>(&result)?,
        MethodWorkspaceApplyEdit => validate_value::<ApplyWorkspaceEditResponse>(&result)?,
        MethodCustomSaveHeapProfile => validate_value::<SaveHeapProfileResponse>(&result)?,
        MethodCustomSaveAllocProfile => validate_value::<SaveAllocProfileResponse>(&result)?,
        MethodCustomStartCPUProfile => validate_value::<StartCPUProfileResponse>(&result)?,
        MethodCustomStopCPUProfile => validate_value::<StopCPUProfileResponse>(&result)?,
        MethodCustomInitializeAPISession => {
            validate_value::<CustomInitializeAPISessionResponse>(&result)?
        }
        MethodCustomProjectInfo => validate_value::<CustomProjectInfoResponse>(&result)?,
        MethodCustomTextDocumentSourceDefinition => {
            validate_value::<CustomTextDocumentSourceDefinitionResponse>(&result)?
        }
        MethodCustomTextDocumentMultiDocumentHighlight => {
            validate_value::<CustomMultiDocumentHighlightResponse>(&result)?
        }
        MethodTextDocumentVSOnAutoInsert => validate_value::<VsOnAutoInsertResponse>(&result)?,
        _ => {}
    }
    Ok(result)
}

fn validate_value<T>(value: &serde_json::Value) -> Result<(), String>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value::<T>(value.clone())
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn validate_value_rejecting_nulls<T>(
    value: &serde_json::Value,
    non_null_fields: &[&str],
) -> Result<(), String>
where
    T: serde::de::DeserializeOwned,
{
    reject_explicit_null_fields(value, non_null_fields)?;
    validate_value::<T>(value)
}

fn reject_explicit_null_fields(
    value: &serde_json::Value,
    non_null_fields: &[&str],
) -> Result<(), String> {
    let serde_json::Value::Object(map) = value else {
        return Ok(());
    };
    for field in non_null_fields {
        if map.get(*field).is_some_and(serde_json::Value::is_null) {
            return Err(err_null(field));
        }
    }
    Ok(())
}

fn validate_null(method: &str, value: &serde_json::Value) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    Err(format!("expected empty params for {method}, got: {value}"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct MessageType(u32);

impl MessageType {
    pub const Error: Self = Self(1);
    pub const Warning: Self = Self(2);
    pub const Info: Self = Self(3);
    pub const Log: Self = Self(4);
    pub const Debug: Self = Self(5);
}

impl Serialize for MessageType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for MessageType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(u32::deserialize(deserializer)?))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ShowMessageParams {
    #[serde(rename = "type")]
    pub r#type: MessageType,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LogMessageParams {
    #[serde(rename = "type")]
    pub r#type: MessageType,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LogTraceParams {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<String>,
}

impl<'de> Deserialize<'de> for LogTraceParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if !map.contains_key("message") {
            return Err(serde::de::Error::custom(err_missing(&[
                "message".to_string()
            ])));
        }

        let message = serde_json::from_value(
            map.remove("message")
                .expect("message is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let verbose = take_non_null_optional_field(&mut map, "verbose")?;

        Ok(Self { message, verbose })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidSaveTextDocumentParams {
    pub text_document: lsp_types_full::TextDocumentIdentifier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl<'de> Deserialize<'de> for DidSaveTextDocumentParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if !map.contains_key("textDocument") {
            return Err(serde::de::Error::custom(err_missing(&[
                "textDocument".to_string()
            ])));
        }

        let text_document = serde_json::from_value(
            map.remove("textDocument")
                .expect("textDocument is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let text = take_non_null_optional_field(&mut map, "text")?;

        Ok(Self {
            text_document,
            text,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DidChangeWorkspaceFoldersParams {
    pub event: WorkspaceFoldersChangeEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkspaceFoldersChangeEvent {
    pub added: Vec<Option<WorkspaceFolder>>,
    pub removed: Vec<Option<WorkspaceFolder>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceFoldersOrNull {
    pub workspace_folders: Option<Vec<Option<WorkspaceFolder>>>,
}

impl Serialize for WorkspaceFoldersOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(workspace_folders) = &self.workspace_folders {
            return workspace_folders.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for WorkspaceFoldersOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let workspace_folders =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    workspace_folders: Some(workspace_folders),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid WorkspaceFoldersOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkDoneProgressCancelParams {
    pub token: IntegerOrString,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CreateFilesParams {
    pub files: Vec<Option<FileCreate>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileCreate {
    pub uri: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RenameFilesParams {
    pub files: Vec<FileRename>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRename {
    pub old_uri: String,
    pub new_uri: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DeleteFilesParams {
    pub files: Vec<Option<FileDelete>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FileDelete {
    pub uri: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRangesFormattingParams {
    #[serde(
        default,
        deserialize_with = "deserialize_non_null_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub work_done_token: Option<IntegerOrString>,
    pub text_document: lsp_types_full::TextDocumentIdentifier,
    pub ranges: Vec<lsp_types_full::Range>,
    pub options: lsp_types_full::FormattingOptions,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineCompletionParams {
    #[serde(
        default,
        deserialize_with = "deserialize_non_null_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub work_done_token: Option<IntegerOrString>,
    pub text_document: lsp_types_full::TextDocumentIdentifier,
    pub position: lsp_types_full::Position,
    pub context: LSPAny,
}

impl<'de> Deserialize<'de> for InlineCompletionParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("textDocument") {
            missing.push("textDocument".to_string());
        }
        if !map.contains_key("position") {
            missing.push("position".to_string());
        }
        if !map.contains_key("context") {
            missing.push("context".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        let text_document = take_non_null_required_field(&mut map, "textDocument")?;
        let position = take_non_null_required_field(&mut map, "position")?;
        let context = take_non_null_required_field(&mut map, "context")?;

        Ok(Self {
            work_done_token,
            text_document,
            position,
            context,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum InlineCompletionWireResponse {
    Array(Vec<LSPAny>),
    List { items: Vec<LSPAny> },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InlineCompletionListOrItemsOrNull {
    pub inline_completion: Option<InlineCompletionWireResponse>,
}

impl Serialize for InlineCompletionListOrItemsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(inline_completion) = &self.inline_completion {
            return inline_completion.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for InlineCompletionListOrItemsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                let inline_completion =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    inline_completion: Some(inline_completion),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid InlineCompletionListOrItemsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextDocumentContentParams {
    pub uri: DocumentUri,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextDocumentContentRefreshParams {
    pub uri: DocumentUri,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextDocumentContentResult {
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InitializeAPISessionParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipe: Option<String>,
}

impl<'de> Deserialize<'de> for InitializeAPISessionParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let pipe = take_non_null_optional_field(&mut map, "pipe")?;

        Ok(Self { pipe })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeAPISessionResult {
    pub session_id: String,
    pub pipe: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfoParams {
    pub text_document: TextDocumentIdentifier,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfoResult {
    pub config_file_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiDocumentHighlightParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub files_to_search: Vec<DocumentUri>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MultiDocumentHighlight {
    pub uri: DocumentUri,
    pub highlights: Vec<DocumentHighlight>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MultiDocumentHighlightsOrNull {
    pub multi_document_highlights: Option<Vec<Option<MultiDocumentHighlight>>>,
}

impl Serialize for MultiDocumentHighlightsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(multi_document_highlights) = &self.multi_document_highlights {
            return multi_document_highlights.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for MultiDocumentHighlightsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let multi_document_highlights =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    multi_document_highlights: Some(multi_document_highlights),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid MultiDocumentHighlightsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VsOnAutoInsertParams {
    #[serde(rename = "_vs_textDocument")]
    pub vs_text_document: lsp_types_full::TextDocumentIdentifier,
    #[serde(rename = "_vs_position")]
    pub vs_position: lsp_types_full::Position,
    #[serde(rename = "_vs_ch")]
    pub vs_ch: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VsOnAutoInsertResponseItem {
    #[serde(rename = "_vs_textEditFormat")]
    pub vs_text_edit_format: InsertTextFormat,
    #[serde(rename = "_vs_textEdit")]
    pub vs_text_edit: TextEdit,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VsOnAutoInsertResponseItemOrNull {
    pub vs_on_auto_insert_response_item: Option<VsOnAutoInsertResponseItem>,
}

impl Serialize for VsOnAutoInsertResponseItemOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(vs_on_auto_insert_response_item) = &self.vs_on_auto_insert_response_item {
            return vs_on_auto_insert_response_item.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for VsOnAutoInsertResponseItemOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let vs_on_auto_insert_response_item =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    vs_on_auto_insert_response_item: Some(vs_on_auto_insert_response_item),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid VsOnAutoInsertResponseItemOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishDiagnosticsParams {
    pub uri: Uri,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
}

impl<'de> Deserialize<'de> for PublishDiagnosticsParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("uri") {
            missing.push("uri".to_string());
        }
        if !map.contains_key("diagnostics") {
            missing.push("diagnostics".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let uri = serde_json::from_value(
            map.remove("uri")
                .expect("uri is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let diagnostics = serde_json::from_value(
            map.remove("diagnostics")
                .expect("diagnostics is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let version = take_non_null_optional_field(&mut map, "version")?;

        Ok(Self {
            uri,
            diagnostics,
            version,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkDoneProgressCreateParams {
    pub token: IntegerOrString,
}

pub type WorkDoneProgressCreateResponse = Null;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkDoneProgressBegin {
    pub title: String,
    #[serde(default = "work_done_progress_begin_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancellable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u32>,
}

fn work_done_progress_begin_kind() -> String {
    "begin".to_string()
}

impl Default for WorkDoneProgressBegin {
    fn default() -> Self {
        Self {
            kind: work_done_progress_begin_kind(),
            title: String::new(),
            cancellable: None,
            message: None,
            percentage: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkDoneProgressReport {
    #[serde(default = "work_done_progress_report_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancellable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u32>,
}

fn work_done_progress_report_kind() -> String {
    "report".to_string()
}

impl Default for WorkDoneProgressReport {
    fn default() -> Self {
        Self {
            kind: work_done_progress_report_kind(),
            cancellable: None,
            message: None,
            percentage: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkDoneProgressEnd {
    #[serde(default = "work_done_progress_end_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn work_done_progress_end_kind() -> String {
    "end".to_string()
}

impl Default for WorkDoneProgressEnd {
    fn default() -> Self {
        Self {
            kind: work_done_progress_end_kind(),
            message: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkDoneProgressBeginOrReportOrEnd {
    pub begin: Option<WorkDoneProgressBegin>,
    pub report: Option<WorkDoneProgressReport>,
    pub end: Option<WorkDoneProgressEnd>,
}

impl Serialize for WorkDoneProgressBeginOrReportOrEnd {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of WorkDoneProgressBeginOrReportOrEnd should be set",
            bool_to_int(self.begin.is_some())
                + bool_to_int(self.report.is_some())
                + bool_to_int(self.end.is_some()),
        );
        if let Some(value) = &self.begin {
            return value.serialize(serializer);
        }
        if let Some(value) = &self.report {
            return value.serialize(serializer);
        }
        if let Some(value) = &self.end {
            return value.serialize(serializer);
        }
        unreachable!()
    }
}

impl<'de> Deserialize<'de> for WorkDoneProgressBeginOrReportOrEnd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value.get("kind").and_then(serde_json::Value::as_str) {
            Some("begin") => Ok(Self {
                begin: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
            Some("report") => Ok(Self {
                report: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
            Some("end") => Ok(Self {
                end: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
            _ => Err(serde::de::Error::custom(
                "invalid WorkDoneProgressBeginOrReportOrEnd",
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProgressParams {
    pub token: IntegerOrString,
    pub value: WorkDoneProgressBeginOrReportOrEnd,
}

pub static WindowLogMessageInfo: LazyLock<NotificationInfo<LogMessageParams>> =
    LazyLock::new(|| NotificationInfo {
        _params: PhantomData,
        method: METHOD_WINDOW_LOG_MESSAGE.to_string(),
    });

pub static WindowWorkDoneProgressCreateInfo: LazyLock<
    RequestInfo<WorkDoneProgressCreateParams, WorkDoneProgressCreateResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: "window/workDoneProgress/create".to_string(),
});

pub static ProgressInfo: LazyLock<NotificationInfo<ProgressParams>> =
    LazyLock::new(|| NotificationInfo {
        _params: PhantomData,
        method: "$/progress".to_string(),
    });

pub static ClientRegisterCapabilityInfo: LazyLock<
    RequestInfo<RegistrationParams, RegistrationResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodClientRegisterCapability.to_string(),
});

pub static ClientUnregisterCapabilityInfo: LazyLock<
    RequestInfo<UnregistrationParams, UnregistrationResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodClientUnregisterCapability.to_string(),
});

pub static WorkspaceDiagnosticRefreshInfo: LazyLock<
    RequestInfo<NoParams, DiagnosticRefreshResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodWorkspaceDiagnosticRefresh.to_string(),
});

pub static WorkspaceInlayHintRefreshInfo: LazyLock<
    RequestInfo<NoParams, InlayHintRefreshResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodWorkspaceInlayHintRefresh.to_string(),
});

pub static WorkspaceCodeLensRefreshInfo: LazyLock<RequestInfo<NoParams, CodeLensRefreshResponse>> =
    LazyLock::new(|| RequestInfo {
        _params: PhantomData,
        _resp: PhantomData,
        method: MethodWorkspaceCodeLensRefresh.to_string(),
    });

pub static WorkspaceConfigurationInfo: LazyLock<
    RequestInfo<ConfigurationParams, ConfigurationResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodWorkspaceConfiguration.to_string(),
});

pub static TextDocumentPublishDiagnosticsInfo: LazyLock<
    NotificationInfo<PublishDiagnosticsParams>,
> = LazyLock::new(|| NotificationInfo {
    _params: PhantomData,
    method: MethodTextDocumentPublishDiagnostics.to_string(),
});

pub static InitializeInfo: LazyLock<RequestInfo<InitializeParams, InitializeResponse>> =
    LazyLock::new(|| RequestInfo {
        _params: PhantomData,
        _resp: PhantomData,
        method: MethodInitialize.to_string(),
    });

pub static InitializedInfo: LazyLock<NotificationInfo<InitializedParams>> =
    LazyLock::new(|| NotificationInfo {
        _params: PhantomData,
        method: MethodInitialized.to_string(),
    });

pub static WorkspaceDidChangeConfigurationInfo: LazyLock<
    NotificationInfo<DidChangeConfigurationParams>,
> = LazyLock::new(|| NotificationInfo {
    _params: PhantomData,
    method: MethodWorkspaceDidChangeConfiguration.to_string(),
});

pub static WorkspaceDidChangeWatchedFilesInfo: LazyLock<
    NotificationInfo<DidChangeWatchedFilesParams>,
> = LazyLock::new(|| NotificationInfo {
    _params: PhantomData,
    method: MethodWorkspaceDidChangeWatchedFiles.to_string(),
});

pub static TextDocumentDidOpenInfo: LazyLock<NotificationInfo<DidOpenTextDocumentParams>> =
    LazyLock::new(|| NotificationInfo {
        _params: PhantomData,
        method: MethodTextDocumentDidOpen.to_string(),
    });

pub static TextDocumentDidChangeInfo: LazyLock<NotificationInfo<DidChangeTextDocumentParams>> =
    LazyLock::new(|| NotificationInfo {
        _params: PhantomData,
        method: MethodTextDocumentDidChange.to_string(),
    });

pub static TextDocumentDidCloseInfo: LazyLock<NotificationInfo<DidCloseTextDocumentParams>> =
    LazyLock::new(|| NotificationInfo {
        _params: PhantomData,
        method: MethodTextDocumentDidClose.to_string(),
    });

pub static TextDocumentCompletionInfo: LazyLock<RequestInfo<CompletionParams, CompletionResponse>> =
    LazyLock::new(|| RequestInfo {
        _params: PhantomData,
        _resp: PhantomData,
        method: MethodTextDocumentCompletion.to_string(),
    });

pub static TextDocumentDiagnosticInfo: LazyLock<
    RequestInfo<DocumentDiagnosticParams, DocumentDiagnosticResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodTextDocumentDiagnostic.to_string(),
});

pub static TextDocumentCodeActionInfo: LazyLock<RequestInfo<CodeActionParams, CodeActionResponse>> =
    LazyLock::new(|| RequestInfo {
        _params: PhantomData,
        _resp: PhantomData,
        method: MethodTextDocumentCodeAction.to_string(),
    });

pub static TextDocumentHoverInfo: LazyLock<RequestInfo<HoverParams, HoverResponse>> =
    LazyLock::new(|| RequestInfo {
        _params: PhantomData,
        _resp: PhantomData,
        method: MethodTextDocumentHover.to_string(),
    });

pub static TextDocumentFormattingInfo: LazyLock<
    RequestInfo<DocumentFormattingParams, DocumentFormattingResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodTextDocumentFormatting.to_string(),
});

pub static TextDocumentRangeFormattingInfo: LazyLock<
    RequestInfo<DocumentRangeFormattingParams, DocumentRangeFormattingResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodTextDocumentRangeFormatting.to_string(),
});

pub static TextDocumentOnTypeFormattingInfo: LazyLock<
    RequestInfo<DocumentOnTypeFormattingParams, DocumentOnTypeFormattingResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodTextDocumentOnTypeFormatting.to_string(),
});

pub static TextDocumentDocumentSymbolInfo: LazyLock<
    RequestInfo<DocumentSymbolParams, DocumentSymbolResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodTextDocumentDocumentSymbol.to_string(),
});

pub static TextDocumentReferencesInfo: LazyLock<RequestInfo<ReferenceParams, ReferencesResponse>> =
    LazyLock::new(|| RequestInfo {
        _params: PhantomData,
        _resp: PhantomData,
        method: MethodTextDocumentReferences.to_string(),
    });

pub static TextDocumentSemanticTokensFullInfo: LazyLock<
    RequestInfo<SemanticTokensParams, SemanticTokensResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodTextDocumentSemanticTokensFull.to_string(),
});

pub static CustomProjectInfoInfo: LazyLock<
    RequestInfo<ProjectInfoParams, CustomProjectInfoResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodCustomProjectInfo.to_string(),
});

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RequestFailureTelemetryEvent {
    pub properties: Option<RequestFailureTelemetryProperties>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestFailureTelemetryProperties {
    pub error_code: String,
    pub request_method: String,
    pub stack: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerformanceStatsTelemetryEvent {
    pub measurements: Option<PerformanceStatsTelemetryMeasurements>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceStatsTelemetryMeasurements {
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub open_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub uptime_seconds: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub project_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub config_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub cached_disk_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub memory_used_bytes: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub go_mem_limit: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub go_gc_percent: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub heap_goal_bytes: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub heap_live_bytes: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub heap_object_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub heap_stack_bytes: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub heap_released_bytes: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub heap_free_bytes: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub gc_scan_heap_bytes: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub go_max_procs: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub goroutine_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub gc_cycles_total: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub gc_cpu_seconds: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub user_cpu_seconds: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub system_mem_total: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub system_mem_used: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub auto_import_project_bucket_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub auto_import_node_modules_bucket_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub auto_import_unique_package_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub auto_import_project_export_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub auto_import_node_modules_export_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub auto_import_project_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub auto_import_node_modules_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub auto_import_node_modules_unfiltered_bucket_count: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectInfoTelemetryEvent {
    pub properties: HashMap<String, String>,
    pub measurements: Option<ProjectInfoTelemetryMeasurements>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfoTelemetryMeasurements {
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub js_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub js_file_size: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub jsx_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub jsx_file_size: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub ts_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub ts_file_size: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub tsx_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub tsx_file_size: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub dts_file_count: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub dts_file_size: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TelemetryEvent {
    pub request_failure_telemetry_event: Option<RequestFailureTelemetryEvent>,
    pub performance_stats_telemetry_event: Option<PerformanceStatsTelemetryEvent>,
    pub project_info_telemetry_event: Option<ProjectInfoTelemetryEvent>,
}

pub static TelemetryEventInfo: LazyLock<NotificationInfo<TelemetryEvent>> =
    LazyLock::new(|| NotificationInfo {
        _params: PhantomData,
        method: MethodTelemetryEvent.to_string(),
    });

impl Serialize for RequestFailureTelemetryEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut value = serde_json::Map::new();
        value.insert(
            "eventName".to_string(),
            serde_json::Value::String("languageServer.errorResponse".to_string()),
        );
        value.insert(
            "telemetryPurpose".to_string(),
            serde_json::Value::String("error".to_string()),
        );
        value.insert(
            "properties".to_string(),
            serde_json::to_value(&self.properties).map_err(serde::ser::Error::custom)?,
        );
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RequestFailureTelemetryEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            event_name: String,
            telemetry_purpose: String,
            properties: RequestFailureTelemetryProperties,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.event_name != "languageServer.errorResponse" {
            return Err(serde::de::Error::custom(format!(
                "expected RequestFailureTelemetryEvent eventName languageServer.errorResponse, got {:?}",
                wire.event_name
            )));
        }
        if wire.telemetry_purpose != "error" {
            return Err(serde::de::Error::custom(format!(
                "expected RequestFailureTelemetryEvent telemetryPurpose error, got {:?}",
                wire.telemetry_purpose
            )));
        }
        Ok(Self {
            properties: Some(wire.properties),
        })
    }
}

impl Serialize for PerformanceStatsTelemetryEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut value = serde_json::Map::new();
        value.insert(
            "eventName".to_string(),
            serde_json::Value::String("languageServer.performanceStats".to_string()),
        );
        value.insert(
            "telemetryPurpose".to_string(),
            serde_json::Value::String("usage".to_string()),
        );
        value.insert(
            "measurements".to_string(),
            serde_json::to_value(&self.measurements).map_err(serde::ser::Error::custom)?,
        );
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PerformanceStatsTelemetryEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            event_name: String,
            telemetry_purpose: String,
            measurements: PerformanceStatsTelemetryMeasurements,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.event_name != "languageServer.performanceStats" {
            return Err(serde::de::Error::custom(format!(
                "expected PerformanceStatsTelemetryEvent eventName languageServer.performanceStats, got {:?}",
                wire.event_name
            )));
        }
        if wire.telemetry_purpose != "usage" {
            return Err(serde::de::Error::custom(format!(
                "expected PerformanceStatsTelemetryEvent telemetryPurpose usage, got {:?}",
                wire.telemetry_purpose
            )));
        }
        Ok(Self {
            measurements: Some(wire.measurements),
        })
    }
}

impl Serialize for ProjectInfoTelemetryEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut value = serde_json::Map::new();
        value.insert(
            "eventName".to_string(),
            serde_json::Value::String("languageServer.projectInfo".to_string()),
        );
        value.insert(
            "telemetryPurpose".to_string(),
            serde_json::Value::String("usage".to_string()),
        );
        value.insert(
            "properties".to_string(),
            serde_json::to_value(&self.properties).map_err(serde::ser::Error::custom)?,
        );
        value.insert(
            "measurements".to_string(),
            serde_json::to_value(&self.measurements).map_err(serde::ser::Error::custom)?,
        );
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProjectInfoTelemetryEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            event_name: String,
            telemetry_purpose: String,
            properties: HashMap<String, String>,
            measurements: ProjectInfoTelemetryMeasurements,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.event_name != "languageServer.projectInfo" {
            return Err(serde::de::Error::custom(format!(
                "expected ProjectInfoTelemetryEvent eventName languageServer.projectInfo, got {:?}",
                wire.event_name
            )));
        }
        if wire.telemetry_purpose != "usage" {
            return Err(serde::de::Error::custom(format!(
                "expected ProjectInfoTelemetryEvent telemetryPurpose usage, got {:?}",
                wire.telemetry_purpose
            )));
        }
        Ok(Self {
            properties: wire.properties,
            measurements: Some(wire.measurements),
        })
    }
}

impl Serialize for TelemetryEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let set_count = self.request_failure_telemetry_event.is_some() as usize
            + self.performance_stats_telemetry_event.is_some() as usize
            + self.project_info_telemetry_event.is_some() as usize;
        if set_count > 1 {
            return Err(serde::ser::Error::custom(
                "more than one element of TelemetryEvent is set",
            ));
        }
        if let Some(event) = &self.request_failure_telemetry_event {
            return event.serialize(serializer);
        }
        if let Some(event) = &self.performance_stats_telemetry_event {
            return event.serialize(serializer);
        }
        if let Some(event) = &self.project_info_telemetry_event {
            return event.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for TelemetryEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(Self::default());
        }

        let event_name = value
            .get("eventName")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("invalid TelemetryEvent eventName"))?;

        match event_name {
            "languageServer.errorResponse" => Ok(Self {
                request_failure_telemetry_event: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
                ..Default::default()
            }),
            "languageServer.performanceStats" => Ok(Self {
                performance_stats_telemetry_event: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
                ..Default::default()
            }),
            "languageServer.projectInfo" => Ok(Self {
                project_info_telemetry_event: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
                ..Default::default()
            }),
            _ => Err(serde::de::Error::custom(format!(
                "invalid TelemetryEvent eventName {event_name:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProfileParams {
    pub dir: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProfileResult {
    pub file: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileResultOrNull {
    pub profile_result: Option<ProfileResult>,
}

impl Serialize for ProfileResultOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(profile_result) = &self.profile_result {
            return profile_result.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for ProfileResultOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let profile_result =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    profile_result: Some(profile_result),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid ProfileResultOrNull: got {other}"
            ))),
        }
    }
}

pub type RunGCResponse = Null;
pub type SaveHeapProfileResponse = ProfileResultOrNull;
pub type SaveAllocProfileResponse = ProfileResultOrNull;
pub type StartCPUProfileResponse = Null;
pub type StopCPUProfileResponse = ProfileResultOrNull;

pub static CustomRunGCInfo: LazyLock<RequestInfo<NoParams, RunGCResponse>> =
    LazyLock::new(|| RequestInfo {
        _params: PhantomData,
        _resp: PhantomData,
        method: MethodCustomRunGC.to_string(),
    });

pub static CustomSaveHeapProfileInfo: LazyLock<
    RequestInfo<ProfileParams, SaveHeapProfileResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodCustomSaveHeapProfile.to_string(),
});

pub static CustomSaveAllocProfileInfo: LazyLock<
    RequestInfo<ProfileParams, SaveAllocProfileResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodCustomSaveAllocProfile.to_string(),
});

pub static CustomStartCPUProfileInfo: LazyLock<
    RequestInfo<ProfileParams, StartCPUProfileResponse>,
> = LazyLock::new(|| RequestInfo {
    _params: PhantomData,
    _resp: PhantomData,
    method: MethodCustomStartCPUProfile.to_string(),
});

pub static CustomStopCPUProfileInfo: LazyLock<RequestInfo<NoParams, StopCPUProfileResponse>> =
    LazyLock::new(|| RequestInfo {
        _params: PhantomData,
        _resp: PhantomData,
        method: MethodCustomStopCPUProfile.to_string(),
    });

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CodeActionKind {
    Empty,
    QuickFix,
    Refactor,
    Source,
    SourceOrganizeImports,
    SourceFixAll,
    SourceRemoveUnusedImports,
    SourceSortImports,
    Custom(String),
}

impl CodeActionKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Empty => "",
            Self::QuickFix => "quickfix",
            Self::Refactor => "refactor",
            Self::Source => "source",
            Self::SourceOrganizeImports => "source.organizeImports",
            Self::SourceFixAll => "source.fixAll",
            Self::SourceRemoveUnusedImports => "source.removeUnusedImports",
            Self::SourceSortImports => "source.sortImports",
            Self::Custom(value) => value,
        }
    }
}

impl fmt::Display for CodeActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for CodeActionKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CodeActionKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "" => Self::Empty,
            "quickfix" => Self::QuickFix,
            "refactor" => Self::Refactor,
            "source" => Self::Source,
            "source.organizeImports" => Self::SourceOrganizeImports,
            "source.fixAll" => Self::SourceFixAll,
            "source.removeUnusedImports" => Self::SourceRemoveUnusedImports,
            "source.sortImports" => Self::SourceSortImports,
            _ => Self::Custom(value),
        })
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, Serialize)]
pub struct Location {
    pub uri: DocumentUri,
    pub range: Range,
}

impl<'de> Deserialize<'de> for Location {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("uri") {
            missing.push("uri".to_string());
        }
        if !map.contains_key("range") {
            missing.push("range".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let uri = serde_json::from_value(
            map.remove("uri")
                .expect("uri is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let range = serde_json::from_value(
            map.remove("range")
                .expect("range is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;

        Ok(Self { uri, range })
    }
}

impl HasLocation for Location {
    fn get_location(&self) -> Location {
        self.clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationsOrNull {
    pub locations: Option<Vec<Location>>,
}

impl Serialize for LocationsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(locations) = &self.locations {
            return locations.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for LocationsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let locations = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    locations: Some(locations),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid LocationsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationLink {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_selection_range: Option<Range>,
    pub target_uri: DocumentUri,
    pub target_range: Range,
    pub target_selection_range: Range,
}

impl<'de> Deserialize<'de> for LocationLink {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["targetUri", "targetRange", "targetSelectionRange"])?;

        let origin_selection_range =
            take_non_null_optional_field(&mut map, "originSelectionRange")?;
        let target_uri = take_non_null_required_field(&mut map, "targetUri")?;
        let target_range = take_non_null_required_field(&mut map, "targetRange")?;
        let target_selection_range =
            take_non_null_required_field(&mut map, "targetSelectionRange")?;

        Ok(Self {
            origin_selection_range,
            target_uri,
            target_range,
            target_selection_range,
        })
    }
}

impl HasLocation for LocationLink {
    fn get_location(&self) -> Location {
        Location {
            uri: self.target_uri.clone(),
            range: self.target_range,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct DocumentLink {
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Uri>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<LSPAny>,
}

impl<'de> Deserialize<'de> for DocumentLink {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["range"])?;

        let range = take_non_null_required_field(&mut map, "range")?;
        let target = take_non_null_optional_field(&mut map, "target")?;
        let tooltip = take_non_null_optional_field(&mut map, "tooltip")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            range,
            target,
            tooltip,
            data,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentLinksOrNull {
    pub document_links: Option<Vec<Option<DocumentLink>>>,
}

impl Serialize for DocumentLinksOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(document_links) = &self.document_links {
            return document_links.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for DocumentLinksOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let document_links =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    document_links: Some(document_links),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid DocumentLinksOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationOrLocationsOrDefinitionLinksOrNull {
    pub location: Option<Location>,
    pub locations: Option<Vec<Location>>,
    pub definition_links: Option<Vec<Option<LocationLink>>>,
}

impl Serialize for LocationOrLocationsOrDefinitionLinksOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_at_most_one(
            "more than one element of LocationOrLocationsOrDefinitionLinksOrNull is set",
            bool_to_int(self.location.is_some())
                + bool_to_int(self.locations.is_some())
                + bool_to_int(self.definition_links.is_some()),
        );

        if let Some(location) = &self.location {
            return location.serialize(serializer);
        }
        if let Some(locations) = &self.locations {
            return locations.serialize(serializer);
        }
        if let Some(definition_links) = &self.definition_links {
            return definition_links.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for LocationOrLocationsOrDefinitionLinksOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_definition_locations_union::<D::Error>(
            value,
            "LocationOrLocationsOrDefinitionLinksOrNull",
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationOrLocationsOrDeclarationLinksOrNull {
    pub location: Option<Location>,
    pub locations: Option<Vec<Location>>,
    pub declaration_links: Option<Vec<Option<LocationLink>>>,
}

impl Serialize for LocationOrLocationsOrDeclarationLinksOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_at_most_one(
            "more than one element of LocationOrLocationsOrDeclarationLinksOrNull is set",
            bool_to_int(self.location.is_some())
                + bool_to_int(self.locations.is_some())
                + bool_to_int(self.declaration_links.is_some()),
        );

        if let Some(location) = &self.location {
            return location.serialize(serializer);
        }
        if let Some(locations) = &self.locations {
            return locations.serialize(serializer);
        }
        if let Some(declaration_links) = &self.declaration_links {
            return declaration_links.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for LocationOrLocationsOrDeclarationLinksOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let definition_union = deserialize_definition_locations_union::<D::Error>(
            serde_json::Value::deserialize(deserializer)?,
            "LocationOrLocationsOrDeclarationLinksOrNull",
        )?;
        Ok(Self {
            location: definition_union.location,
            locations: definition_union.locations,
            declaration_links: definition_union.definition_links,
        })
    }
}

fn deserialize_definition_locations_union<E>(
    value: serde_json::Value,
    type_name: &str,
) -> Result<LocationOrLocationsOrDefinitionLinksOrNull, E>
where
    E: serde::de::Error,
{
    match value {
        serde_json::Value::Null => Ok(LocationOrLocationsOrDefinitionLinksOrNull::default()),
        serde_json::Value::Object(_) => Ok(LocationOrLocationsOrDefinitionLinksOrNull {
            location: Some(serde_json::from_value(value).map_err(E::custom)?),
            ..Default::default()
        }),
        serde_json::Value::Array(_) => {
            if let Ok(locations) = serde_json::from_value::<Vec<Location>>(value.clone()) {
                return Ok(LocationOrLocationsOrDefinitionLinksOrNull {
                    locations: Some(locations),
                    ..Default::default()
                });
            }
            if let Ok(definition_links) =
                serde_json::from_value::<Vec<Option<LocationLink>>>(value.clone())
            {
                return Ok(LocationOrLocationsOrDefinitionLinksOrNull {
                    definition_links: Some(definition_links),
                    ..Default::default()
                });
            }
            let data = serde_json::to_vec(&value).unwrap_or_default();
            Err(E::custom(err_invalid_value(type_name, &data)))
        }
        other => Err(E::custom(format!("invalid {type_name}: got {other}"))),
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<DiagnosticSeverity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_information: Option<Vec<DiagnosticRelatedInformation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<DiagnosticTag>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRelatedInformation {
    pub location: Location,
    pub message: String,
}

#[allow(non_upper_case_globals)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticSeverity(pub u32);

#[allow(non_upper_case_globals)]
impl DiagnosticSeverity {
    pub const Error: Self = Self(1);
    pub const Warning: Self = Self(2);
    pub const Information: Self = Self(3);
    pub const Hint: Self = Self(4);
}

#[allow(non_upper_case_globals)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct DiagnosticTag(pub u32);

#[allow(non_upper_case_globals)]
impl DiagnosticTag {
    pub const Unnecessary: Self = Self(1);
    pub const Deprecated: Self = Self(2);
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StringLiteralFull;

impl Serialize for StringLiteralFull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("full")
    }
}

impl<'de> Deserialize<'de> for StringLiteralFull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value == serde_json::Value::String("full".to_string()) {
            Ok(Self)
        } else {
            let data = serde_json::to_vec(&value).map_err(serde::de::Error::custom)?;
            Err(serde::de::Error::custom(err_literal_mismatch(
                "StringLiteralFull",
                "\"full\"",
                &data,
            )))
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StringLiteralUnchanged;

impl Serialize for StringLiteralUnchanged {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("unchanged")
    }
}

impl<'de> Deserialize<'de> for StringLiteralUnchanged {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value == serde_json::Value::String("unchanged".to_string()) {
            Ok(Self)
        } else {
            let data = serde_json::to_vec(&value).map_err(serde::de::Error::custom)?;
            Err(serde::de::Error::custom(err_literal_mismatch(
                "StringLiteralUnchanged",
                "\"unchanged\"",
                &data,
            )))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullDocumentDiagnosticReport {
    pub kind: StringLiteralFull,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    pub items: Vec<Diagnostic>,
}

impl<'de> Deserialize<'de> for FullDocumentDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };
        require_fields::<D::Error>(&map, &["kind", "items"])?;

        let kind = take_non_null_required_field(&mut map, "kind")?;
        let result_id = take_non_null_optional_field(&mut map, "resultId")?;
        let items = take_non_null_required_field(&mut map, "items")?;
        Ok(Self {
            kind,
            result_id,
            items,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnchangedDocumentDiagnosticReport {
    pub kind: StringLiteralUnchanged,
    pub result_id: String,
}

impl<'de> Deserialize<'de> for UnchangedDocumentDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };
        require_fields::<D::Error>(&map, &["kind", "resultId"])?;

        let kind = take_non_null_required_field(&mut map, "kind")?;
        let result_id = take_non_null_required_field(&mut map, "resultId")?;
        Ok(Self { kind, result_id })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedFullDocumentDiagnosticReport {
    pub kind: StringLiteralFull,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    pub items: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_documents: Option<
        HashMap<DocumentUri, FullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport>,
    >,
}

impl<'de> Deserialize<'de> for RelatedFullDocumentDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };
        require_fields::<D::Error>(&map, &["kind", "items"])?;

        let kind = take_non_null_required_field(&mut map, "kind")?;
        let result_id = take_non_null_optional_field(&mut map, "resultId")?;
        let items = take_non_null_required_field(&mut map, "items")?;
        let related_documents = take_non_null_optional_field(&mut map, "relatedDocuments")?;
        Ok(Self {
            kind,
            result_id,
            items,
            related_documents,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedUnchangedDocumentDiagnosticReport {
    pub kind: StringLiteralUnchanged,
    pub result_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_documents: Option<
        HashMap<DocumentUri, FullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport>,
    >,
}

impl<'de> Deserialize<'de> for RelatedUnchangedDocumentDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };
        require_fields::<D::Error>(&map, &["kind", "resultId"])?;

        let kind = take_non_null_required_field(&mut map, "kind")?;
        let result_id = take_non_null_required_field(&mut map, "resultId")?;
        let related_documents = take_non_null_optional_field(&mut map, "relatedDocuments")?;
        Ok(Self {
            kind,
            result_id,
            related_documents,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport {
    pub full_document_diagnostic_report: Option<FullDocumentDiagnosticReport>,
    pub unchanged_document_diagnostic_report: Option<UnchangedDocumentDiagnosticReport>,
}

impl Serialize for FullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of FullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport should be set",
            bool_to_int(self.full_document_diagnostic_report.is_some())
                + bool_to_int(self.unchanged_document_diagnostic_report.is_some()),
        );
        if let Some(full_document_diagnostic_report) = &self.full_document_diagnostic_report {
            return full_document_diagnostic_report.serialize(serializer);
        }
        if let Some(unchanged_document_diagnostic_report) =
            &self.unchanged_document_diagnostic_report
        {
            return unchanged_document_diagnostic_report.serialize(serializer);
        }
        unreachable!()
    }
}

impl<'de> Deserialize<'de> for FullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match json_object_raw_field_from_value(&value, "kind") {
            serde_json::Value::String(kind) if kind == "full" => {
                let full_document_diagnostic_report =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    full_document_diagnostic_report: Some(full_document_diagnostic_report),
                    unchanged_document_diagnostic_report: None,
                })
            }
            serde_json::Value::String(kind) if kind == "unchanged" => {
                let unchanged_document_diagnostic_report =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    full_document_diagnostic_report: None,
                    unchanged_document_diagnostic_report: Some(
                        unchanged_document_diagnostic_report,
                    ),
                })
            }
            _ => {
                let data = serde_json::to_vec(&value).map_err(serde::de::Error::custom)?;
                Err(serde::de::Error::custom(err_invalid_value(
                    "FullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport",
                    &data,
                )))
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RelatedFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport {
    pub full_document_diagnostic_report: Option<RelatedFullDocumentDiagnosticReport>,
    pub unchanged_document_diagnostic_report: Option<RelatedUnchangedDocumentDiagnosticReport>,
}

impl Serialize for RelatedFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of RelatedFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport should be set",
            bool_to_int(self.full_document_diagnostic_report.is_some())
                + bool_to_int(self.unchanged_document_diagnostic_report.is_some()),
        );
        if let Some(full_document_diagnostic_report) = &self.full_document_diagnostic_report {
            return full_document_diagnostic_report.serialize(serializer);
        }
        if let Some(unchanged_document_diagnostic_report) =
            &self.unchanged_document_diagnostic_report
        {
            return unchanged_document_diagnostic_report.serialize(serializer);
        }
        unreachable!()
    }
}

impl<'de> Deserialize<'de>
    for RelatedFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match json_object_raw_field_from_value(&value, "kind") {
            serde_json::Value::String(kind) if kind == "full" => {
                let full_document_diagnostic_report =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    full_document_diagnostic_report: Some(full_document_diagnostic_report),
                    unchanged_document_diagnostic_report: None,
                })
            }
            serde_json::Value::String(kind) if kind == "unchanged" => {
                let unchanged_document_diagnostic_report =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    full_document_diagnostic_report: None,
                    unchanged_document_diagnostic_report: Some(
                        unchanged_document_diagnostic_report,
                    ),
                })
            }
            _ => {
                let data = serde_json::to_vec(&value).map_err(serde::de::Error::custom)?;
                Err(serde::de::Error::custom(err_invalid_value(
                    "RelatedFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport",
                    &data,
                )))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFullDocumentDiagnosticReport {
    pub kind: StringLiteralFull,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    pub items: Vec<Diagnostic>,
    pub uri: DocumentUri,
    pub version: Option<i32>,
}

impl<'de> Deserialize<'de> for WorkspaceFullDocumentDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };
        require_fields::<D::Error>(&map, &["kind", "items", "uri", "version"])?;

        let kind = take_non_null_required_field(&mut map, "kind")?;
        let result_id = take_non_null_optional_field(&mut map, "resultId")?;
        let items = take_non_null_required_field(&mut map, "items")?;
        let uri = take_non_null_required_field(&mut map, "uri")?;
        let version = map
            .remove("version")
            .expect("version is present after required-field check");
        let version = serde_json::from_value(version).map_err(serde::de::Error::custom)?;
        Ok(Self {
            kind,
            result_id,
            items,
            uri,
            version,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUnchangedDocumentDiagnosticReport {
    pub kind: StringLiteralUnchanged,
    pub result_id: String,
    pub uri: DocumentUri,
    pub version: Option<i32>,
}

impl<'de> Deserialize<'de> for WorkspaceUnchangedDocumentDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };
        require_fields::<D::Error>(&map, &["kind", "resultId", "uri", "version"])?;

        let kind = take_non_null_required_field(&mut map, "kind")?;
        let result_id = take_non_null_required_field(&mut map, "resultId")?;
        let uri = take_non_null_required_field(&mut map, "uri")?;
        let version = map
            .remove("version")
            .expect("version is present after required-field check");
        let version = serde_json::from_value(version).map_err(serde::de::Error::custom)?;
        Ok(Self {
            kind,
            result_id,
            uri,
            version,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport {
    pub full_document_diagnostic_report: Option<WorkspaceFullDocumentDiagnosticReport>,
    pub unchanged_document_diagnostic_report: Option<WorkspaceUnchangedDocumentDiagnosticReport>,
}

impl Serialize for WorkspaceFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of WorkspaceFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport should be set",
            bool_to_int(self.full_document_diagnostic_report.is_some())
                + bool_to_int(self.unchanged_document_diagnostic_report.is_some()),
        );
        if let Some(full_document_diagnostic_report) = &self.full_document_diagnostic_report {
            return full_document_diagnostic_report.serialize(serializer);
        }
        if let Some(unchanged_document_diagnostic_report) =
            &self.unchanged_document_diagnostic_report
        {
            return unchanged_document_diagnostic_report.serialize(serializer);
        }
        unreachable!()
    }
}

impl<'de> Deserialize<'de>
    for WorkspaceFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match json_object_raw_field_from_value(&value, "kind") {
            serde_json::Value::String(kind) if kind == "full" => {
                let full_document_diagnostic_report =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    full_document_diagnostic_report: Some(full_document_diagnostic_report),
                    unchanged_document_diagnostic_report: None,
                })
            }
            serde_json::Value::String(kind) if kind == "unchanged" => {
                let unchanged_document_diagnostic_report =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    full_document_diagnostic_report: None,
                    unchanged_document_diagnostic_report: Some(
                        unchanged_document_diagnostic_report,
                    ),
                })
            }
            _ => {
                let data = serde_json::to_vec(&value).map_err(serde::de::Error::custom)?;
                Err(serde::de::Error::custom(err_invalid_value(
                    "WorkspaceFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport",
                    &data,
                )))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkspaceDiagnosticReport {
    pub items: Vec<WorkspaceFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport>,
}

impl<'de> Deserialize<'de> for WorkspaceDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };
        require_fields::<D::Error>(&map, &["items"])?;

        let items = take_non_null_required_field(&mut map, "items")?;
        Ok(Self { items })
    }
}
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

impl TextEdit {
    pub fn compare(&self, other: &Self) -> i32 {
        let range = self.range.compare(&other.range);
        if range != 0 {
            return range;
        }
        match self.new_text.cmp(&other.new_text) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextEditsOrNull {
    pub text_edits: Option<Vec<Option<TextEdit>>>,
}

impl Serialize for TextEditsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(text_edits) = &self.text_edits {
            return text_edits.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for TextEditsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let text_edits = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    text_edits: Some(text_edits),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid TextEditsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonikersOrNull {
    pub monikers: Option<Vec<Option<lsp_types_full::Moniker>>>,
}

impl Serialize for MonikersOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(monikers) = &self.monikers {
            return monikers.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for MonikersOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let monikers = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    monikers: Some(monikers),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid MonikersOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertReplaceEdit {
    pub new_text: String,
    pub insert: Range,
    pub replace: Range,
}
pub type CreateFileOptions = lsp_types_full::CreateFileOptions;
pub type RenameFileOptions = lsp_types_full::RenameFileOptions;
pub type DeleteFileOptions = lsp_types_full::DeleteFileOptions;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<HashMap<DocumentUri, Vec<TextEdit>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_changes: Option<Vec<TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_annotations: Option<HashMap<String, ChangeAnnotation>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeAnnotation {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_confirmation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StringLiteralCreate;

impl Default for StringLiteralCreate {
    fn default() -> Self {
        Self
    }
}

impl Serialize for StringLiteralCreate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("create")
    }
}

impl<'de> Deserialize<'de> for StringLiteralCreate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "create" {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format!(
                "expected StringLiteralCreate value create, got {value:?}"
            )))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StringLiteralRename;

impl Default for StringLiteralRename {
    fn default() -> Self {
        Self
    }
}

impl Serialize for StringLiteralRename {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("rename")
    }
}

impl<'de> Deserialize<'de> for StringLiteralRename {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "rename" {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format!(
                "expected StringLiteralRename value rename, got {value:?}"
            )))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StringLiteralDelete;

impl Default for StringLiteralDelete {
    fn default() -> Self {
        Self
    }
}

impl Serialize for StringLiteralDelete {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("delete")
    }
}

impl<'de> Deserialize<'de> for StringLiteralDelete {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "delete" {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format!(
                "expected StringLiteralDelete value delete, got {value:?}"
            )))
        }
    }
}

// Create file operation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFile {
    // A create
    pub kind: StringLiteralCreate,
    // An optional annotation identifier describing the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation_id: Option<String>,
    // The resource to create.
    pub uri: DocumentUri,
    // Additional options
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<CreateFileOptions>,
}

// Rename file operation
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameFile {
    // A rename
    pub kind: StringLiteralRename,
    // An optional annotation identifier describing the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation_id: Option<String>,
    // The old (existing) location.
    pub old_uri: DocumentUri,
    // The new location.
    pub new_uri: DocumentUri,
    // Rename options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<RenameFileOptions>,
}

// Delete file operation
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFile {
    // A delete
    pub kind: StringLiteralDelete,
    // An optional annotation identifier describing the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation_id: Option<String>,
    // The file to delete.
    pub uri: DocumentUri,
    // Delete options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<DeleteFileOptions>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextEditOrInsertReplaceEdit {
    pub text_edit: Option<TextEdit>,
    pub insert_replace_edit: Option<InsertReplaceEdit>,
}

impl Serialize for TextEditOrInsertReplaceEdit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of TextEditOrInsertReplaceEdit should be set",
            bool_to_int(self.text_edit.is_some()) + bool_to_int(self.insert_replace_edit.is_some()),
        );
        if let Some(text_edit) = &self.text_edit {
            return text_edit.serialize(serializer);
        }
        self.insert_replace_edit
            .as_ref()
            .expect("TextEditOrInsertReplaceEdit has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TextEditOrInsertReplaceEdit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match json_object_has_key_from_value(&value, &["insert", "range"]) {
            0 => Ok(Self {
                text_edit: None,
                insert_replace_edit: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
            }),
            1 => Ok(Self {
                text_edit: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                insert_replace_edit: None,
            }),
            _ => Err(serde::de::Error::custom(
                "invalid TextEditOrInsertReplaceEdit",
            )),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile {
    pub text_document_edit: Option<TextDocumentEdit>,
    pub create_file: Option<CreateFile>,
    pub rename_file: Option<RenameFile>,
    pub delete_file: Option<DeleteFile>,
}

impl Serialize for TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile should be set",
            bool_to_int(self.text_document_edit.is_some())
                + bool_to_int(self.create_file.is_some())
                + bool_to_int(self.rename_file.is_some())
                + bool_to_int(self.delete_file.is_some()),
        );
        if let Some(text_document_edit) = &self.text_document_edit {
            return text_document_edit.serialize(serializer);
        }
        if let Some(create_file) = &self.create_file {
            return create_file.serialize(serializer);
        }
        if let Some(rename_file) = &self.rename_file {
            return rename_file.serialize(serializer);
        }
        self.delete_file
            .as_ref()
            .expect("TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match json_object_raw_field_from_value(&value, "kind") {
            serde_json::Value::String(kind) if kind == "rename" => Ok(Self {
                rename_file: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
            serde_json::Value::String(kind) if kind == "create" => Ok(Self {
                create_file: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
            serde_json::Value::String(kind) if kind == "delete" => Ok(Self {
                delete_file: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
            _ => Ok(Self {
                text_document_edit: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
                ..Default::default()
            }),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceEditOrNull {
    pub workspace_edit: Option<WorkspaceEdit>,
}

impl WorkspaceEditOrNull {
    #[allow(non_snake_case)]
    pub fn WorkspaceEdit(workspace_edit: WorkspaceEdit) -> Self {
        Self {
            workspace_edit: Some(workspace_edit),
        }
    }
}

impl Serialize for WorkspaceEditOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(workspace_edit) = &self.workspace_edit {
            return workspace_edit.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for WorkspaceEditOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let workspace_edit =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    workspace_edit: Some(workspace_edit),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid WorkspaceEditOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareRenamePlaceholder {
    pub range: Range,
    pub placeholder: String,
}

impl<'de> Deserialize<'de> for PrepareRenamePlaceholder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("range") {
            missing.push("range".to_string());
        }
        if !map.contains_key("placeholder") {
            missing.push("placeholder".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let range = take_non_null_required_field(&mut map, "range")?;
        let placeholder = take_non_null_required_field(&mut map, "placeholder")?;
        Ok(Self { range, placeholder })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareRenameDefaultBehavior {
    pub default_behavior: bool,
}

impl<'de> Deserialize<'de> for PrepareRenameDefaultBehavior {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if !map.contains_key("defaultBehavior") {
            return Err(serde::de::Error::custom(err_missing(&[
                "defaultBehavior".to_string()
            ])));
        }

        let default_behavior = take_non_null_required_field(&mut map, "defaultBehavior")?;
        Ok(Self { default_behavior })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RangeOrPrepareRenamePlaceholderOrPrepareRenameDefaultBehaviorOrNull {
    pub range: Option<Range>,
    pub prepare_rename_placeholder: Option<PrepareRenamePlaceholder>,
    pub prepare_rename_default_behavior: Option<PrepareRenameDefaultBehavior>,
}

impl RangeOrPrepareRenamePlaceholderOrPrepareRenameDefaultBehaviorOrNull {
    #[allow(non_snake_case)]
    pub fn PrepareRenamePlaceholder(prepare_rename_placeholder: PrepareRenamePlaceholder) -> Self {
        Self {
            range: None,
            prepare_rename_placeholder: Some(prepare_rename_placeholder),
            prepare_rename_default_behavior: None,
        }
    }

    #[allow(non_snake_case)]
    pub fn Range(range: Range) -> Self {
        Self {
            range: Some(range),
            prepare_rename_placeholder: None,
            prepare_rename_default_behavior: None,
        }
    }

    #[allow(non_snake_case)]
    pub fn PrepareRenameDefaultBehavior(
        prepare_rename_default_behavior: PrepareRenameDefaultBehavior,
    ) -> Self {
        Self {
            range: None,
            prepare_rename_placeholder: None,
            prepare_rename_default_behavior: Some(prepare_rename_default_behavior),
        }
    }
}

impl Serialize for RangeOrPrepareRenamePlaceholderOrPrepareRenameDefaultBehaviorOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_at_most_one(
            "more than one element of RangeOrPrepareRenamePlaceholderOrPrepareRenameDefaultBehaviorOrNull is set",
            bool_to_int(self.range.is_some())
                + bool_to_int(self.prepare_rename_placeholder.is_some())
                + bool_to_int(self.prepare_rename_default_behavior.is_some()),
        );

        if let Some(range) = &self.range {
            return range.serialize(serializer);
        }
        if let Some(prepare_rename_placeholder) = &self.prepare_rename_placeholder {
            return prepare_rename_placeholder.serialize(serializer);
        }
        if let Some(prepare_rename_default_behavior) = &self.prepare_rename_default_behavior {
            return prepare_rename_default_behavior.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for RangeOrPrepareRenamePlaceholderOrPrepareRenameDefaultBehaviorOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(map) if map.contains_key("start") => {
                let range = serde_json::from_value(serde_json::Value::Object(map))
                    .map_err(serde::de::Error::custom)?;
                Ok(Self::Range(range))
            }
            serde_json::Value::Object(map) if map.contains_key("range") => {
                let prepare_rename_placeholder =
                    serde_json::from_value(serde_json::Value::Object(map))
                        .map_err(serde::de::Error::custom)?;
                Ok(Self::PrepareRenamePlaceholder(prepare_rename_placeholder))
            }
            serde_json::Value::Object(map) if map.contains_key("defaultBehavior") => {
                let prepare_rename_default_behavior =
                    serde_json::from_value(serde_json::Value::Object(map))
                        .map_err(serde::de::Error::custom)?;
                Ok(Self::PrepareRenameDefaultBehavior(
                    prepare_rename_default_behavior,
                ))
            }
            serde_json::Value::Object(map) => {
                let data = serde_json::to_vec(&serde_json::Value::Object(map))
                    .map_err(serde::de::Error::custom)?;
                Err(serde::de::Error::custom(err_invalid_value(
                    "RangeOrPrepareRenamePlaceholderOrPrepareRenameDefaultBehaviorOrNull",
                    &data,
                )))
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid RangeOrPrepareRenamePlaceholderOrPrepareRenameDefaultBehaviorOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentIdentifier {
    pub uri: DocumentUri,
}
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}
pub type MarkupContent = lsp_types_full::MarkupContent;
pub type HoverContents = MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings;

// Deprecated: use MarkupContent instead.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MarkedStringWithLanguage {
    pub language: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StringOrMarkedStringWithLanguage {
    pub string: Option<String>,
    pub marked_string_with_language: Option<MarkedStringWithLanguage>,
}

impl Serialize for StringOrMarkedStringWithLanguage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of StringOrMarkedStringWithLanguage should be set",
            bool_to_int(self.string.is_some())
                + bool_to_int(self.marked_string_with_language.is_some()),
        );
        if let Some(string) = &self.string {
            return string.serialize(serializer);
        }
        self.marked_string_with_language
            .as_ref()
            .expect("StringOrMarkedStringWithLanguage has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StringOrMarkedStringWithLanguage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(string) => Ok(Self {
                string: Some(string),
                marked_string_with_language: None,
            }),
            serde_json::Value::Object(_) => Ok(Self {
                string: None,
                marked_string_with_language: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
            }),
            other => Err(serde::de::Error::custom(format!(
                "invalid StringOrMarkedStringWithLanguage: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings {
    pub markup_content: Option<MarkupContent>,
    pub string: Option<String>,
    pub marked_string_with_language: Option<MarkedStringWithLanguage>,
    pub marked_strings: Option<Vec<StringOrMarkedStringWithLanguage>>,
}

impl Serialize for MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings should be set",
            bool_to_int(self.markup_content.is_some())
                + bool_to_int(self.string.is_some())
                + bool_to_int(self.marked_string_with_language.is_some())
                + bool_to_int(self.marked_strings.is_some()),
        );
        if let Some(markup_content) = &self.markup_content {
            return markup_content.serialize(serializer);
        }
        if let Some(string) = &self.string {
            return string.serialize(serializer);
        }
        if let Some(marked_string_with_language) = &self.marked_string_with_language {
            return marked_string_with_language.serialize(serializer);
        }
        self.marked_strings
            .as_ref()
            .expect("MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Object(_) => {
                match json_object_has_key_from_value(&value, &["kind", "language"]) {
                    0 => Ok(Self {
                        markup_content: Some(
                            serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                        ),
                        ..Default::default()
                    }),
                    1 => Ok(Self {
                        marked_string_with_language: Some(
                            serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                        ),
                        ..Default::default()
                    }),
                    _ => Err(serde::de::Error::custom(
                        "invalid MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings",
                    )),
                }
            }
            serde_json::Value::String(string) => Ok(Self {
                string: Some(string),
                ..Default::default()
            }),
            serde_json::Value::Array(_) => Ok(Self {
                marked_strings: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
                ..Default::default()
            }),
            other => Err(serde::de::Error::custom(format!(
                "invalid MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings: got {other}"
            ))),
        }
    }
}
pub type SemanticTokensLegend = lsp_types_full::SemanticTokensLegend;
pub type SemanticTokensDelta = lsp_types_full::SemanticTokensDelta;
pub type SemanticTokenType = lsp_types_full::SemanticTokenType;
pub type SemanticTokenModifier = lsp_types_full::SemanticTokenModifier;
pub type FoldingRangeKind = lsp_types_full::FoldingRangeKind;
pub type LinkedEditingRanges = lsp_types_full::LinkedEditingRanges;
pub type InlayHintKind = lsp_types_full::InlayHintKind;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StringOrInlayHintLabelParts {
    pub string: Option<String>,
    pub inlay_hint_label_parts: Option<Vec<InlayHintLabelPart>>,
}

impl Serialize for StringOrInlayHintLabelParts {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of StringOrInlayHintLabelParts should be set",
            bool_to_int(self.string.is_some()) + bool_to_int(self.inlay_hint_label_parts.is_some()),
        );
        if let Some(string) = &self.string {
            return string.serialize(serializer);
        }
        self.inlay_hint_label_parts
            .as_ref()
            .expect("StringOrInlayHintLabelParts has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StringOrInlayHintLabelParts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(string) => Ok(Self {
                string: Some(string),
                inlay_hint_label_parts: None,
            }),
            serde_json::Value::Array(_) => Ok(Self {
                string: None,
                inlay_hint_label_parts: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
            }),
            other => Err(serde::de::Error::custom(format!(
                "invalid StringOrInlayHintLabelParts: got {other}"
            ))),
        }
    }
}

pub type InlayHintLabel = StringOrInlayHintLabelParts;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintLabelPart {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<StringOrMarkupContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
}

pub type InlayHintTooltip = StringOrMarkupContent;
pub type CompletionItemKind = lsp_types_full::CompletionItemKind;
pub type CompletionTextEdit = TextEditOrInsertReplaceEdit;
pub type CompletionItemLabelDetails = lsp_types_full::CompletionItemLabelDetails;
pub type CompletionItemTag = lsp_types_full::CompletionItemTag;
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompletionTriggerKind(pub u32);

#[allow(non_upper_case_globals)]
impl CompletionTriggerKind {
    pub const Invoked: Self = Self(1);
    pub const TriggerCharacter: Self = Self(2);
    pub const TriggerForIncompleteCompletions: Self = Self(3);
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionContext {
    pub trigger_kind: CompletionTriggerKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_character: Option<String>,
}
pub type SymbolKind = lsp_types_full::SymbolKind;
pub const SymbolKindFile: SymbolKind = SymbolKind::FILE;
pub const SymbolKindModule: SymbolKind = SymbolKind::MODULE;
pub const SymbolKindNamespace: SymbolKind = SymbolKind::NAMESPACE;
pub const SymbolKindPackage: SymbolKind = SymbolKind::PACKAGE;
pub const SymbolKindClass: SymbolKind = SymbolKind::CLASS;
pub const SymbolKindMethod: SymbolKind = SymbolKind::METHOD;
pub const SymbolKindProperty: SymbolKind = SymbolKind::PROPERTY;
pub const SymbolKindConstructor: SymbolKind = SymbolKind::CONSTRUCTOR;
pub const SymbolKindEnum: SymbolKind = SymbolKind::ENUM;
pub const SymbolKindInterface: SymbolKind = SymbolKind::INTERFACE;
pub const SymbolKindFunction: SymbolKind = SymbolKind::FUNCTION;
pub const SymbolKindVariable: SymbolKind = SymbolKind::VARIABLE;
pub const SymbolKindString: SymbolKind = SymbolKind::STRING;
pub const SymbolKindObject: SymbolKind = SymbolKind::OBJECT;
pub const SymbolKindEnumMember: SymbolKind = SymbolKind::ENUM_MEMBER;
pub const SymbolKindTypeParameter: SymbolKind = SymbolKind::TYPE_PARAMETER;
pub type SignatureHelpContext = lsp_types_full::SignatureHelpContext;
pub type SignatureHelpTriggerKind = lsp_types_full::SignatureHelpTriggerKind;
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceContext {
    pub include_declaration: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentContentChangePartial {
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_length: Option<u32>,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentContentChangeWholeDocument {
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextDocumentContentChangePartialOrWholeDocument {
    pub partial: Option<TextDocumentContentChangePartial>,
    pub whole_document: Option<TextDocumentContentChangeWholeDocument>,
}

impl Serialize for TextDocumentContentChangePartialOrWholeDocument {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of TextDocumentContentChangePartialOrWholeDocument should be set",
            bool_to_int(self.partial.is_some()) + bool_to_int(self.whole_document.is_some()),
        );
        if let Some(partial) = &self.partial {
            return partial.serialize(serializer);
        }
        self.whole_document
            .as_ref()
            .expect("TextDocumentContentChangePartialOrWholeDocument has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TextDocumentContentChangePartialOrWholeDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("range").is_some() {
            return Ok(Self {
                partial: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                whole_document: None,
            });
        }
        Ok(Self {
            partial: None,
            whole_document: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
        })
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalVersionedTextDocumentIdentifier {
    // The text document's uri.
    pub uri: DocumentUri,
    // The version number of this document, or null when unknown.
    pub version: Option<i32>,
}
pub type ClientCapabilities = lsp_types_full::ClientCapabilities;
pub type ServerCapabilities = lsp_types_full::ServerCapabilities;
pub type ClientInfo = lsp_types_full::ClientInfo;
pub type ServerInfo = lsp_types_full::ServerInfo;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterInformation {
    pub label: StringOrTuple,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<StringOrMarkupContent>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInformation {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<StringOrMarkupContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<ParameterInformation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_parameter: Option<UintegerOrNull>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_signature: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_parameter: Option<UintegerOrNull>,
}

pub type ApplyKind = u32;
pub const ApplyKindReplace: ApplyKind = 1;
pub const ApplyKindMerge: ApplyKind = 2;
pub type CodeActionTag = u32;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct DocumentHighlightKind(pub u32);

#[allow(non_upper_case_globals)]
impl DocumentHighlightKind {
    pub const Text: Self = Self(1);
    pub const Read: Self = Self(2);
    pub const Write: Self = Self(3);
}

impl Serialize for DocumentHighlightKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for DocumentHighlightKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(u32::deserialize(deserializer)?))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct DocumentHighlight {
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<DocumentHighlightKind>,
}

impl<'de> Deserialize<'de> for DocumentHighlight {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["range"])?;

        let range = take_non_null_required_field(&mut map, "range")?;
        let kind = take_non_null_optional_field(&mut map, "kind")?;

        Ok(Self { range, kind })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentHighlightsOrNull {
    pub document_highlights: Option<Vec<Option<DocumentHighlight>>>,
}

impl Serialize for DocumentHighlightsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(document_highlights) = &self.document_highlights {
            return document_highlights.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for DocumentHighlightsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let document_highlights =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    document_highlights: Some(document_highlights),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid DocumentHighlightsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SelectionRange {
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Box<SelectionRange>>,
}

impl<'de> Deserialize<'de> for SelectionRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["range"])?;

        let range = take_non_null_required_field(&mut map, "range")?;
        let parent = take_non_null_optional_field(&mut map, "parent")?;

        Ok(Self { range, parent })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionRangesOrNull {
    pub selection_ranges: Option<Vec<Option<SelectionRange>>>,
}

impl Serialize for SelectionRangesOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(selection_ranges) = &self.selection_ranges {
            return selection_ranges.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for SelectionRangesOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let selection_ranges =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    selection_ranges: Some(selection_ranges),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid SelectionRangesOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Command {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<LSPAny>>,
}

impl<'de> Deserialize<'de> for Command {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["title", "command"])?;

        let title = take_non_null_required_field(&mut map, "title")?;
        let tooltip = take_non_null_optional_field(&mut map, "tooltip")?;
        let command = take_non_null_required_field(&mut map, "command")?;
        let arguments = take_non_null_optional_field(&mut map, "arguments")?;

        Ok(Self {
            title,
            tooltip,
            command,
            arguments,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAction {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<CodeActionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Vec<Diagnostic>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_preferred: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<CodeActionDisabled>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<WorkspaceEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<LSPAny>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<CodeActionTag>>,
}

impl<'de> Deserialize<'de> for CodeAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["title"])?;

        let title = take_non_null_required_field(&mut map, "title")?;
        let kind = take_non_null_optional_field(&mut map, "kind")?;
        let diagnostics = take_non_null_optional_field(&mut map, "diagnostics")?;
        let is_preferred = take_non_null_optional_field(&mut map, "isPreferred")?;
        let disabled = take_non_null_optional_field(&mut map, "disabled")?;
        let edit = take_non_null_optional_field(&mut map, "edit")?;
        let command = take_non_null_optional_field(&mut map, "command")?;
        let data = take_non_null_optional_field(&mut map, "data")?;
        let tags = take_non_null_optional_field(&mut map, "tags")?;

        Ok(Self {
            title,
            kind,
            diagnostics,
            is_preferred,
            disabled,
            edit,
            command,
            data,
            tags,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CodeActionDisabled {
    pub reason: String,
}

impl<'de> Deserialize<'de> for CodeActionDisabled {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["reason"])?;

        let reason = take_non_null_required_field(&mut map, "reason")?;
        Ok(Self { reason })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandOrCodeAction {
    pub command: Option<Command>,
    pub code_action: Option<CodeAction>,
}

impl Serialize for CommandOrCodeAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of CommandOrCodeAction should be set",
            bool_to_int(self.command.is_some()) + bool_to_int(self.code_action.is_some()),
        );

        if let Some(command) = &self.command {
            return command.serialize(serializer);
        }
        if let Some(code_action) = &self.code_action {
            return code_action.serialize(serializer);
        }
        unreachable!()
    }
}

impl<'de> Deserialize<'de> for CommandOrCodeAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Ok(command) = serde_json::from_value::<Command>(value.clone()) {
            return Ok(Self {
                command: Some(command),
                ..Default::default()
            });
        }
        if let Ok(code_action) = serde_json::from_value::<CodeAction>(value.clone()) {
            return Ok(Self {
                code_action: Some(code_action),
                ..Default::default()
            });
        }
        let data = serde_json::to_vec(&value).unwrap_or_default();
        Err(serde::de::Error::custom(err_invalid_value(
            "CommandOrCodeAction",
            &data,
        )))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandOrCodeActionArrayOrNull {
    pub command_or_code_action_array: Option<Vec<CommandOrCodeAction>>,
}

impl Serialize for CommandOrCodeActionArrayOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(command_or_code_action_array) = &self.command_or_code_action_array {
            return command_or_code_action_array.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for CommandOrCodeActionArrayOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let command_or_code_action_array =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    command_or_code_action_array: Some(command_or_code_action_array),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid CommandOrCodeActionArrayOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CodeLens {
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<CodeLensData>,
}

impl<'de> Deserialize<'de> for CodeLens {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["range"])?;

        let range = take_non_null_required_field(&mut map, "range")?;
        let command = take_non_null_optional_field(&mut map, "command")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            range,
            command,
            data,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodeLensesOrNull {
    pub code_lenses: Option<Vec<Option<CodeLens>>>,
}

impl Serialize for CodeLensesOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(code_lenses) = &self.code_lenses {
            return code_lenses.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for CodeLensesOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let code_lenses =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    code_lenses: Some(code_lenses),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid CodeLensesOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInformation {
    pub name: String,
    pub kind: SymbolKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<lsp_types_full::SymbolTag>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    pub location: Location,
}

impl Default for SymbolInformation {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: SymbolKind::FILE,
            tags: None,
            container_name: None,
            deprecated: None,
            location: Location::default(),
        }
    }
}

impl<'de> Deserialize<'de> for SymbolInformation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["name", "kind", "location"])?;

        let name = take_non_null_required_field(&mut map, "name")?;
        let kind = take_non_null_required_field(&mut map, "kind")?;
        let tags = take_non_null_optional_field(&mut map, "tags")?;
        let container_name = take_non_null_optional_field(&mut map, "containerName")?;
        let deprecated = take_non_null_optional_field(&mut map, "deprecated")?;
        let location = take_non_null_required_field(&mut map, "location")?;

        Ok(Self {
            name,
            kind,
            tags,
            container_name,
            deprecated,
            location,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbol {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub kind: SymbolKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<lsp_types_full::SymbolTag>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    pub range: Range,
    pub selection_range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<DocumentSymbol>>,
}

impl Default for DocumentSymbol {
    fn default() -> Self {
        Self {
            name: String::new(),
            detail: None,
            kind: SymbolKind::FILE,
            tags: None,
            deprecated: None,
            range: Range::default(),
            selection_range: Range::default(),
            children: None,
        }
    }
}

impl<'de> Deserialize<'de> for DocumentSymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["name", "kind", "range", "selectionRange"])?;

        let name = take_non_null_required_field(&mut map, "name")?;
        let detail = take_non_null_optional_field(&mut map, "detail")?;
        let kind = take_non_null_required_field(&mut map, "kind")?;
        let tags = take_non_null_optional_field(&mut map, "tags")?;
        let deprecated = take_non_null_optional_field(&mut map, "deprecated")?;
        let range = take_non_null_required_field(&mut map, "range")?;
        let selection_range = take_non_null_required_field(&mut map, "selectionRange")?;
        let children = take_non_null_optional_field(&mut map, "children")?;

        Ok(Self {
            name,
            detail,
            kind,
            tags,
            deprecated,
            range,
            selection_range,
            children,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymbolInformationsOrDocumentSymbolsOrNull {
    pub symbol_informations: Option<Vec<Option<Box<SymbolInformation>>>>,
    pub document_symbols: Option<Vec<Option<DocumentSymbol>>>,
}

impl Serialize for SymbolInformationsOrDocumentSymbolsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_at_most_one(
            "more than one element of SymbolInformationsOrDocumentSymbolsOrNull is set",
            bool_to_int(self.symbol_informations.is_some())
                + bool_to_int(self.document_symbols.is_some()),
        );

        if let Some(symbol_informations) = &self.symbol_informations {
            return symbol_informations.serialize(serializer);
        }
        if let Some(document_symbols) = &self.document_symbols {
            return document_symbols.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for SymbolInformationsOrDocumentSymbolsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                if let Ok(symbol_informations) =
                    serde_json::from_value::<Vec<Option<Box<SymbolInformation>>>>(value.clone())
                {
                    return Ok(Self {
                        symbol_informations: Some(symbol_informations),
                        ..Default::default()
                    });
                }
                if let Ok(document_symbols) =
                    serde_json::from_value::<Vec<Option<DocumentSymbol>>>(value.clone())
                {
                    return Ok(Self {
                        document_symbols: Some(document_symbols),
                        ..Default::default()
                    });
                }
                let data = serde_json::to_vec(&value).unwrap_or_default();
                Err(serde::de::Error::custom(err_invalid_value(
                    "SymbolInformationsOrDocumentSymbolsOrNull",
                    &data,
                )))
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid SymbolInformationsOrDocumentSymbolsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationOrLocationUriOnly {
    pub location: Option<Location>,
    pub location_uri_only: Option<LocationUriOnly>,
}

impl Serialize for LocationOrLocationUriOnly {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of LocationOrLocationUriOnly should be set",
            bool_to_int(self.location.is_some()) + bool_to_int(self.location_uri_only.is_some()),
        );

        if let Some(location) = &self.location {
            return location.serialize(serializer);
        }
        if let Some(location_uri_only) = &self.location_uri_only {
            return location_uri_only.serialize(serializer);
        }
        unreachable!()
    }
}

impl<'de> Deserialize<'de> for LocationOrLocationUriOnly {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(map) = &value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if map.contains_key("range") {
            return Ok(Self {
                location: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            });
        }
        Ok(Self {
            location_uri_only: Some(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ),
            ..Default::default()
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct LocationUriOnly {
    pub uri: DocumentUri,
}

impl<'de> Deserialize<'de> for LocationUriOnly {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["uri"])?;

        let uri = take_non_null_required_field(&mut map, "uri")?;
        Ok(Self { uri })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: SymbolKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<lsp_types_full::SymbolTag>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    pub location: LocationOrLocationUriOnly,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<LSPAny>,
}

impl Default for WorkspaceSymbol {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: SymbolKind::FILE,
            tags: None,
            container_name: None,
            location: LocationOrLocationUriOnly::default(),
            data: None,
        }
    }
}

impl<'de> Deserialize<'de> for WorkspaceSymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["name", "kind", "location"])?;

        let name = take_non_null_required_field(&mut map, "name")?;
        let kind = take_non_null_required_field(&mut map, "kind")?;
        let tags = take_non_null_optional_field(&mut map, "tags")?;
        let container_name = take_non_null_optional_field(&mut map, "containerName")?;
        let location = take_non_null_required_field(&mut map, "location")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            name,
            kind,
            tags,
            container_name,
            location,
            data,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymbolInformationsOrWorkspaceSymbolsOrNull {
    pub symbol_informations: Option<Vec<Option<Box<SymbolInformation>>>>,
    pub workspace_symbols: Option<Vec<Option<Box<WorkspaceSymbol>>>>,
}

impl Serialize for SymbolInformationsOrWorkspaceSymbolsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_at_most_one(
            "more than one element of SymbolInformationsOrWorkspaceSymbolsOrNull is set",
            bool_to_int(self.symbol_informations.is_some())
                + bool_to_int(self.workspace_symbols.is_some()),
        );

        if let Some(symbol_informations) = &self.symbol_informations {
            return symbol_informations.serialize(serializer);
        }
        if let Some(workspace_symbols) = &self.workspace_symbols {
            return workspace_symbols.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for SymbolInformationsOrWorkspaceSymbolsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                if let Ok(symbol_informations) =
                    serde_json::from_value::<Vec<Option<Box<SymbolInformation>>>>(value.clone())
                {
                    return Ok(Self {
                        symbol_informations: Some(symbol_informations),
                        ..Default::default()
                    });
                }
                if let Ok(workspace_symbols) =
                    serde_json::from_value::<Vec<Option<Box<WorkspaceSymbol>>>>(value.clone())
                {
                    return Ok(Self {
                        workspace_symbols: Some(workspace_symbols),
                        ..Default::default()
                    });
                }
                let data = serde_json::to_vec(&value).unwrap_or_default();
                Err(serde::de::Error::custom(err_invalid_value(
                    "SymbolInformationsOrWorkspaceSymbolsOrNull",
                    &data,
                )))
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid SymbolInformationsOrWorkspaceSymbolsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionList {
    pub is_incomplete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_defaults: Option<CompletionItemDefaults>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_kind: Option<CompletionItemApplyKinds>,
    pub items: Vec<CompletionItem>,
}

impl<'de> Deserialize<'de> for CompletionList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["isIncomplete", "items"])?;

        let is_incomplete = take_non_null_required_field(&mut map, "isIncomplete")?;
        let item_defaults = take_non_null_optional_field(&mut map, "itemDefaults")?;
        let apply_kind = take_non_null_optional_field(&mut map, "applyKind")?;
        let items = take_non_null_required_field(&mut map, "items")?;

        Ok(Self {
            is_incomplete,
            item_defaults,
            apply_kind,
            items,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_characters: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_range: Option<RangeOrEditRangeWithInsertReplace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert_text_format: Option<InsertTextFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert_text_mode: Option<lsp_types_full::InsertTextMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Map<String, serde_json::Value>>,
}

impl<'de> Deserialize<'de> for CompletionItemDefaults {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let commit_characters = take_non_null_optional_field(&mut map, "commitCharacters")?;
        let edit_range = take_non_null_optional_field(&mut map, "editRange")?;
        let insert_text_format = take_non_null_optional_field(&mut map, "insertTextFormat")?;
        let insert_text_mode = take_non_null_optional_field(&mut map, "insertTextMode")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            commit_characters,
            edit_range,
            insert_text_format,
            insert_text_mode,
            data,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemApplyKinds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_characters: Option<ApplyKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<ApplyKind>,
}

impl<'de> Deserialize<'de> for CompletionItemApplyKinds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let commit_characters = take_non_null_optional_field(&mut map, "commitCharacters")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            commit_characters,
            data,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RangeOrEditRangeWithInsertReplace {
    pub range: Option<Range>,
    pub edit_range_with_insert_replace: Option<EditRangeWithInsertReplace>,
}

impl Serialize for RangeOrEditRangeWithInsertReplace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of RangeOrEditRangeWithInsertReplace should be set",
            bool_to_int(self.range.is_some())
                + bool_to_int(self.edit_range_with_insert_replace.is_some()),
        );

        if let Some(range) = &self.range {
            return range.serialize(serializer);
        }
        if let Some(edit_range_with_insert_replace) = &self.edit_range_with_insert_replace {
            return edit_range_with_insert_replace.serialize(serializer);
        }
        unreachable!()
    }
}

impl<'de> Deserialize<'de> for RangeOrEditRangeWithInsertReplace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(map) = &value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if map.contains_key("start") {
            return Ok(Self {
                range: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            });
        }
        if map.contains_key("insert") {
            return Ok(Self {
                edit_range_with_insert_replace: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
                ..Default::default()
            });
        }

        let data = serde_json::to_vec(&value).unwrap_or_default();
        Err(serde::de::Error::custom(err_invalid_value(
            "RangeOrEditRangeWithInsertReplace",
            &data,
        )))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct EditRangeWithInsertReplace {
    pub insert: Range,
    pub replace: Range,
}

impl<'de> Deserialize<'de> for EditRangeWithInsertReplace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["insert", "replace"])?;

        let insert = take_non_null_required_field(&mut map, "insert")?;
        let replace = take_non_null_required_field(&mut map, "replace")?;

        Ok(Self { insert, replace })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompletionItemsOrListOrNull {
    pub items: Option<Vec<Option<CompletionItem>>>,
    pub list: Option<CompletionList>,
}

impl Serialize for CompletionItemsOrListOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_at_most_one(
            "more than one element of CompletionItemsOrListOrNull is set",
            bool_to_int(self.items.is_some()) + bool_to_int(self.list.is_some()),
        );

        if let Some(items) = &self.items {
            return items.serialize(serializer);
        }
        if let Some(list) = &self.list {
            return list.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for CompletionItemsOrListOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => Ok(Self {
                items: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
            serde_json::Value::Object(_) => Ok(Self {
                list: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
            other => Err(serde::de::Error::custom(format!(
                "invalid CompletionItemsOrListOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum TraceValue {
    #[default]
    Off,
    Messages,
    Verbose,
    Other(String),
}

impl TraceValue {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Off => "off",
            Self::Messages => "messages",
            Self::Verbose => "verbose",
            Self::Other(value) => value,
        }
    }
}

impl<'de> Deserialize<'de> for TraceValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "off" => Self::Off,
            "messages" => Self::Messages,
            "verbose" => Self::Verbose,
            _ => Self::Other(value),
        })
    }
}

impl Serialize for TraceValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTraceParams {
    pub value: TraceValue,
}
pub type WorkDoneProgressParams = lsp_types_full::WorkDoneProgressParams;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StringLiteralSnippet;

impl Default for StringLiteralSnippet {
    fn default() -> Self {
        Self
    }
}

impl Serialize for StringLiteralSnippet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("snippet")
    }
}

impl<'de> Deserialize<'de> for StringLiteralSnippet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "snippet" {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(format!(
                "expected StringLiteralSnippet value snippet, got {value:?}"
            )))
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StringValue {
    pub kind: StringLiteralSnippet,
    pub value: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotatedTextEdit {
    pub range: Range,
    pub new_text: String,
    pub annotation_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetTextEdit {
    pub range: Range,
    pub snippet: StringValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation_id: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextEditOrAnnotatedTextEditOrSnippetTextEdit {
    pub text_edit: Option<TextEdit>,
    pub annotated_text_edit: Option<AnnotatedTextEdit>,
    pub snippet_text_edit: Option<SnippetTextEdit>,
}

impl Serialize for TextEditOrAnnotatedTextEditOrSnippetTextEdit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_only_one(
            "exactly one element of TextEditOrAnnotatedTextEditOrSnippetTextEdit should be set",
            bool_to_int(self.text_edit.is_some())
                + bool_to_int(self.annotated_text_edit.is_some())
                + bool_to_int(self.snippet_text_edit.is_some()),
        );
        if let Some(text_edit) = &self.text_edit {
            return text_edit.serialize(serializer);
        }
        if let Some(annotated_text_edit) = &self.annotated_text_edit {
            return annotated_text_edit.serialize(serializer);
        }
        self.snippet_text_edit
            .as_ref()
            .expect("TextEditOrAnnotatedTextEditOrSnippetTextEdit has one active element")
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TextEditOrAnnotatedTextEditOrSnippetTextEdit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match json_object_has_key_from_value(&value, &["snippet", "annotationId"]) {
            0 => Ok(Self {
                snippet_text_edit: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
                ..Default::default()
            }),
            1 => Ok(Self {
                annotated_text_edit: Some(
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?,
                ),
                ..Default::default()
            }),
            _ => Ok(Self {
                text_edit: Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?),
                ..Default::default()
            }),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
    pub process_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ClientInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    pub root_uri: Option<DocumentUri>,
    pub capabilities: ClientCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<InitializationOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<TraceValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_folders: Option<Vec<Option<WorkspaceFolder>>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_push_diagnostics: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_lens_show_locations_command_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_preferences: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_telemetry: Option<bool>,
}

impl<'de> Deserialize<'de> for InitializeParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("processId") {
            missing.push("processId".to_string());
        }
        if !map.contains_key("rootUri") {
            missing.push("rootUri".to_string());
        }
        if !map.contains_key("capabilities") {
            missing.push("capabilities".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        let process_id = serde_json::from_value(
            map.remove("processId")
                .expect("processId is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let client_info = take_non_null_optional_field(&mut map, "clientInfo")?;
        let locale = take_non_null_optional_field(&mut map, "locale")?;
        let root_path = map
            .remove("rootPath")
            .map(serde_json::from_value)
            .transpose()
            .map_err(serde::de::Error::custom)?
            .flatten();
        let root_uri = serde_json::from_value(
            map.remove("rootUri")
                .expect("rootUri is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let capabilities = take_non_null_required_field(&mut map, "capabilities")?;
        let initialization_options =
            take_non_null_optional_field(&mut map, "initializationOptions")?;
        let trace = take_non_null_optional_field(&mut map, "trace")?;
        let workspace_folders = map
            .remove("workspaceFolders")
            .map(serde_json::from_value)
            .transpose()
            .map_err(serde::de::Error::custom)?
            .flatten();

        Ok(Self {
            work_done_progress_params: WorkDoneProgressParams { work_done_token },
            process_id,
            client_info,
            locale,
            root_path,
            root_uri,
            capabilities,
            initialization_options,
            trace,
            workspace_folders,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_info: Option<ServerInfo>,
}

impl<'de> Deserialize<'de> for InitializeResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if !map.contains_key("capabilities") {
            return Err(serde::de::Error::custom(err_missing(&[
                "capabilities".to_string()
            ])));
        }

        let capabilities = take_non_null_required_field(&mut map, "capabilities")?;
        let server_info = take_non_null_optional_field(&mut map, "serverInfo")?;

        Ok(Self {
            capabilities,
            server_info,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InitializeResultOrNull {
    pub initialize_result: Option<InitializeResult>,
}

impl Serialize for InitializeResultOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(initialize_result) = &self.initialize_result {
            return initialize_result.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for InitializeResultOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let initialize_result =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    initialize_result: Some(initialize_result),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid InitializeResultOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokens {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    pub data: Vec<u32>,
}

impl<'de> Deserialize<'de> for SemanticTokens {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if !map.contains_key("data") {
            return Err(serde::de::Error::custom(err_missing(&["data".to_string()])));
        }

        let result_id = take_non_null_optional_field(&mut map, "resultId")?;
        let data = take_non_null_required_field(&mut map, "data")?;

        Ok(Self { result_id, data })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticTokensOrNull {
    pub semantic_tokens: Option<SemanticTokens>,
}

impl Serialize for SemanticTokensOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(semantic_tokens) = &self.semantic_tokens {
            return semantic_tokens.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for SemanticTokensOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let semantic_tokens =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    semantic_tokens: Some(semantic_tokens),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid SemanticTokensOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticTokensOrSemanticTokensDeltaOrNull {
    pub semantic_tokens: Option<SemanticTokens>,
    pub semantic_tokens_delta: Option<SemanticTokensDelta>,
}

impl Serialize for SemanticTokensOrSemanticTokensDeltaOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        assert_at_most_one(
            "more than one element of SemanticTokensOrSemanticTokensDeltaOrNull is set",
            bool_to_int(self.semantic_tokens.is_some())
                + bool_to_int(self.semantic_tokens_delta.is_some()),
        );

        if let Some(semantic_tokens) = &self.semantic_tokens {
            return semantic_tokens.serialize(serializer);
        }
        if let Some(semantic_tokens_delta) = &self.semantic_tokens_delta {
            return semantic_tokens_delta.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for SemanticTokensOrSemanticTokensDeltaOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(map) if map.contains_key("data") => {
                let semantic_tokens = serde_json::from_value(serde_json::Value::Object(map))
                    .map_err(serde::de::Error::custom)?;
                Ok(Self {
                    semantic_tokens: Some(semantic_tokens),
                    semantic_tokens_delta: None,
                })
            }
            serde_json::Value::Object(map) if map.contains_key("edits") => {
                let semantic_tokens_delta = serde_json::from_value(serde_json::Value::Object(map))
                    .map_err(serde::de::Error::custom)?;
                Ok(Self {
                    semantic_tokens: None,
                    semantic_tokens_delta: Some(semantic_tokens_delta),
                })
            }
            serde_json::Value::Object(map) => {
                let data = serde_json::to_vec(&serde_json::Value::Object(map))
                    .map_err(serde::de::Error::custom)?;
                Err(serde::de::Error::custom(err_invalid_value(
                    "SemanticTokensOrSemanticTokensDeltaOrNull",
                    &data,
                )))
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid SemanticTokensOrSemanticTokensDeltaOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkedEditingRangesOrNull {
    pub linked_editing_ranges: Option<LinkedEditingRanges>,
}

impl Serialize for LinkedEditingRangesOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(linked_editing_ranges) = &self.linked_editing_ranges {
            return linked_editing_ranges.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for LinkedEditingRangesOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let linked_editing_ranges =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    linked_editing_ranges: Some(linked_editing_ranges),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid LinkedEditingRangesOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentEdit {
    pub text_document: OptionalVersionedTextDocumentIdentifier,
    pub edits: Vec<TextEditOrAnnotatedTextEditOrSnippetTextEdit>,
}

impl<'de> Deserialize<'de> for TextDocumentEdit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("textDocument") {
            missing.push("textDocument".to_string());
        }
        if !map.contains_key("edits") {
            missing.push("edits".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let text_document = serde_json::from_value(
            map.remove("textDocument")
                .expect("textDocument is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let edits = take_non_null_required_field(&mut map, "edits")?;

        Ok(Self {
            text_document,
            edits,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyItem {
    pub name: String,
    pub kind: SymbolKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<lsp_types_full::SymbolTag>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub uri: DocumentUri,
    pub range: Range,
    pub selection_range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Map<String, serde_json::Value>>,
}

impl<'de> Deserialize<'de> for CallHierarchyItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["name", "kind", "uri", "range", "selectionRange"])?;

        let name = take_non_null_required_field(&mut map, "name")?;
        let kind = take_non_null_required_field(&mut map, "kind")?;
        let tags = take_non_null_optional_field(&mut map, "tags")?;
        let detail = take_non_null_optional_field(&mut map, "detail")?;
        let uri = take_non_null_required_field(&mut map, "uri")?;
        let range = take_non_null_required_field(&mut map, "range")?;
        let selection_range = take_non_null_required_field(&mut map, "selectionRange")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            name,
            kind,
            tags,
            detail,
            uri,
            range,
            selection_range,
            data,
        })
    }
}

impl HasLocation for CallHierarchyItem {
    fn get_location(&self) -> Location {
        Location {
            uri: self.uri.clone(),
            range: self.range,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CallHierarchyItemsOrNull {
    pub call_hierarchy_items: Option<Vec<Option<CallHierarchyItem>>>,
}

impl Serialize for CallHierarchyItemsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(call_hierarchy_items) = &self.call_hierarchy_items {
            return call_hierarchy_items.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for CallHierarchyItemsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let call_hierarchy_items =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    call_hierarchy_items: Some(call_hierarchy_items),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid CallHierarchyItemsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyIncomingCallsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    pub item: CallHierarchyItem,
}

impl<'de> Deserialize<'de> for CallHierarchyIncomingCallsParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if !map.contains_key("item") {
            return Err(serde::de::Error::custom(err_missing(&["item".to_string()])));
        }

        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        let partial_result_token = take_non_null_optional_field(&mut map, "partialResultToken")?;
        let item = take_non_null_required_field(&mut map, "item")?;

        Ok(Self {
            work_done_token,
            partial_result_token,
            item,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyOutgoingCallsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    pub item: CallHierarchyItem,
}

impl<'de> Deserialize<'de> for CallHierarchyOutgoingCallsParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["item"])?;

        let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
        let partial_result_token = take_non_null_optional_field(&mut map, "partialResultToken")?;
        let item = take_non_null_required_field(&mut map, "item")?;

        Ok(Self {
            work_done_token,
            partial_result_token,
            item,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyIncomingCall {
    pub from: CallHierarchyItem,
    pub from_ranges: Vec<Range>,
}

impl<'de> Deserialize<'de> for CallHierarchyIncomingCall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("from") {
            missing.push("from".to_string());
        }
        if !map.contains_key("fromRanges") {
            missing.push("fromRanges".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let from = take_non_null_required_field(&mut map, "from")?;
        let from_ranges = take_non_null_required_field(&mut map, "fromRanges")?;

        Ok(Self { from, from_ranges })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CallHierarchyIncomingCallsOrNull {
    pub call_hierarchy_incoming_calls: Option<Vec<Option<CallHierarchyIncomingCall>>>,
}

impl Serialize for CallHierarchyIncomingCallsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(call_hierarchy_incoming_calls) = &self.call_hierarchy_incoming_calls {
            return call_hierarchy_incoming_calls.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for CallHierarchyIncomingCallsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let call_hierarchy_incoming_calls =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    call_hierarchy_incoming_calls: Some(call_hierarchy_incoming_calls),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid CallHierarchyIncomingCallsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyOutgoingCall {
    pub to: CallHierarchyItem,
    pub from_ranges: Vec<Range>,
}

impl<'de> Deserialize<'de> for CallHierarchyOutgoingCall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("to") {
            missing.push("to".to_string());
        }
        if !map.contains_key("fromRanges") {
            missing.push("fromRanges".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let to = take_non_null_required_field(&mut map, "to")?;
        let from_ranges = take_non_null_required_field(&mut map, "fromRanges")?;

        Ok(Self { to, from_ranges })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CallHierarchyOutgoingCallsOrNull {
    pub call_hierarchy_outgoing_calls: Option<Vec<Option<CallHierarchyOutgoingCall>>>,
}

impl Serialize for CallHierarchyOutgoingCallsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(call_hierarchy_outgoing_calls) = &self.call_hierarchy_outgoing_calls {
            return call_hierarchy_outgoing_calls.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for CallHierarchyOutgoingCallsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let call_hierarchy_outgoing_calls =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    call_hierarchy_outgoing_calls: Some(call_hierarchy_outgoing_calls),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid CallHierarchyOutgoingCallsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InlineValuesOrNull {
    pub inline_values: Option<Vec<lsp_types_full::InlineValue>>,
}

impl Serialize for InlineValuesOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(inline_values) = &self.inline_values {
            return inline_values.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for InlineValuesOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let inline_values =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    inline_values: Some(inline_values),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid InlineValuesOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeHierarchyItem {
    pub name: String,
    pub kind: SymbolKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<lsp_types_full::SymbolTag>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub uri: DocumentUri,
    pub range: Range,
    pub selection_range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Map<String, serde_json::Value>>,
}

impl<'de> Deserialize<'de> for TypeHierarchyItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        require_fields::<D::Error>(&map, &["name", "kind", "uri", "range", "selectionRange"])?;

        let name = take_non_null_required_field(&mut map, "name")?;
        let kind = take_non_null_required_field(&mut map, "kind")?;
        let tags = take_non_null_optional_field(&mut map, "tags")?;
        let detail = take_non_null_optional_field(&mut map, "detail")?;
        let uri = take_non_null_required_field(&mut map, "uri")?;
        let range = take_non_null_required_field(&mut map, "range")?;
        let selection_range = take_non_null_required_field(&mut map, "selectionRange")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            name,
            kind,
            tags,
            detail,
            uri,
            range,
            selection_range,
            data,
        })
    }
}

impl HasLocation for TypeHierarchyItem {
    fn get_location(&self) -> Location {
        Location {
            uri: self.uri.clone(),
            range: self.range,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeHierarchyItemsOrNull {
    pub type_hierarchy_items: Option<Vec<Option<TypeHierarchyItem>>>,
}

impl Serialize for TypeHierarchyItemsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(type_hierarchy_items) = &self.type_hierarchy_items {
            return type_hierarchy_items.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for TypeHierarchyItemsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let type_hierarchy_items =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    type_hierarchy_items: Some(type_hierarchy_items),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid TypeHierarchyItemsOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeHierarchySupertypesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<IntegerOrString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_result_token: Option<IntegerOrString>,
    pub item: TypeHierarchyItem,
}

impl<'de> Deserialize<'de> for TypeHierarchySupertypesParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_type_hierarchy_sub_or_super_params(deserializer)
    }
}

pub type TypeHierarchySubtypesParams = TypeHierarchySupertypesParams;

fn deserialize_type_hierarchy_sub_or_super_params<'de, D>(
    deserializer: D,
) -> Result<TypeHierarchySupertypesParams, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let serde_json::Value::Object(mut map) = value else {
        return Err(serde::de::Error::custom("expected object start"));
    };

    require_fields::<D::Error>(&map, &["item"])?;

    let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
    let partial_result_token = take_non_null_optional_field(&mut map, "partialResultToken")?;
    let item = take_non_null_required_field(&mut map, "item")?;

    Ok(TypeHierarchySupertypesParams {
        work_done_token,
        partial_result_token,
        item,
    })
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkDoneProgressOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_done_progress: Option<bool>,
}

impl<'de> Deserialize<'de> for WorkDoneProgressOptions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let work_done_progress = take_non_null_optional_field(&mut map, "workDoneProgress")?;

        Ok(Self { work_done_progress })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hover {
    pub contents: HoverContents,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_increase_verbosity: bool,
}

impl<'de> Deserialize<'de> for Hover {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if !map.contains_key("contents") {
            return Err(serde::de::Error::custom(err_missing(&[
                "contents".to_string()
            ])));
        }

        let contents = serde_json::from_value(
            map.remove("contents")
                .expect("contents is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let range = take_non_null_optional_field(&mut map, "range")?;
        let can_increase_verbosity = map
            .remove("canIncreaseVerbosity")
            .map(serde_json::from_value)
            .transpose()
            .map_err(serde::de::Error::custom)?
            .unwrap_or(false);

        Ok(Self {
            contents,
            range,
            can_increase_verbosity,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HoverOrNull {
    pub hover: Option<Hover>,
}

impl Serialize for HoverOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(hover) = &self.hover {
            return hover.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for HoverOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let hover = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self { hover: Some(hover) })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid HoverOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignatureHelpOrNull {
    pub signature_help: Option<SignatureHelp>,
}

impl Serialize for SignatureHelpOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(signature_help) = &self.signature_help {
            return signature_help.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for SignatureHelpOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let signature_help =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    signature_help: Some(signature_help),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid SignatureHelpOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MessageActionItemOrNull {
    pub message_action_item: Option<lsp_types_full::MessageActionItem>,
}

impl Serialize for MessageActionItemOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(message_action_item) = &self.message_action_item {
            return message_action_item.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for MessageActionItemOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(_) => {
                let message_action_item =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    message_action_item: Some(message_action_item),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid MessageActionItemOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_details: Option<CompletionItemLabelDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<CompletionItemKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<CompletionItemTag>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<lsp_types_full::Documentation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preselect: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert_text_format: Option<InsertTextFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert_text_mode: Option<lsp_types_full::InsertTextMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_edit: Option<CompletionTextEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_edit_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_text_edits: Option<Vec<TextEdit>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_characters: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<CompletionItemData>,
}

impl<'de> Deserialize<'de> for CompletionItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        if !map.contains_key("label") {
            return Err(serde::de::Error::custom(err_missing(
                &["label".to_string()],
            )));
        }

        let label = serde_json::from_value(
            map.remove("label")
                .expect("label is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let label_details = take_non_null_optional_field(&mut map, "labelDetails")?;
        let kind = take_non_null_optional_field(&mut map, "kind")?;
        let tags = take_non_null_optional_field(&mut map, "tags")?;
        let detail = take_non_null_optional_field(&mut map, "detail")?;
        let documentation = take_non_null_optional_field(&mut map, "documentation")?;
        let deprecated = take_non_null_optional_field(&mut map, "deprecated")?;
        let preselect = take_non_null_optional_field(&mut map, "preselect")?;
        let sort_text = take_non_null_optional_field(&mut map, "sortText")?;
        let filter_text = take_non_null_optional_field(&mut map, "filterText")?;
        let insert_text = take_non_null_optional_field(&mut map, "insertText")?;
        let insert_text_format = take_non_null_optional_field(&mut map, "insertTextFormat")?;
        let insert_text_mode = take_non_null_optional_field(&mut map, "insertTextMode")?;
        let text_edit = take_non_null_optional_field(&mut map, "textEdit")?;
        let text_edit_text = take_non_null_optional_field(&mut map, "textEditText")?;
        let additional_text_edits = take_non_null_optional_field(&mut map, "additionalTextEdits")?;
        let commit_characters = take_non_null_optional_field(&mut map, "commitCharacters")?;
        let command = take_non_null_optional_field(&mut map, "command")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            label,
            label_details,
            kind,
            tags,
            detail,
            documentation,
            deprecated,
            preselect,
            sort_text,
            filter_text,
            insert_text,
            insert_text_format,
            insert_text_mode,
            text_edit,
            text_edit_text,
            additional_text_edits,
            commit_characters,
            command,
            data,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRange {
    pub start_line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_character: Option<u32>,
    pub end_line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_character: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<FoldingRangeKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed_text: Option<String>,
}

impl<'de> Deserialize<'de> for FoldingRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("startLine") {
            missing.push("startLine".to_string());
        }
        if !map.contains_key("endLine") {
            missing.push("endLine".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let start_line = serde_json::from_value(
            map.remove("startLine")
                .expect("startLine is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let start_character = take_non_null_optional_field(&mut map, "startCharacter")?;
        let end_line = serde_json::from_value(
            map.remove("endLine")
                .expect("endLine is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let end_character = take_non_null_optional_field(&mut map, "endCharacter")?;
        let kind = take_non_null_optional_field(&mut map, "kind")?;
        let collapsed_text = take_non_null_optional_field(&mut map, "collapsedText")?;

        Ok(Self {
            start_line,
            start_character,
            end_line,
            end_character,
            kind,
            collapsed_text,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FoldingRangesOrNull {
    pub folding_ranges: Option<Vec<Option<FoldingRange>>>,
}

impl Serialize for FoldingRangesOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(folding_ranges) = &self.folding_ranges {
            return folding_ranges.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for FoldingRangesOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let folding_ranges =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    folding_ranges: Some(folding_ranges),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid FoldingRangesOrNull: got {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlayHint {
    pub position: Position,
    pub label: InlayHintLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<InlayHintKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_edits: Option<Vec<TextEdit>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<InlayHintTooltip>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_left: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_right: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<LSPAny>,
}

impl<'de> Deserialize<'de> for InlayHint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let serde_json::Value::Object(mut map) = value else {
            return Err(serde::de::Error::custom("expected object start"));
        };

        let mut missing = Vec::new();
        if !map.contains_key("position") {
            missing.push("position".to_string());
        }
        if !map.contains_key("label") {
            missing.push("label".to_string());
        }
        if !missing.is_empty() {
            return Err(serde::de::Error::custom(err_missing(&missing)));
        }

        let position = serde_json::from_value(
            map.remove("position")
                .expect("position is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let label = serde_json::from_value(
            map.remove("label")
                .expect("label is present after required-field check"),
        )
        .map_err(serde::de::Error::custom)?;
        let kind = take_non_null_optional_field(&mut map, "kind")?;
        let text_edits = take_non_null_optional_field(&mut map, "textEdits")?;
        let tooltip = take_non_null_optional_field(&mut map, "tooltip")?;
        let padding_left = take_non_null_optional_field(&mut map, "paddingLeft")?;
        let padding_right = take_non_null_optional_field(&mut map, "paddingRight")?;
        let data = take_non_null_optional_field(&mut map, "data")?;

        Ok(Self {
            position,
            label,
            kind,
            text_edits,
            tooltip,
            padding_left,
            padding_right,
            data,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct InlayHintsOrNull {
    pub inlay_hints: Option<Vec<Option<InlayHint>>>,
}

impl Serialize for InlayHintsOrNull {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(inlay_hints) = &self.inlay_hints {
            return inlay_hints.serialize(serializer);
        }
        serializer.serialize_none()
    }
}

impl<'de> Deserialize<'de> for InlayHintsOrNull {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Array(_) => {
                let inlay_hints =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    inlay_hints: Some(inlay_hints),
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid InlayHintsOrNull: got {other}"
            ))),
        }
    }
}

fn take_non_null_optional_field<T, E>(
    map: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<Option<T>, E>
where
    T: serde::de::DeserializeOwned,
    E: serde::de::Error,
{
    let Some(value) = map.remove(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Err(E::custom(err_null(field)));
    }
    serde_json::from_value(value).map(Some).map_err(E::custom)
}

fn take_non_null_required_field<T, E>(
    map: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<T, E>
where
    T: serde::de::DeserializeOwned,
    E: serde::de::Error,
{
    let value = map
        .remove(field)
        .expect("field is present after required-field check");
    if value.is_null() {
        return Err(E::custom(err_null(field)));
    }
    serde_json::from_value(value).map_err(E::custom)
}

fn require_fields<E>(
    map: &serde_json::Map<String, serde_json::Value>,
    fields: &[&str],
) -> Result<(), E>
where
    E: serde::de::Error,
{
    let missing: Vec<String> = fields
        .iter()
        .filter(|field| !map.contains_key(**field))
        .map(|field| (*field).to_string())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(E::custom(err_missing(&missing)))
    }
}

fn deserialize_text_document_position_progress_params<'de, D>(
    deserializer: D,
) -> Result<ImplementationParams, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let serde_json::Value::Object(mut map) = value else {
        return Err(serde::de::Error::custom("expected object start"));
    };

    require_fields::<D::Error>(&map, &["textDocument", "position"])?;

    let text_document = take_non_null_required_field(&mut map, "textDocument")?;
    let position = take_non_null_required_field(&mut map, "position")?;
    let work_done_token = take_non_null_optional_field(&mut map, "workDoneToken")?;
    let partial_result_token = take_non_null_optional_field(&mut map, "partialResultToken")?;
    Ok(ImplementationParams {
        text_document,
        position,
        work_done_token,
        partial_result_token,
    })
}

pub trait DocumentUriExt {
    fn file_name(&self) -> String;
    fn path(&self, use_case_sensitive_file_names: bool) -> tspath::Path;
}

impl DocumentUriExt for DocumentUri {
    fn file_name(&self) -> String {
        if bundled::is_bundled(self) {
            return self.clone();
        }
        if self.starts_with("file://") {
            let parsed =
                url::Url::parse(self).unwrap_or_else(|_| panic!("invalid file URI: {self}"));
            let path = percent_decode_path(parsed.path());
            if let Some(authority) = file_uri_authority(self)
                && !authority.is_empty()
            {
                return format!("//{authority}{path}");
            }
            return fix_windows_uri_path(&path);
        }

        // Leave all other URIs escaped so we can round-trip them.

        let (scheme, path) = self
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid URI: {self}"));

        let mut authority = "ts-nul-authority";
        let mut path = path;
        if let Some(rest) = path.strip_prefix("//") {
            let (found_authority, found_path) = rest
                .split_once('/')
                .unwrap_or_else(|| panic!("invalid URI: {self}"));
            authority = found_authority;
            path = found_path;
        }

        format!("^/{scheme}/{authority}/{path}")
    }

    fn path(&self, use_case_sensitive_file_names: bool) -> tspath::Path {
        tspath::to_path(&self.file_name(), "", use_case_sensitive_file_names)
    }
}

pub fn fix_windows_uri_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('/') {
        let (volume, rest) = split_volume_path(rest);
        if !volume.is_empty() {
            return format!("{volume}{rest}");
        }
    }
    path.to_string()
}

fn file_uri_authority(uri: &str) -> Option<&str> {
    let rest = uri.strip_prefix("file://")?;
    if rest.starts_with('/') {
        return None;
    }
    let end = rest
        .find(|ch| matches!(ch, '/' | '?' | '#'))
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

fn percent_decode_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                decoded.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub trait HasTextDocumentUri {
    fn text_document_uri(&self) -> DocumentUri;
}

pub trait HasTextDocumentPosition: HasTextDocumentUri {
    fn text_document_position(&self) -> Position;
}

impl HasTextDocumentUri for ReferenceParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.to_string()
    }
}

impl HasTextDocumentPosition for ReferenceParams {
    fn text_document_position(&self) -> Position {
        Position {
            line: self.position.line,
            character: self.position.character,
        }
    }
}

impl HasTextDocumentUri for RenameParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentPosition for RenameParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

impl HasTextDocumentUri for ImplementationParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.to_string()
    }
}

impl HasTextDocumentUri for TextDocumentPositionParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentPosition for TextDocumentPositionParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

impl HasTextDocumentUri for DefinitionParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentPosition for DefinitionParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

impl HasTextDocumentUri for CompletionParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentPosition for CompletionParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

impl HasTextDocumentUri for DocumentDiagnosticParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.to_string()
    }
}

impl HasTextDocumentUri for CodeActionParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentUri for HoverParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.to_string()
    }
}

impl HasTextDocumentPosition for HoverParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

impl HasTextDocumentUri for SignatureHelpParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentPosition for SignatureHelpParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

impl HasTextDocumentUri for DocumentFormattingParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentUri for DocumentRangeFormattingParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentUri for DocumentOnTypeFormattingParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentPosition for DocumentOnTypeFormattingParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

impl HasTextDocumentUri for CallHierarchyPrepareParams {
    fn text_document_uri(&self) -> DocumentUri {
        self.text_document.uri.clone()
    }
}

impl HasTextDocumentPosition for CallHierarchyPrepareParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

impl HasTextDocumentPosition for ImplementationParams {
    fn text_document_position(&self) -> Position {
        self.position
    }
}

pub trait HasLocations {
    fn get_locations(&self) -> &Vec<Location>;
}

pub trait HasLocation {
    fn get_location(&self) -> Location;
}

pub fn unmarshal_ptr_to<T>(data: &[u8]) -> Result<Box<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::from_slice(data)
        .map_err(|err| format!("failed to unmarshal {}: {err}", std::any::type_name::<T>()))?;
    Ok(Box::new(value))
}

pub fn unmarshal_value<T>(data: &[u8]) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_slice(data)
        .map_err(|err| format!("failed to unmarshal {}: {err}", std::any::type_name::<T>()))
}

pub fn unmarshal_any(data: &[u8]) -> Result<serde_json::Value, String> {
    let mut value = serde_json::Value::Null;
    json::unmarshal(data, &mut value, &[])
        .map_err(|err| format!("failed to unmarshal any: {err}"))?;
    Ok(value)
}

pub fn unmarshal_empty(data: &[u8]) -> Result<(), String> {
    if !data.is_empty() {
        return Err(format!(
            "expected empty, got: {}",
            String::from_utf8_lossy(data)
        ));
    }
    Ok(())
}

pub fn bool_to_int(value: bool) -> i32 {
    if value { 1 } else { 0 }
}

pub fn err_not_object(kind: json::Kind) -> String {
    format!("expected object start, but encountered {kind}")
}

pub fn err_null(field: &str) -> String {
    format!("null value is not allowed for field {field:?}")
}

pub fn err_missing(props: &[String]) -> String {
    format!("missing required properties: {}", props.join(", "))
}

pub fn err_invalid_kind(type_name: &str, got: json::Kind) -> String {
    format!("invalid {type_name}: got {got}")
}

pub fn err_invalid_value(type_name: &str, data: &[u8]) -> String {
    format!("invalid {type_name}: {}", String::from_utf8_lossy(data))
}

pub fn err_literal_mismatch(type_name: &str, expected: &str, got: &[u8]) -> String {
    format!(
        "expected {type_name} value {expected}, got {}",
        String::from_utf8_lossy(got)
    )
}

pub fn assert_only_one(message: &str, count: i32) {
    assert!(count == 1, "{message}");
}

pub fn assert_at_most_one(message: &str, count: i32) {
    assert!(count <= 1, "{message}");
}

pub fn json_key_check(name: &[u8], key: &str) -> bool {
    // jsonKeyCheck compares a raw JSON key token (including quotes) against a Go string.
    name.len() == key.len() + 2
        && name.first() == Some(&b'"')
        && name.last() == Some(&b'"')
        && String::from_utf8_lossy(&name[1..name.len() - 1]) == key
}

pub fn json_object_raw_field(data: &[u8], field: &str) -> json::Value {
    // jsonObjectRawField scans the top-level keys of a JSON object looking for the
    // given field name, and returns its raw JSON value (e.g. `"full"` with quotes).
    // Returns nil if the field is not found.
    let value: serde_json::Value = match serde_json::from_slice(data) {
        Ok(value) => value,
        Err(_) => return serde_json::Value::Null,
    };
    match value {
        serde_json::Value::Object(map) => {
            map.get(field).cloned().unwrap_or(serde_json::Value::Null)
        }
        _ => serde_json::Value::Null,
    }
}

fn json_object_raw_field_from_value(value: &serde_json::Value, field: &str) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            map.get(field).cloned().unwrap_or(serde_json::Value::Null)
        }
        _ => serde_json::Value::Null,
    }
}

fn json_object_has_key_from_value(value: &serde_json::Value, keys: &[&str]) -> i32 {
    let serde_json::Value::Object(map) = value else {
        return -1;
    };
    for (index, key) in keys.iter().enumerate() {
        if map.contains_key(*key) {
            return index as i32;
        }
    }
    -1
}

pub fn json_object_has_key(data: &[u8], keys: &[&str]) -> i32 {
    let value: serde_json::Value = match serde_json::from_slice(data) {
        Ok(value) => value,
        Err(_) => return -1,
    };
    let serde_json::Value::Object(map) = value else {
        return -1;
    };
    for (index, key) in keys.iter().enumerate() {
        if map.contains_key(*key) {
            return index as i32;
        }
    }
    -1
}

pub struct RequestInfo<Params, Resp> {
    _params: PhantomData<Params>,
    _resp: PhantomData<Resp>,
    pub method: Method,
}

impl<Params, Resp> Clone for RequestInfo<Params, Resp> {
    fn clone(&self) -> Self {
        Self {
            _params: PhantomData,
            _resp: PhantomData,
            method: self.method.clone(),
        }
    }
}

impl<Params, Resp> RequestInfo<Params, Resp> {
    pub fn unmarshal_result(&self, result: serde_json::Value) -> Result<Resp, String>
    where
        Resp: serde::de::DeserializeOwned,
    {
        if let Ok(value) = serde_json::from_value(result.clone()) {
            return Ok(value);
        }
        let result = unmarshal_result(self.method.clone(), result)?;
        serde_json::from_value(result).map_err(|err| err.to_string())
    }

    pub fn new_request_message(
        &self,
        id: Option<jsonrpc::Id>,
        params: Params,
    ) -> super::RequestMessage
    where
        Params: Serialize,
    {
        super::RequestMessage {
            jsonrpc: jsonrpc::JsonRpcVersion,
            id,
            method: self.method.clone(),
            params: serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        }
    }
}

pub struct NotificationInfo<Params> {
    _params: PhantomData<Params>,
    pub method: Method,
}

impl<Params> Clone for NotificationInfo<Params> {
    fn clone(&self) -> Self {
        Self {
            _params: PhantomData,
            method: self.method.clone(),
        }
    }
}

impl<Params> NotificationInfo<Params> {
    pub fn new_notification_message(&self, params: Params) -> super::RequestMessage
    where
        Params: Serialize,
    {
        super::RequestMessage {
            jsonrpc: jsonrpc::JsonRpcVersion,
            id: None,
            method: self.method.clone(),
            params: serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Null;

impl<'de> serde::Deserialize<'de> for Null {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = serde_json::Value::deserialize(deserializer)?;
        if data != serde_json::Value::Null {
            return Err(serde::de::Error::custom(format!(
                "expected null, got {data}"
            )));
        }
        Ok(Self)
    }
}

impl serde::Serialize for Null {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_unit()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoParams;

impl NoParams {
    pub fn is_zero(&self) -> bool {
        true
    }
}

impl<'de> serde::Deserialize<'de> for NoParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = serde_json::Value::deserialize(deserializer)?;
        match data {
            serde_json::Value::Null => Ok(Self),
            serde_json::Value::Object(map) if map.is_empty() => Ok(Self),
            other => Err(serde::de::Error::custom(format!(
                "expected null or empty object for NoParams, got {other}"
            ))),
        }
    }
}

impl serde::Serialize for NoParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_unit()
    }
}

pub fn with_client_capabilities(
    ctx: core::Context,
    caps: &ResolvedClientCapabilities,
) -> core::Context {
    if let Ok(mut store) = client_capability_store().lock() {
        store.insert(core::get_request_id(&ctx), caps.clone());
    }
    ctx
}

pub fn get_client_capabilities(ctx: &core::Context) -> ResolvedClientCapabilities {
    let Ok(store) = client_capability_store().lock() else {
        return ResolvedClientCapabilities::default();
    };
    store
        .get(&core::get_request_id(ctx))
        .cloned()
        .unwrap_or_default()
}

pub fn preferred_markup_kind(formats: &[MarkupKind]) -> MarkupKind {
    if let Some(format) = formats.first() {
        return format.clone();
    }
    MarkupKindPlainText
}

pub trait ClientCapabilitiesExt {
    fn resolve(&self) -> ResolvedClientCapabilities;
}

impl ClientCapabilitiesExt for ClientCapabilities {
    fn resolve(&self) -> ResolvedClientCapabilities {
        ResolvedClientCapabilities {
            workspace: self
                .workspace
                .as_ref()
                .map(resolve_workspace_client_capabilities)
                .unwrap_or_default(),
            text_document: self
                .text_document
                .as_ref()
                .map(resolve_text_document_client_capabilities)
                .unwrap_or_default(),
            window: self
                .window
                .as_ref()
                .map(resolve_window_client_capabilities)
                .unwrap_or_default(),
            general: self
                .general
                .as_ref()
                .map(resolve_general_client_capabilities)
                .unwrap_or_default(),
            vs_supports_visual_studio_extensions: false,
            vs_supported_snippet_version: 0,
            vs_supports_not_including_text_in_text_document_did_open: false,
            vs_supports_icon_extensions: false,
            vs_supports_diagnostic_requests: false,
        }
    }
}

fn resolve_text_document_client_capabilities(
    v: &lsp_types_full::TextDocumentClientCapabilities,
) -> ResolvedTextDocumentClientCapabilities {
    ResolvedTextDocumentClientCapabilities {
        completion: v
            .completion
            .as_ref()
            .map(resolve_completion_client_capabilities)
            .unwrap_or_default(),
        definition: v
            .definition
            .as_ref()
            .map(resolve_goto_client_capabilities)
            .unwrap_or_default(),
        type_definition: v
            .type_definition
            .as_ref()
            .map(resolve_goto_client_capabilities)
            .unwrap_or_default(),
        implementation: v
            .implementation
            .as_ref()
            .map(resolve_goto_client_capabilities)
            .unwrap_or_default(),
        document_symbol: v
            .document_symbol
            .as_ref()
            .map(resolve_document_symbol_client_capabilities)
            .unwrap_or_default(),
        hover: v
            .hover
            .as_ref()
            .map(resolve_hover_client_capabilities)
            .unwrap_or_default(),
        signature_help: v
            .signature_help
            .as_ref()
            .map(resolve_signature_help_client_capabilities)
            .unwrap_or_default(),
        folding_range: v
            .folding_range
            .as_ref()
            .map(resolve_folding_range_client_capabilities)
            .unwrap_or_default(),
        semantic_tokens: v
            .semantic_tokens
            .as_ref()
            .map(resolve_semantic_tokens_client_capabilities)
            .unwrap_or_default(),
        diagnostic: v
            .diagnostic
            .as_ref()
            .map(resolve_diagnostic_client_capabilities)
            .unwrap_or_default(),
        publish_diagnostics: v
            .publish_diagnostics
            .as_ref()
            .map(resolve_publish_diagnostics_client_capabilities)
            .unwrap_or_default(),
    }
}

fn resolve_goto_client_capabilities(
    v: &lsp_types_full::GotoCapability,
) -> ResolvedGotoClientCapabilities {
    ResolvedGotoClientCapabilities {
        link_support: v.link_support.unwrap_or_default(),
    }
}

fn resolve_document_symbol_client_capabilities(
    v: &lsp_types_full::DocumentSymbolClientCapabilities,
) -> ResolvedDocumentSymbolClientCapabilities {
    ResolvedDocumentSymbolClientCapabilities {
        hierarchical_document_symbol_support: v
            .hierarchical_document_symbol_support
            .unwrap_or_default(),
    }
}

fn resolve_completion_client_capabilities(
    v: &lsp_types_full::CompletionClientCapabilities,
) -> ResolvedCompletionClientCapabilities {
    ResolvedCompletionClientCapabilities {
        completion_item: v
            .completion_item
            .as_ref()
            .map(resolve_completion_item_client_capabilities)
            .unwrap_or_default(),
        completion_list: v
            .completion_list
            .as_ref()
            .map(resolve_completion_list_client_capabilities)
            .unwrap_or_default(),
    }
}

fn resolve_completion_item_client_capabilities(
    v: &lsp_types_full::CompletionItemCapability,
) -> ResolvedCompletionItemClientCapabilities {
    ResolvedCompletionItemClientCapabilities {
        label_details_support: v.label_details_support.unwrap_or_default(),
        snippet_support: v.snippet_support.unwrap_or_default(),
        commit_characters_support: v.commit_characters_support.unwrap_or_default(),
        insert_replace_support: v.insert_replace_support.unwrap_or_default(),
        documentation_format: v.documentation_format.clone().unwrap_or_default(),
    }
}

fn resolve_completion_list_client_capabilities(
    v: &lsp_types_full::CompletionListCapability,
) -> ResolvedCompletionListClientCapabilities {
    ResolvedCompletionListClientCapabilities {
        item_defaults: v.item_defaults.clone().unwrap_or_default(),
    }
}

fn resolve_hover_client_capabilities(
    v: &lsp_types_full::HoverClientCapabilities,
) -> ResolvedHoverClientCapabilities {
    ResolvedHoverClientCapabilities {
        content_format: v.content_format.clone().unwrap_or_default(),
        verbosity_level: false,
    }
}

fn resolve_signature_help_client_capabilities(
    v: &lsp_types_full::SignatureHelpClientCapabilities,
) -> ResolvedSignatureHelpClientCapabilities {
    ResolvedSignatureHelpClientCapabilities {
        signature_information: v
            .signature_information
            .as_ref()
            .map(resolve_signature_information_client_capabilities)
            .unwrap_or_default(),
    }
}

fn resolve_signature_information_client_capabilities(
    v: &lsp_types_full::SignatureInformationSettings,
) -> ResolvedSignatureInformationClientCapabilities {
    let active_parameter_support = v.active_parameter_support.unwrap_or_default();
    ResolvedSignatureInformationClientCapabilities {
        documentation_format: v.documentation_format.clone().unwrap_or_default(),
        active_parameter_support,
        no_active_parameter_support: !active_parameter_support,
    }
}

fn resolve_folding_range_client_capabilities(
    v: &lsp_types_full::FoldingRangeClientCapabilities,
) -> ResolvedFoldingRangeClientCapabilities {
    ResolvedFoldingRangeClientCapabilities {
        line_folding_only: v.line_folding_only.unwrap_or_default(),
        folding_range: v
            .folding_range
            .as_ref()
            .map(resolve_folding_range_client_capabilities_inner)
            .unwrap_or_default(),
    }
}

fn resolve_folding_range_client_capabilities_inner(
    v: &lsp_types_full::FoldingRangeCapability,
) -> ResolvedFoldingRangeClientCapabilitiesInner {
    ResolvedFoldingRangeClientCapabilitiesInner {
        collapsed_text: v.collapsed_text.unwrap_or_default(),
    }
}

fn resolve_semantic_tokens_client_capabilities(
    v: &lsp_types_full::SemanticTokensClientCapabilities,
) -> ResolvedSemanticTokensClientCapabilities {
    ResolvedSemanticTokensClientCapabilities {
        token_types: v
            .token_types
            .iter()
            .map(|token_type| token_type.as_str().to_string())
            .collect(),
        token_modifiers: v
            .token_modifiers
            .iter()
            .map(|token_modifier| token_modifier.as_str().to_string())
            .collect(),
    }
}

fn resolve_diagnostic_client_capabilities(
    v: &lsp_types_full::DiagnosticClientCapabilities,
) -> ResolvedDiagnosticClientCapabilities {
    ResolvedDiagnosticClientCapabilities {
        related_information: v.related_document_support.unwrap_or_default(),
        tag_support: ResolvedTagSupport::default(),
    }
}

fn resolve_publish_diagnostics_client_capabilities(
    v: &lsp_types_full::PublishDiagnosticsClientCapabilities,
) -> ResolvedDiagnosticClientCapabilities {
    ResolvedDiagnosticClientCapabilities {
        related_information: v.related_information.unwrap_or_default(),
        tag_support: v
            .tag_support
            .as_ref()
            .map(|tag_support| ResolvedTagSupport {
                value_set: tag_support
                    .value_set
                    .iter()
                    .filter_map(resolve_diagnostic_tag)
                    .collect(),
            })
            .unwrap_or_default(),
    }
}

fn resolve_diagnostic_tag(tag: &lsp_types_full::DiagnosticTag) -> Option<DiagnosticTag> {
    if tag == &lsp_types_full::DiagnosticTag::UNNECESSARY {
        return Some(DiagnosticTag::Unnecessary);
    }
    if tag == &lsp_types_full::DiagnosticTag::DEPRECATED {
        return Some(DiagnosticTag::Deprecated);
    }
    None
}

fn resolve_workspace_client_capabilities(
    v: &lsp_types_full::WorkspaceClientCapabilities,
) -> ResolvedWorkspaceClientCapabilities {
    ResolvedWorkspaceClientCapabilities {
        workspace_edit: v
            .workspace_edit
            .as_ref()
            .map(resolve_workspace_edit_client_capabilities)
            .unwrap_or_default(),
        did_change_configuration: v
            .did_change_configuration
            .as_ref()
            .map(resolve_did_change_configuration_client_capabilities)
            .unwrap_or_default(),
        did_change_watched_files: v
            .did_change_watched_files
            .as_ref()
            .map(resolve_did_change_watched_files_client_capabilities)
            .unwrap_or_default(),
        file_operations: v
            .file_operations
            .as_ref()
            .map(resolve_workspace_file_operations_client_capabilities)
            .unwrap_or_default(),
        workspace_folders: v.workspace_folders.unwrap_or_default(),
        configuration: v.configuration.unwrap_or_default(),
        code_lens: v
            .code_lens
            .as_ref()
            .map(resolve_code_lens_workspace_client_capabilities)
            .unwrap_or_default(),
        inlay_hint: v
            .inlay_hint
            .as_ref()
            .map(resolve_inlay_hint_workspace_client_capabilities)
            .unwrap_or_default(),
        diagnostics: v
            .diagnostic
            .as_ref()
            .map(resolve_diagnostic_workspace_client_capabilities)
            .unwrap_or_default(),
    }
}

fn resolve_workspace_edit_client_capabilities(
    v: &lsp_types_full::WorkspaceEditClientCapabilities,
) -> ResolvedWorkspaceEditClientCapabilities {
    ResolvedWorkspaceEditClientCapabilities {
        document_changes: v.document_changes.unwrap_or_default(),
        resource_operations: v.resource_operations.clone().unwrap_or_default(),
    }
}

fn resolve_did_change_configuration_client_capabilities(
    v: &lsp_types_full::DidChangeConfigurationClientCapabilities,
) -> ResolvedDidChangeConfigurationClientCapabilities {
    ResolvedDidChangeConfigurationClientCapabilities {
        dynamic_registration: v.dynamic_registration.unwrap_or_default(),
    }
}

fn resolve_did_change_watched_files_client_capabilities(
    v: &lsp_types_full::DidChangeWatchedFilesClientCapabilities,
) -> ResolvedDidChangeWatchedFilesClientCapabilities {
    ResolvedDidChangeWatchedFilesClientCapabilities {
        dynamic_registration: v.dynamic_registration.unwrap_or_default(),
        relative_pattern_support: v.relative_pattern_support.unwrap_or_default(),
    }
}

fn resolve_workspace_file_operations_client_capabilities(
    v: &lsp_types_full::WorkspaceFileOperationsClientCapabilities,
) -> ResolvedWorkspaceFileOperationsClientCapabilities {
    ResolvedWorkspaceFileOperationsClientCapabilities {
        will_rename: v.will_rename.unwrap_or_default(),
    }
}

fn resolve_code_lens_workspace_client_capabilities(
    v: &lsp_types_full::CodeLensWorkspaceClientCapabilities,
) -> ResolvedCodeLensWorkspaceClientCapabilities {
    ResolvedCodeLensWorkspaceClientCapabilities {
        refresh_support: v.refresh_support.unwrap_or_default(),
    }
}

fn resolve_inlay_hint_workspace_client_capabilities(
    v: &lsp_types_full::InlayHintWorkspaceClientCapabilities,
) -> ResolvedInlayHintWorkspaceClientCapabilities {
    ResolvedInlayHintWorkspaceClientCapabilities {
        refresh_support: v.refresh_support.unwrap_or_default(),
    }
}

fn resolve_diagnostic_workspace_client_capabilities(
    v: &lsp_types_full::DiagnosticWorkspaceClientCapabilities,
) -> ResolvedDiagnosticWorkspaceClientCapabilities {
    ResolvedDiagnosticWorkspaceClientCapabilities {
        refresh_support: v.refresh_support.unwrap_or_default(),
    }
}

fn resolve_window_client_capabilities(
    v: &lsp_types_full::WindowClientCapabilities,
) -> ResolvedWindowClientCapabilities {
    ResolvedWindowClientCapabilities {
        work_done_progress: v.work_done_progress.unwrap_or_default(),
    }
}

fn resolve_general_client_capabilities(
    v: &lsp_types_full::GeneralClientCapabilities,
) -> ResolvedGeneralClientCapabilities {
    ResolvedGeneralClientCapabilities {
        position_encodings: v
            .position_encodings
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|encoding| match encoding.as_str() {
                "utf-8" => PositionEncodingKindUTF8,
                "utf-16" => PositionEncodingKindUTF16,
                "utf-32" => PositionEncodingKindUTF32,
                _ => PositionEncodingKindUTF16,
            })
            .collect(),
    }
}

pub const CodeActionKindSourceRemoveUnusedImports: CodeActionKind =
    CodeActionKind::SourceRemoveUnusedImports;
pub const CodeActionKindSourceSortImports: CodeActionKind = CodeActionKind::SourceSortImports;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedClientCapabilities {
    pub workspace: ResolvedWorkspaceClientCapabilities,
    pub text_document: ResolvedTextDocumentClientCapabilities,
    pub window: ResolvedWindowClientCapabilities,
    pub general: ResolvedGeneralClientCapabilities,
    #[serde(rename = "_vs_supportsVisualStudioExtensions")]
    pub vs_supports_visual_studio_extensions: bool,
    #[serde(rename = "_vs_supportedSnippetVersion")]
    pub vs_supported_snippet_version: i32,
    #[serde(rename = "_vs_supportsNotIncludingTextInTextDocumentDidOpen")]
    pub vs_supports_not_including_text_in_text_document_did_open: bool,
    #[serde(rename = "_vs_supportsIconExtensions")]
    pub vs_supports_icon_extensions: bool,
    #[serde(rename = "_vs_supportsDiagnosticRequests")]
    pub vs_supports_diagnostic_requests: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedWorkspaceClientCapabilities {
    pub workspace_edit: ResolvedWorkspaceEditClientCapabilities,
    pub did_change_configuration: ResolvedDidChangeConfigurationClientCapabilities,
    pub did_change_watched_files: ResolvedDidChangeWatchedFilesClientCapabilities,
    pub file_operations: ResolvedWorkspaceFileOperationsClientCapabilities,
    pub workspace_folders: bool,
    pub configuration: bool,
    pub code_lens: ResolvedCodeLensWorkspaceClientCapabilities,
    pub inlay_hint: ResolvedInlayHintWorkspaceClientCapabilities,
    pub diagnostics: ResolvedDiagnosticWorkspaceClientCapabilities,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDidChangeConfigurationClientCapabilities {
    pub dynamic_registration: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDidChangeWatchedFilesClientCapabilities {
    pub dynamic_registration: bool,
    pub relative_pattern_support: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedWorkspaceEditClientCapabilities {
    pub document_changes: bool,
    pub resource_operations: Vec<ResourceOperationKind>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedWorkspaceFileOperationsClientCapabilities {
    pub will_rename: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCodeLensWorkspaceClientCapabilities {
    pub refresh_support: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedInlayHintWorkspaceClientCapabilities {
    pub refresh_support: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDiagnosticWorkspaceClientCapabilities {
    pub refresh_support: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTextDocumentClientCapabilities {
    pub completion: ResolvedCompletionClientCapabilities,
    pub definition: ResolvedGotoClientCapabilities,
    pub type_definition: ResolvedGotoClientCapabilities,
    pub implementation: ResolvedGotoClientCapabilities,
    pub document_symbol: ResolvedDocumentSymbolClientCapabilities,
    pub hover: ResolvedHoverClientCapabilities,
    pub signature_help: ResolvedSignatureHelpClientCapabilities,
    pub folding_range: ResolvedFoldingRangeClientCapabilities,
    pub semantic_tokens: ResolvedSemanticTokensClientCapabilities,
    pub diagnostic: ResolvedDiagnosticClientCapabilities,
    pub publish_diagnostics: ResolvedDiagnosticClientCapabilities,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedGotoClientCapabilities {
    pub link_support: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDocumentSymbolClientCapabilities {
    pub hierarchical_document_symbol_support: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCompletionClientCapabilities {
    pub completion_item: ResolvedCompletionItemClientCapabilities,
    pub completion_list: ResolvedCompletionListClientCapabilities,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCompletionItemClientCapabilities {
    pub label_details_support: bool,
    pub snippet_support: bool,
    pub commit_characters_support: bool,
    pub insert_replace_support: bool,
    pub documentation_format: Vec<MarkupKind>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCompletionListClientCapabilities {
    pub item_defaults: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedHoverClientCapabilities {
    pub content_format: Vec<MarkupKind>,
    pub verbosity_level: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSignatureHelpClientCapabilities {
    pub signature_information: ResolvedSignatureInformationClientCapabilities,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSignatureInformationClientCapabilities {
    pub documentation_format: Vec<MarkupKind>,
    pub active_parameter_support: bool,
    pub no_active_parameter_support: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFoldingRangeClientCapabilities {
    pub line_folding_only: bool,
    pub folding_range: ResolvedFoldingRangeClientCapabilitiesInner,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFoldingRangeClientCapabilitiesInner {
    pub collapsed_text: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSemanticTokensClientCapabilities {
    pub token_types: Vec<String>,
    pub token_modifiers: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDiagnosticClientCapabilities {
    pub related_information: bool,
    pub tag_support: ResolvedTagSupport<DiagnosticTag>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTagSupport<T> {
    pub value_set: Vec<T>,
}

impl<T> Default for ResolvedTagSupport<T> {
    fn default() -> Self {
        Self {
            value_set: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedWindowClientCapabilities {
    pub work_done_progress: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedGeneralClientCapabilities {
    pub position_encodings: Vec<PositionEncodingKind>,
}

fn client_capability_store() -> &'static Mutex<HashMap<String, ResolvedClientCapabilities>> {
    static STORE: OnceLock<Mutex<HashMap<String, ResolvedClientCapabilities>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn split_volume_path(file_name: &str) -> (String, String) {
    if file_name.len() >= 2
        && file_name.as_bytes()[0].is_ascii_alphabetic()
        && file_name.as_bytes()[1] == b':'
    {
        return (
            file_name[..2].to_ascii_lowercase(),
            file_name[2..].to_string(),
        );
    }
    (String::new(), file_name.to_string())
}
