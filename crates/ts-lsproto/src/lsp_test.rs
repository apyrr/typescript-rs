use lsp_types_full::CompletionItemKind;
use serde_json::json;
use std::collections::HashMap;

use super::{
    CompletionItem, CompletionTextEdit, InsertReplaceEdit, InsertTextFormat, MethodTelemetryEvent,
    PerformanceStatsTelemetryEvent, PerformanceStatsTelemetryMeasurements, Position,
    ProjectInfoTelemetryEvent, ProjectInfoTelemetryMeasurements, Range,
    RequestFailureTelemetryEvent, RequestFailureTelemetryProperties, TelemetryEvent,
    TelemetryEventInfo,
};

#[test]
fn test_unmarshal_completion_item() {
    let message = r#"{
    "label": "pageXOffset",
    "insertTextFormat": 1,
    "textEdit": {
        "newText": "pageXOffset",
        "insert": {
            "start": {
                "line": 4,
                "character": 0
            },
            "end": {
                "line": 4,
                "character": 4
            }
        },
        "replace": {
            "start": {
                "line": 4,
                "character": 0
            },
            "end": {
                "line": 4,
                "character": 4
            }
        }
    },
    "kind": 6,
    "sortText": "15",
    "commitCharacters": [
        ".",
        ",",
        ";"
    ]
}"#;

    let result: CompletionItem = super::unmarshal_value(message.as_bytes()).unwrap();

    assert_eq!(
        result,
        CompletionItem {
            label: "pageXOffset".to_string(),
            insert_text_format: Some(InsertTextFormat::PlainText),
            text_edit: Some(CompletionTextEdit {
                text_edit: None,
                insert_replace_edit: Some(InsertReplaceEdit {
                    new_text: "pageXOffset".to_string(),
                    insert: Range {
                        start: Position {
                            line: 4,
                            character: 0,
                        },
                        end: Position {
                            line: 4,
                            character: 4,
                        },
                    },
                    replace: Range {
                        start: Position {
                            line: 4,
                            character: 0,
                        },
                        end: Position {
                            line: 4,
                            character: 4,
                        },
                    },
                }),
            }),
            kind: Some(CompletionItemKind::VARIABLE),
            sort_text: Some("15".to_string()),
            commit_characters: Some(vec![".".to_string(), ",".to_string(), ";".to_string()]),
            ..Default::default()
        }
    );
}

#[test]
fn test_marshal_request_failure_telemetry_event() {
    let event = TelemetryEvent {
        request_failure_telemetry_event: Some(RequestFailureTelemetryEvent {
            properties: Some(RequestFailureTelemetryProperties {
                error_code: "-32603".to_string(),
                request_method: "textDocument/completion".to_string(),
                stack: "stack trace".to_string(),
            }),
        }),
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_value(event).unwrap(),
        json!({
            "eventName": "languageServer.errorResponse",
            "telemetryPurpose": "error",
            "properties": {
                "errorCode": "-32603",
                "requestMethod": "textDocument/completion",
                "stack": "stack trace"
            }
        })
    );
}

#[test]
fn test_marshal_performance_stats_telemetry_omits_zero_measurements() {
    let event = TelemetryEvent {
        performance_stats_telemetry_event: Some(PerformanceStatsTelemetryEvent {
            measurements: Some(PerformanceStatsTelemetryMeasurements {
                open_file_count: 2.0,
                memory_used_bytes: 4096.0,
                ..Default::default()
            }),
        }),
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_value(event).unwrap(),
        json!({
            "eventName": "languageServer.performanceStats",
            "telemetryPurpose": "usage",
            "measurements": {
                "openFileCount": 2.0,
                "memoryUsedBytes": 4096.0
            }
        })
    );
}

#[test]
fn test_marshal_project_info_telemetry_event() {
    let mut properties = HashMap::new();
    properties.insert("projectType".to_string(), "configured".to_string());
    properties.insert(
        "compilerOptions".to_string(),
        r#"{"strict":true}"#.to_string(),
    );

    let event = TelemetryEvent {
        project_info_telemetry_event: Some(ProjectInfoTelemetryEvent {
            properties,
            measurements: Some(ProjectInfoTelemetryMeasurements {
                ts_file_count: 3.0,
                ts_file_size: 1200.0,
                ..Default::default()
            }),
        }),
        ..Default::default()
    };

    assert_eq!(
        serde_json::to_value(event).unwrap(),
        json!({
            "eventName": "languageServer.projectInfo",
            "telemetryPurpose": "usage",
            "properties": {
                "projectType": "configured",
                "compilerOptions": "{\"strict\":true}"
            },
            "measurements": {
                "tsFileCount": 3.0,
                "tsFileSize": 1200.0
            }
        })
    );
}

#[test]
fn test_marshal_telemetry_event_null_when_no_arm_is_set() {
    assert_eq!(
        serde_json::to_value(TelemetryEvent::default()).unwrap(),
        serde_json::Value::Null
    );
}

#[test]
fn test_marshal_telemetry_event_rejects_multiple_arms() {
    let err = serde_json::to_value(TelemetryEvent {
        request_failure_telemetry_event: Some(RequestFailureTelemetryEvent::default()),
        performance_stats_telemetry_event: Some(PerformanceStatsTelemetryEvent::default()),
        ..Default::default()
    })
    .unwrap_err()
    .to_string();

    assert!(err.contains("more than one element of TelemetryEvent is set"));
}

#[test]
fn test_unmarshal_telemetry_event_uses_event_name_discriminator() {
    let event: TelemetryEvent = serde_json::from_value(json!({
        "eventName": "languageServer.performanceStats",
        "telemetryPurpose": "usage",
        "measurements": {
            "openFileCount": 4.0,
            "memoryUsedBytes": 8192.0
        }
    }))
    .unwrap();

    assert_eq!(
        event.performance_stats_telemetry_event,
        Some(PerformanceStatsTelemetryEvent {
            measurements: Some(PerformanceStatsTelemetryMeasurements {
                open_file_count: 4.0,
                memory_used_bytes: 8192.0,
                ..Default::default()
            }),
        })
    );
    assert!(event.request_failure_telemetry_event.is_none());
    assert!(event.project_info_telemetry_event.is_none());
}

#[test]
fn test_unmarshal_telemetry_event_rejects_wrong_literal() {
    let err = serde_json::from_value::<TelemetryEvent>(json!({
        "eventName": "languageServer.projectInfo",
        "telemetryPurpose": "error",
        "properties": {},
        "measurements": {}
    }))
    .unwrap_err()
    .to_string();

    assert!(err.contains("expected ProjectInfoTelemetryEvent telemetryPurpose usage"));
}

#[test]
fn test_unmarshal_telemetry_event_accepts_null_union() {
    assert_eq!(
        serde_json::from_value::<TelemetryEvent>(serde_json::Value::Null).unwrap(),
        TelemetryEvent::default()
    );
}

#[test]
fn test_telemetry_event_info_uses_telemetry_method() {
    assert_eq!(MethodTelemetryEvent, "telemetry/event");
    assert_eq!(TelemetryEventInfo.method, MethodTelemetryEvent);
}
