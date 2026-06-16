use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;

use super::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CompletionItem, DocumentUri,
    FoldingRange, Hover, InitializeParams, InitializeResult, InlayHint, IntegerOrString, Location,
    MethodCallHierarchyIncomingCalls, MethodCallHierarchyOutgoingCalls,
    MethodCustomSaveAllocProfile, MethodCustomSaveHeapProfile, MethodCustomStopCPUProfile,
    MethodCustomTextDocumentMultiDocumentHighlight, MethodTextDocumentCodeLens,
    MethodTextDocumentDocumentLink, MethodTextDocumentInlayHint,
    MethodTextDocumentInlineCompletion, MethodTextDocumentInlineValue,
    MethodTextDocumentPrepareCallHierarchy, MethodTextDocumentPrepareTypeHierarchy,
    MethodTextDocumentVSOnAutoInsert, MethodTypeHierarchySubtypes, MethodTypeHierarchySupertypes,
    Position, Range, SemanticTokens, TextDocumentEdit, WorkDoneProgressBeginOrReportOrEnd,
    WorkDoneProgressOptions, unmarshal_result,
};

type InlayHintKind = lsp_types_full::InlayHintKind;
type FoldingRangeKind = lsp_types_full::FoldingRangeKind;
type ClientCapabilities = lsp_types_full::ClientCapabilities;
type ServerCapabilities = lsp_types_full::ServerCapabilities;
type TextEdit = lsp_types_full::TextEdit;
type TextEditOrInsertReplaceEdit =
    lsp_types_full::OneOf<TextEdit, lsp_types_full::InsertReplaceEdit>;
type TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile = lsp_types_full::DocumentChangeOperation;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializationOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    user_preferences: Option<serde_json::Value>,
}

type IntegerOrNull = Option<i32>;
type DocumentUriOrNull = Option<DocumentUri>;
type StringOrInlayHintLabelParts = lsp_types_full::InlayHintLabel;
type BooleanOrHoverOptions = lsp_types_full::OneOf<bool, lsp_types_full::HoverOptions>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct StringLiteralCreate;

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

fn unmarshal<T>(input: &str) -> serde_json::Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_str(input)
}

fn marshal<T>(value: &T) -> serde_json::Result<String>
where
    T: Serialize,
{
    serde_json::to_string(value)
}

fn assert_error_contains<T>(input: &str, err_text: &str)
where
    T: DeserializeOwned,
{
    let err = match unmarshal::<T>(input) {
        Ok(_) => panic!("expected unmarshal error for input {input}"),
        Err(err) => err,
    };
    let err = err.to_string();
    assert!(
        err.contains(err_text),
        "expected error containing {err_text:?}, got {err:?}"
    );
}

fn assert_unmarshal_error<T>(input: &str)
where
    T: DeserializeOwned,
{
    assert!(
        unmarshal::<T>(input).is_err(),
        "expected error for input {input}"
    );
}

fn lsp_position(line: u32, character: u32) -> Position {
    Position { line, character }
}

#[test]
fn test_unmarshal_rejects_null_for_optional_non_nullable_fields() {
    let tests = [
        (
            "InlayHint kind null",
            r#"{"position": {"line": 0, "character": 0}, "label": "foo", "kind": null}"#,
            "null value is not allowed for field \"kind\"",
        ),
        (
            "InlayHint textEdits null",
            r#"{"position": {"line": 0, "character": 0}, "label": "foo", "textEdits": null}"#,
            "null value is not allowed for field \"textEdits\"",
        ),
        (
            "InlayHint paddingLeft null",
            r#"{"position": {"line": 0, "character": 0}, "label": "foo", "paddingLeft": null}"#,
            "null value is not allowed for field \"paddingLeft\"",
        ),
    ];

    for (name, input, err_text) in tests {
        let _ = name;
        assert_error_contains::<InlayHint>(input, err_text);
    }

    let tests = [
        (
            "FoldingRange kind null",
            r#"{"startLine": 0, "endLine": 10, "kind": null}"#,
            "null value is not allowed for field \"kind\"",
        ),
        (
            "FoldingRange startCharacter null",
            r#"{"startLine": 0, "endLine": 10, "startCharacter": null}"#,
            "null value is not allowed for field \"startCharacter\"",
        ),
    ];

    for (name, input, err_text) in tests {
        let _ = name;
        assert_error_contains::<FoldingRange>(input, err_text);
    }

    assert_error_contains::<CompletionItem>(
        r#"{"label": "test", "insertTextFormat": null}"#,
        "null value is not allowed for field \"insertTextFormat\"",
    );
    assert_error_contains::<Hover>(
        r#"{"contents": {"kind": "plaintext", "value": "hi"}, "range": null}"#,
        "null value is not allowed for field \"range\"",
    );
    assert_error_contains::<WorkDoneProgressOptions>(
        r#"{"workDoneProgress": null}"#,
        "null value is not allowed for field \"workDoneProgress\"",
    );
    assert_error_contains::<CallHierarchyIncomingCallsParams>(
        r#"{"item": null}"#,
        "null value is not allowed for field \"item\"",
    );
    assert_error_contains::<CallHierarchyIncomingCall>(
        r#"{"from": null, "fromRanges": []}"#,
        "null value is not allowed for field \"from\"",
    );
    assert_error_contains::<InitializeParams>(
        r#"{"processId": null, "rootUri": null, "capabilities": null}"#,
        "null value is not allowed for field \"capabilities\"",
    );
    assert_error_contains::<InitializeResult>(
        r#"{"capabilities": null}"#,
        "null value is not allowed for field \"capabilities\"",
    );
    assert_error_contains::<SemanticTokens>(
        r#"{"data": null}"#,
        "null value is not allowed for field \"data\"",
    );
    assert_error_contains::<TextDocumentEdit>(
        r#"{"textDocument": {"uri": "file:///a.ts", "version": 1}, "edits": null}"#,
        "null value is not allowed for field \"edits\"",
    );
}

#[test]
fn test_unmarshal_accepts_null_for_nullable_fields() {
    let tests = [
        (
            "InitializeParams rootUri null",
            r#"{"processId": null, "rootUri": null, "capabilities": {}}"#,
        ),
        (
            "InitializeParams workspaceFolders null",
            r#"{"processId": null, "rootUri": null, "capabilities": {}, "workspaceFolders": null}"#,
        ),
        (
            "InitializeParams workspaceFolders null element",
            r#"{"processId": null, "rootUri": null, "capabilities": {}, "workspaceFolders": [null]}"#,
        ),
        (
            "InitializeParams processId null",
            r#"{"processId": null, "rootUri": null, "capabilities": {}}"#,
        ),
    ];

    for (name, input) in tests {
        let _ = name;
        assert!(unmarshal::<InitializeParams>(input).is_ok());
    }

    assert!(unmarshal::<InitializationOptions>(r#"{"userPreferences": null}"#).is_ok());
}

#[test]
fn test_unmarshal_accepts_omitted_optional_fields() {
    let hint: InlayHint =
        unmarshal(r#"{"position": {"line": 1, "character": 5}, "label": "test"}"#).unwrap();
    assert!(hint.kind.is_none());
    assert!(hint.text_edits.is_none());
    assert!(hint.tooltip.is_none());
    assert!(hint.padding_left.is_none());
    assert!(hint.padding_right.is_none());
    assert!(hint.data.is_none());
    assert_eq!(hint.position.line, 1);
    assert_eq!(hint.position.character, 5);

    let fr: FoldingRange = unmarshal(r#"{"startLine": 5, "endLine": 10}"#).unwrap();
    assert!(fr.kind.is_none());
    assert!(fr.start_character.is_none());
    assert!(fr.end_character.is_none());
    assert!(fr.collapsed_text.is_none());
    assert_eq!(fr.start_line, 5);
    assert_eq!(fr.end_line, 10);
}

#[test]
fn test_unmarshal_rejects_incomplete_objects() {
    assert_error_contains::<InlayHint>(
        r#"{"label": "test"}"#,
        "missing required properties: position",
    );
    assert_error_contains::<InlayHint>(
        r#"{"position": {"line": 0, "character": 0}}"#,
        "missing required properties: label",
    );
    assert_error_contains::<Location>(
        r#"{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}}"#,
        "missing required properties: uri",
    );
    assert_error_contains::<Location>(r#"{}"#, "missing required properties: uri, range");
}

#[test]
fn test_marshal_unmarshal_round_trip() {
    let value = InlayHint {
        position: lsp_position(1, 5),
        label: super::InlayHintLabel {
            string: Some("param".to_string()),
            inlay_hint_label_parts: None,
        },
        kind: Some(InlayHintKind::PARAMETER),
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: None,
        data: None,
    };
    let data = marshal(&value).unwrap();
    let result: InlayHint = unmarshal(&data).unwrap();
    assert_eq!(
        serde_json::to_value(&value).unwrap(),
        serde_json::to_value(&result).unwrap()
    );

    let value = InlayHint {
        position: lsp_position(0, 0),
        label: super::InlayHintLabel {
            string: Some("x".to_string()),
            inlay_hint_label_parts: None,
        },
        kind: None,
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: None,
        data: None,
    };
    let data = marshal(&value).unwrap();
    let result: InlayHint = unmarshal(&data).unwrap();
    assert_eq!(
        serde_json::to_value(&value).unwrap(),
        serde_json::to_value(&result).unwrap()
    );

    let value = FoldingRange {
        start_line: 1,
        start_character: Some(0),
        end_line: 10,
        end_character: Some(5),
        kind: Some(FoldingRangeKind::Region),
        collapsed_text: Some("...".to_string()),
    };
    let data = marshal(&value).unwrap();
    let result: FoldingRange = unmarshal(&data).unwrap();
    assert_eq!(value, result);

    let value = Location {
        uri: "file:///test.ts".to_string(),
        range: Range {
            start: Position {
                line: 1,
                character: 2,
            },
            end: Position {
                line: 3,
                character: 4,
            },
        },
    };
    let data = marshal(&value).unwrap();
    let result: Location = unmarshal(&data).unwrap();
    assert_eq!(value, result);

    let value = InitializeParams {
        process_id: None,
        root_uri: Some("file:///workspace".parse().unwrap()),
        capabilities: ClientCapabilities::default(),
        ..Default::default()
    };
    let data = marshal(&value).unwrap();
    let result: InitializeParams = unmarshal(&data).unwrap();
    assert_eq!(value, result);
}

#[test]
fn test_unmarshal_union_types() {
    let v: IntegerOrString = unmarshal("42").unwrap();
    assert_eq!(v.integer, Some(42));
    assert!(v.string.is_none());

    let v: IntegerOrString = unmarshal(r#""hello""#).unwrap();
    assert_eq!(v.string.as_deref(), Some("hello"));
    assert!(v.integer.is_none());

    let v: IntegerOrNull = unmarshal("42").unwrap();
    assert_eq!(v, Some(42));

    let v: IntegerOrNull = unmarshal("null").unwrap();
    assert!(v.is_none());

    let v: DocumentUriOrNull = unmarshal(r#""file:///test.ts""#).unwrap();
    assert_eq!(v.as_deref(), Some("file:///test.ts"));

    let v: DocumentUriOrNull = unmarshal("null").unwrap();
    assert!(v.is_none());
}

#[test]
fn test_marshal_union_types() {
    assert_eq!(marshal(&Some(42_i32)).unwrap(), "42");
    let v: IntegerOrNull = None;
    assert_eq!(marshal(&v).unwrap(), "null");
    assert_eq!(marshal(&IntegerOrString::from(7)).unwrap(), "7");
    assert_eq!(
        marshal(&IntegerOrString::from("tok".to_string())).unwrap(),
        r#""tok""#
    );
}

#[test]
fn test_unmarshal_ignores_unknown_fields() {
    let loc: Location = unmarshal(
        r#"{
            "uri": "file:///test.ts",
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}},
            "someUnknownField": 42,
            "anotherUnknown": {"nested": true}
        }"#,
    )
    .unwrap();
    assert_eq!(loc.uri, "file:///test.ts");

    let _: InlayHint = unmarshal(
        r#"{
            "position": {"line": 0, "character": 0},
            "label": "x",
            "futureField": [1, 2, 3]
        }"#,
    )
    .unwrap();
}

#[test]
fn test_unmarshal_rejects_wrong_types() {
    assert_unmarshal_error::<Location>("[]");
    assert_unmarshal_error::<Location>(r#""not an object""#);
    assert_unmarshal_error::<Location>("42");
    assert_unmarshal_error::<Location>("null");
    assert_unmarshal_error::<FoldingRange>("true");
}

#[test]
fn test_unmarshal_union_type_wrong_kind() {
    assert_unmarshal_error::<IntegerOrString>("true");
    assert_unmarshal_error::<IntegerOrString>("null");
    assert_unmarshal_error::<IntegerOrString>("{}");
    assert_unmarshal_error::<IntegerOrString>("[]");
    assert_unmarshal_error::<StringOrInlayHintLabelParts>("42");
    assert_unmarshal_error::<StringOrInlayHintLabelParts>("true");
}

#[test]
fn test_unmarshal_boolean_union_types() {
    let v: BooleanOrHoverOptions = unmarshal("true").unwrap();
    assert!(matches!(v, lsp_types_full::OneOf::Left(true)));

    let v: BooleanOrHoverOptions = unmarshal("false").unwrap();
    assert!(matches!(v, lsp_types_full::OneOf::Left(false)));

    let v: BooleanOrHoverOptions = unmarshal("{}").unwrap();
    assert!(matches!(v, lsp_types_full::OneOf::Right(_)));

    assert_unmarshal_error::<BooleanOrHoverOptions>(r#""nope""#);
}

#[test]
fn test_unmarshal_discriminator_union() {
    let v: WorkDoneProgressBeginOrReportOrEnd =
        unmarshal(r#"{"kind": "begin", "title": "Indexing"}"#).unwrap();
    assert!(v.begin.is_some());
    assert!(v.report.is_none());
    assert!(v.end.is_none());
    assert_eq!(v.begin.unwrap().title, "Indexing");

    let v: WorkDoneProgressBeginOrReportOrEnd =
        unmarshal(r#"{"kind": "report", "message": "50%"}"#).unwrap();
    assert!(v.begin.is_none());
    assert!(v.report.is_some());
    assert!(v.end.is_none());
    assert_eq!(v.report.unwrap().message.as_deref(), Some("50%"));

    let v: WorkDoneProgressBeginOrReportOrEnd = unmarshal(r#"{"kind": "end"}"#).unwrap();
    assert!(v.begin.is_none());
    assert!(v.report.is_none());
    assert!(v.end.is_some());

    assert_unmarshal_error::<WorkDoneProgressBeginOrReportOrEnd>(r#"{"kind": "invalid"}"#);
}

#[test]
fn test_unmarshal_presence_discriminator_union() {
    let v: TextEditOrInsertReplaceEdit = unmarshal(
        r#"{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}},
            "newText": "x"
        }"#,
    )
    .unwrap();
    match v {
        lsp_types_full::OneOf::Left(text_edit) => assert_eq!(text_edit.new_text, "x"),
        lsp_types_full::OneOf::Right(_) => panic!("expected TextEdit"),
    }

    let v: TextEditOrInsertReplaceEdit = unmarshal(
        r#"{
            "insert": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}},
            "replace": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 2}},
            "newText": "y"
        }"#,
    )
    .unwrap();
    match v {
        lsp_types_full::OneOf::Left(_) => panic!("expected InsertReplaceEdit"),
        lsp_types_full::OneOf::Right(insert_replace_edit) => {
            assert_eq!(insert_replace_edit.new_text, "y")
        }
    }
}

#[test]
fn test_unmarshal_string_or_array_union() {
    let v: StringOrInlayHintLabelParts = unmarshal(r#""hello""#).unwrap();
    match v {
        lsp_types_full::InlayHintLabel::String(value) => assert_eq!(value, "hello"),
        lsp_types_full::InlayHintLabel::LabelParts(_) => panic!("expected string label"),
    }

    let v: StringOrInlayHintLabelParts =
        unmarshal(r#"[{"value": "param"}, {"value": ": "}, {"value": "string"}]"#).unwrap();
    match v {
        lsp_types_full::InlayHintLabel::String(_) => panic!("expected label parts"),
        lsp_types_full::InlayHintLabel::LabelParts(parts) => {
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0].value, "param");
        }
    }
}

#[test]
fn test_unmarshal_document_edit_union() {
    let v: TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile = unmarshal(
        r#"{
            "textDocument": {"uri": "file:///a.ts", "version": 1},
            "edits": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": "x"}]
        }"#,
    )
    .unwrap();
    assert!(matches!(
        v,
        lsp_types_full::DocumentChangeOperation::Edit(_)
    ));

    let v: TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile =
        unmarshal(r#"{"kind": "create", "uri": "file:///new.ts"}"#).unwrap();
    match v {
        lsp_types_full::DocumentChangeOperation::Op(lsp_types_full::ResourceOp::Create(
            create_file,
        )) => {
            assert_eq!(create_file.uri.as_str(), "file:///new.ts");
        }
        _ => panic!("expected CreateFile"),
    }

    let v: TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile =
        unmarshal(r#"{"kind": "rename", "oldUri": "file:///old.ts", "newUri": "file:///new.ts"}"#)
            .unwrap();
    match v {
        lsp_types_full::DocumentChangeOperation::Op(lsp_types_full::ResourceOp::Rename(
            rename_file,
        )) => {
            assert_eq!(rename_file.old_uri.as_str(), "file:///old.ts");
        }
        _ => panic!("expected RenameFile"),
    }

    let v: TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile =
        unmarshal(r#"{"kind": "delete", "uri": "file:///gone.ts"}"#).unwrap();
    match v {
        lsp_types_full::DocumentChangeOperation::Op(lsp_types_full::ResourceOp::Delete(
            delete_file,
        )) => {
            assert_eq!(delete_file.uri.as_str(), "file:///gone.ts");
        }
        _ => panic!("expected DeleteFile"),
    }
}

#[test]
fn test_unmarshal_field_ordering() {
    let loc: Location = unmarshal(
        r#"{
            "range": {"start": {"line": 1, "character": 2}, "end": {"line": 3, "character": 4}},
            "uri": "file:///test.ts"
        }"#,
    )
    .unwrap();
    assert_eq!(loc.uri, "file:///test.ts");
    assert_eq!(loc.range.start.line, 1);

    let hint: InlayHint = unmarshal(
        r#"{
            "kind": 1,
            "label": "x",
            "position": {"line": 0, "character": 0}
        }"#,
    )
    .unwrap();
    assert_eq!(hint.kind, Some(InlayHintKind::TYPE));
}

#[test]
fn test_unmarshal_empty_object() {
    let v: WorkDoneProgressOptions = unmarshal("{}").unwrap();
    assert!(v.work_done_progress.is_none());

    let _: InitializationOptions = unmarshal("{}").unwrap();
    let _: ClientCapabilities = unmarshal("{}").unwrap();
    let _: ServerCapabilities = unmarshal("{}").unwrap();
}

#[test]
fn test_marshal_omits_zero_optional_fields() {
    let hint = InlayHint {
        position: lsp_position(0, 0),
        label: super::InlayHintLabel {
            string: Some("x".to_string()),
            inlay_hint_label_parts: None,
        },
        kind: None,
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: None,
        data: None,
    };
    let s = marshal(&hint).unwrap();
    assert!(!s.contains("kind"), "should not contain 'kind', got: {s}");
    assert!(
        !s.contains("textEdits"),
        "should not contain 'textEdits', got: {s}"
    );
    assert!(
        !s.contains("paddingLeft"),
        "should not contain 'paddingLeft', got: {s}"
    );
    assert!(
        s.contains("position"),
        "should contain 'position', got: {s}"
    );
    assert!(s.contains("label"), "should contain 'label', got: {s}");

    let fr = FoldingRange {
        start_line: 1,
        end_line: 10,
        ..Default::default()
    };
    let s = marshal(&fr).unwrap();
    assert!(!s.contains("kind"), "should not contain 'kind', got: {s}");
    assert!(
        !s.contains("startCharacter"),
        "should not contain 'startCharacter', got: {s}"
    );
    assert!(
        s.contains("startLine"),
        "should contain 'startLine', got: {s}"
    );
    assert!(s.contains("endLine"), "should contain 'endLine', got: {s}");
}

#[test]
fn test_literal_types() {
    assert_eq!(marshal(&StringLiteralCreate).unwrap(), r#""create""#);
    let _: StringLiteralCreate = unmarshal(r#""create""#).unwrap();
    assert_unmarshal_error::<StringLiteralCreate>(r#""delete""#);
    assert_unmarshal_error::<StringLiteralCreate>("42");
}

#[test]
fn test_enum_string_values() {
    assert_eq!(format!("{:?}", InlayHintKind::TYPE), "Type");
    assert_eq!(format!("{:?}", InlayHintKind::PARAMETER), "Parameter");
    assert_eq!(format!("{:?}", lsp_types_full::SymbolKind::FILE), "File");
    assert_eq!(
        format!("{:?}", lsp_types_full::SymbolKind::FUNCTION),
        "Function"
    );
    assert_eq!(
        format!("{:?}", lsp_types_full::SymbolKind::VARIABLE),
        "Variable"
    );

    let s = format!(
        "{:?}",
        serde_json::from_str::<InlayHintKind>("999").unwrap()
    );
    assert!(
        s.contains("999"),
        "should contain the numeric value, got: {s}"
    );
}

#[test]
fn test_unmarshal_result_accepts_null_pointer_responses() {
    for method in [
        super::MethodWindowShowDocument,
        super::MethodWorkspaceApplyEdit,
        super::MethodInlayHintResolve,
        super::MethodWorkspaceDiagnostic,
        super::MethodWorkspaceTextDocumentContent,
        super::MethodCompletionItemResolve,
        super::MethodCodeActionResolve,
        super::MethodWorkspaceSymbolResolve,
        super::MethodCodeLensResolve,
        super::MethodDocumentLinkResolve,
        super::MethodCustomInitializeAPISession,
        super::MethodCustomProjectInfo,
        super::MethodCustomSaveHeapProfile,
        super::MethodCustomSaveAllocProfile,
        super::MethodCustomStopCPUProfile,
    ] {
        assert_eq!(
            super::unmarshal_result(method.to_string(), serde_json::Value::Null).unwrap(),
            serde_json::Value::Null
        );
    }
}

#[test]
fn test_unmarshal_result_rejects_non_object_pointer_responses() {
    for (method, wrapper) in [
        (super::MethodWindowShowDocument, "ShowDocumentResultOrNull"),
        (
            super::MethodWorkspaceApplyEdit,
            "ApplyWorkspaceEditResultOrNull",
        ),
        (super::MethodInlayHintResolve, "InlayHintOrNull"),
        (
            super::MethodWorkspaceDiagnostic,
            "WorkspaceDiagnosticReportOrNull",
        ),
        (
            super::MethodWorkspaceTextDocumentContent,
            "TextDocumentContentResultOrNull",
        ),
        (super::MethodCompletionItemResolve, "CompletionItemOrNull"),
        (super::MethodCodeActionResolve, "CodeActionOrNull"),
        (super::MethodWorkspaceSymbolResolve, "WorkspaceSymbolOrNull"),
        (super::MethodCodeLensResolve, "CodeLensOrNull"),
        (super::MethodDocumentLinkResolve, "DocumentLinkOrNull"),
        (
            super::MethodCustomInitializeAPISession,
            "InitializeAPISessionResultOrNull",
        ),
        (super::MethodCustomProjectInfo, "ProjectInfoResultOrNull"),
        (super::MethodCustomSaveHeapProfile, "ProfileResultOrNull"),
        (super::MethodCustomSaveAllocProfile, "ProfileResultOrNull"),
        (super::MethodCustomStopCPUProfile, "ProfileResultOrNull"),
    ] {
        let err = super::unmarshal_result(method.to_string(), json!(true)).unwrap_err();
        assert!(err.contains(&format!("invalid {wrapper}")));
    }
}

#[test]
fn test_unmarshal_result_rejects_null_apply_edit_optional_fields() {
    let err = super::unmarshal_result(
        super::MethodWorkspaceApplyEdit.to_string(),
        json!({
            "applied": false,
            "failureReason": null
        }),
    )
    .unwrap_err();
    assert!(err.contains("null value is not allowed for field \"failureReason\""));

    let err = super::unmarshal_result(
        super::MethodWorkspaceApplyEdit.to_string(),
        json!({
            "applied": false,
            "failedChange": null
        }),
    )
    .unwrap_err();
    assert!(err.contains("null value is not allowed for field \"failedChange\""));
}

#[test]
fn test_unmarshal_result_accepts_null_initialize_response() {
    assert_eq!(
        super::unmarshal_result(super::MethodInitialize.to_string(), serde_json::Value::Null)
            .unwrap(),
        serde_json::Value::Null
    );
}

#[test]
fn test_unmarshal_result_rejects_null_initialize_server_info() {
    let err = super::unmarshal_result(
        super::MethodInitialize.to_string(),
        json!({
            "capabilities": {},
            "serverInfo": null
        }),
    )
    .unwrap_err();
    assert!(err.contains("null value is not allowed for field \"serverInfo\""));
}

#[test]
fn test_unmarshal_result_uses_document_diagnostic_discriminator() {
    let value = super::unmarshal_result(
        super::MethodTextDocumentDiagnostic.to_string(),
        json!({
            "kind": "full",
            "items": []
        }),
    )
    .unwrap();
    assert_eq!(value["kind"], "full");

    let value = super::unmarshal_result(
        super::MethodTextDocumentDiagnostic.to_string(),
        json!({
            "kind": "unchanged",
            "resultId": "1"
        }),
    )
    .unwrap();
    assert_eq!(value["kind"], "unchanged");
}

#[test]
fn test_unmarshal_result_rejects_undiscriminated_document_diagnostic_response() {
    let err = super::unmarshal_result(
        super::MethodTextDocumentDiagnostic.to_string(),
        json!({
            "items": []
        }),
    )
    .unwrap_err();
    assert!(
        err.contains(
            "invalid RelatedFullDocumentDiagnosticReportOrUnchangedDocumentDiagnosticReport"
        ),
        "got {err}"
    );
}

#[test]
fn test_unmarshal_result_rejects_null_document_diagnostic_result_id() {
    let err = super::unmarshal_result(
        super::MethodTextDocumentDiagnostic.to_string(),
        json!({
            "kind": "full",
            "resultId": null,
            "items": []
        }),
    )
    .unwrap_err();
    assert!(err.contains("null value is not allowed for field \"resultId\""));
}

#[test]
fn test_unmarshal_result_uses_workspace_diagnostic_discriminator() {
    let value = super::unmarshal_result(
        super::MethodWorkspaceDiagnostic.to_string(),
        json!({
            "items": [
                {
                    "kind": "full",
                    "items": [],
                    "uri": "file:///a.ts",
                    "version": null
                },
                {
                    "kind": "unchanged",
                    "resultId": "1",
                    "uri": "file:///b.ts",
                    "version": 2
                }
            ]
        }),
    )
    .unwrap();
    assert_eq!(value["items"][0]["kind"], "full");
    assert_eq!(value["items"][1]["kind"], "unchanged");
}

#[test]
fn test_unmarshal_result_rejects_null_workspace_diagnostic_items() {
    let err = super::unmarshal_result(
        super::MethodWorkspaceDiagnostic.to_string(),
        json!({
            "items": null
        }),
    )
    .unwrap_err();
    assert!(err.contains("null value is not allowed for field \"items\""));
}

#[test]
fn test_unmarshal_result_rejects_object_call_hierarchy_prepare_response() {
    let err = unmarshal_result(
        MethodTextDocumentPrepareCallHierarchy.to_string(),
        json!({
            "name": "symbol",
            "kind": 12,
            "uri": "file:///a.ts",
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 0, "character": 6}
            },
            "selectionRange": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 0, "character": 6}
            }
        }),
    )
    .unwrap_err();
    assert!(err.contains("invalid CallHierarchyItemsOrNull"));
}

#[test]
fn test_unmarshal_result_rejects_object_call_hierarchy_incoming_calls_response() {
    let err = unmarshal_result(
        MethodCallHierarchyIncomingCalls.to_string(),
        json!({
            "from": {
                "name": "caller",
                "kind": 12,
                "uri": "file:///a.ts",
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 6}
                },
                "selectionRange": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 6}
                }
            },
            "fromRanges": []
        }),
    )
    .unwrap_err();
    assert!(err.contains("invalid CallHierarchyIncomingCallsOrNull"));
}

#[test]
fn test_unmarshal_result_rejects_object_call_hierarchy_outgoing_calls_response() {
    let err = unmarshal_result(
        MethodCallHierarchyOutgoingCalls.to_string(),
        json!({
            "to": {
                "name": "callee",
                "kind": 12,
                "uri": "file:///a.ts",
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 6}
                },
                "selectionRange": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 6}
                }
            },
            "fromRanges": []
        }),
    )
    .unwrap_err();
    assert!(err.contains("invalid CallHierarchyOutgoingCallsOrNull"));
}

#[test]
fn test_unmarshal_result_rejects_object_inline_value_response() {
    let err = unmarshal_result(
        MethodTextDocumentInlineValue.to_string(),
        json!({
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 0, "character": 1}
            },
            "text": "value"
        }),
    )
    .unwrap_err();
    assert!(err.contains("invalid InlineValuesOrNull"));
}

#[test]
fn test_unmarshal_result_rejects_object_type_hierarchy_responses() {
    for method in [
        MethodTextDocumentPrepareTypeHierarchy,
        MethodTypeHierarchySupertypes,
        MethodTypeHierarchySubtypes,
    ] {
        let err = unmarshal_result(
            method.to_string(),
            json!({
                "name": "symbol",
                "kind": 12,
                "uri": "file:///a.ts",
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 6}
                },
                "selectionRange": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 6}
                }
            }),
        )
        .unwrap_err();
        assert!(err.contains("invalid TypeHierarchyItemsOrNull"));
    }
}

#[test]
fn test_unmarshal_result_rejects_object_nullable_array_responses() {
    for (method, wrapper) in [
        (MethodTextDocumentInlayHint, "InlayHintsOrNull"),
        (MethodTextDocumentCodeLens, "CodeLensesOrNull"),
        (MethodTextDocumentDocumentLink, "DocumentLinksOrNull"),
        (
            MethodCustomTextDocumentMultiDocumentHighlight,
            "MultiDocumentHighlightsOrNull",
        ),
    ] {
        let err = unmarshal_result(
            method.to_string(),
            json!({
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 1}
                }
            }),
        )
        .unwrap_err();
        assert!(err.contains(&format!("invalid {wrapper}")));
    }
}

#[test]
fn test_unmarshal_result_rejects_array_vs_on_auto_insert_response() {
    let err =
        unmarshal_result(MethodTextDocumentVSOnAutoInsert.to_string(), json!([])).unwrap_err();
    assert!(err.contains("invalid VsOnAutoInsertResponseItemOrNull"));
}

#[test]
fn test_unmarshal_result_rejects_primitive_inline_completion_response() {
    let err =
        unmarshal_result(MethodTextDocumentInlineCompletion.to_string(), json!(true)).unwrap_err();
    assert!(err.contains("invalid InlineCompletionListOrItemsOrNull"));
}

#[test]
fn test_unmarshal_result_rejects_array_profile_responses() {
    for method in [
        MethodCustomSaveHeapProfile,
        MethodCustomSaveAllocProfile,
        MethodCustomStopCPUProfile,
    ] {
        let err = unmarshal_result(method.to_string(), json!([])).unwrap_err();
        assert!(err.contains("invalid ProfileResultOrNull"));
    }
}

#[test]
fn test_unmarshal_result_rejects_null_color_presentation_optional_fields() {
    let err = super::unmarshal_result(
        super::MethodTextDocumentColorPresentation.to_string(),
        json!([
            {
                "label": "red",
                "textEdit": null
            }
        ]),
    )
    .unwrap_err();
    assert!(err.contains("null value is not allowed for field \"textEdit\""));

    let err = super::unmarshal_result(
        super::MethodTextDocumentColorPresentation.to_string(),
        json!([
            {
                "label": "red",
                "additionalTextEdits": null
            }
        ]),
    )
    .unwrap_err();
    assert!(err.contains("null value is not allowed for field \"additionalTextEdits\""));
}

#[test]
fn test_unmarshal_result_accepts_null_color_response_elements() {
    assert_eq!(
        super::unmarshal_result(
            super::MethodTextDocumentDocumentColor.to_string(),
            json!([null])
        )
        .unwrap(),
        json!([null])
    );
    assert_eq!(
        super::unmarshal_result(
            super::MethodTextDocumentColorPresentation.to_string(),
            json!([null])
        )
        .unwrap(),
        json!([null])
    );
}

#[test]
fn test_unmarshal_result_accepts_null_pointer_array_response_elements() {
    for method in [
        super::MethodWorkspaceWorkspaceFolders,
        super::MethodTextDocumentFoldingRange,
        super::MethodTextDocumentDeclaration,
        super::MethodTextDocumentSelectionRange,
        super::MethodTextDocumentPrepareCallHierarchy,
        super::MethodCallHierarchyIncomingCalls,
        super::MethodCallHierarchyOutgoingCalls,
        super::MethodTextDocumentWillSaveWaitUntil,
        super::MethodTextDocumentMoniker,
        super::MethodTextDocumentPrepareTypeHierarchy,
        super::MethodTypeHierarchySupertypes,
        super::MethodTypeHierarchySubtypes,
        super::MethodTextDocumentInlayHint,
        super::MethodTextDocumentCompletion,
        super::MethodTextDocumentDefinition,
        super::MethodTextDocumentDocumentHighlight,
        super::MethodTextDocumentDocumentSymbol,
        super::MethodWorkspaceSymbol,
        super::MethodTextDocumentCodeLens,
        super::MethodTextDocumentDocumentLink,
        super::MethodTextDocumentFormatting,
        super::MethodTextDocumentRangeFormatting,
        super::MethodTextDocumentRangesFormatting,
        super::MethodTextDocumentOnTypeFormatting,
        super::MethodCustomTextDocumentSourceDefinition,
        super::MethodCustomTextDocumentMultiDocumentHighlight,
    ] {
        assert_eq!(
            super::unmarshal_result(method.to_string(), json!([null])).unwrap(),
            json!([null])
        );
    }
}

#[test]
fn test_unmarshal_result_rejects_missing_document_color_fields() {
    let err = super::unmarshal_result(
        super::MethodTextDocumentDocumentColor.to_string(),
        json!([
            {
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 1}
                }
            }
        ]),
    )
    .unwrap_err();
    assert!(err.contains("missing required properties: color"));
}
