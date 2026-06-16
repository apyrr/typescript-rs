use std::{collections::HashMap, io, sync::Arc};

use ts_bundled as bundled;
use ts_core as core;
use ts_ls as lsconv;
use ts_ls as lsutil;
use ts_testutil::lsptestutil;
use ts_vfs::vfstest;

use crate::{ServerOptions, lsproto};

fn init_completion_client(
    files: HashMap<String, String>,
    prefs: lsutil::UserPreferences,
) -> lsptestutil::LSPClient {
    let fs = bundled::wrap_fs(vfstest::from_map(files, false));

    let prefs_for_request = prefs.clone();
    let on_server_request =
        move |req: &lsproto::RequestMessage| -> Option<lsproto::ResponseMessage> {
            match req.method.as_str() {
                lsproto::MethodWorkspaceConfiguration => Some(lsproto::ResponseMessage {
                    id: req.id.clone(),
                    jsonrpc: req.jsonrpc.clone(),
                    result: serde_json::to_value(vec![prefs_for_request.clone()])
                        .unwrap_or_default(),
                    ..Default::default()
                }),
                lsproto::MethodClientRegisterCapability
                | lsproto::MethodClientUnregisterCapability => Some(lsproto::ResponseMessage {
                    id: req.id.clone(),
                    jsonrpc: req.jsonrpc.clone(),
                    result: serde_json::to_value(lsproto::Null {}).unwrap_or_default(),
                    ..Default::default()
                }),
                _ => None,
            }
        };

    let (mut client, _close_client) = lsptestutil::new_lsp_client(
        ServerOptions {
            err: Some(Box::new(io::sink())),
            cwd: "/home/projects".to_string(),
            fs: Some(Arc::new(fs)),
            default_library_path: bundled::lib_path(),
            ..Default::default()
        },
        Some(Box::new(on_server_request)),
    );

    let (init_msg, _, ok) = lsptestutil::send_request(
        &mut client,
        &*lsproto::InitializeInfo,
        lsproto::InitializeParams {
            capabilities: lsproto::ClientCapabilities::default(),
            ..Default::default()
        },
    );
    assert!(
        ok && init_msg.as_response().error.is_none(),
        "Initialize failed"
    );
    lsptestutil::send_notification(
        &client,
        &*lsproto::InitializedInfo,
        lsproto::InitializedParams {},
    );
    client.server.init_complete().recv().unwrap();

    lsptestutil::send_notification(
        &client,
        &*lsproto::WorkspaceDidChangeConfigurationInfo,
        lsproto::DidChangeConfigurationParams {
            settings: serde_json::json!({ "typescript": prefs }),
        },
    );

    client
}

fn completion_items(resp: lsproto::CompletionResponse) -> Vec<lsproto::CompletionItem> {
    if let Some(list) = resp.list {
        return list.items;
    }
    if let Some(items) = resp.items {
        return items.into_iter().flatten().collect();
    }
    Vec::new()
}

fn find_completion_item<'a>(
    items: &'a [lsproto::CompletionItem],
    label: &str,
) -> Option<&'a lsproto::CompletionItem> {
    items.iter().find(|item| item.label == label)
}

// Verifies that completion succeeds on a file that was already closed
// by the time the server processes the completion request.
#[test]
fn test_completion_after_file_close() {
    if !bundled::embedded() {
        eprintln!("bundled files are not embedded");
        return;
    }

    let prefs = lsutil::UserPreferences {
        include_completions_for_module_exports: core::TSTrue,
        include_completions_for_import_statements: core::TSTrue,
        ..Default::default()
    };
    let mut client = init_completion_client(
        HashMap::from([
            (
                "/home/projects/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"module": "esnext", "target": "esnext"}}"#.to_string(),
            ),
            (
                "/home/projects/a.ts".to_string(),
                "export const someVar = 10;".to_string(),
            ),
            ("/home/projects/b.ts".to_string(), "s".to_string()),
        ]),
        prefs,
    );

    let a_uri = lsconv::file_name_to_document_uri("/home/projects/a.ts");
    let b_uri = lsconv::file_name_to_document_uri("/home/projects/b.ts");
    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidOpenInfo,
        lsproto::DidOpenTextDocumentParams {
            text_document: lsproto::TextDocumentItem {
                uri: a_uri,
                language_id: "typescript".to_string(),
                text: "export const someVar = 10;".to_string(),
                ..Default::default()
            },
        },
    );
    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidOpenInfo,
        lsproto::DidOpenTextDocumentParams {
            text_document: lsproto::TextDocumentItem {
                uri: b_uri.clone(),
                language_id: "typescript".to_string(),
                text: "s".to_string(),
                ..Default::default()
            },
        },
    );

    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidCloseInfo,
        lsproto::DidCloseTextDocumentParams {
            text_document: lsproto::TextDocumentIdentifier { uri: b_uri.clone() },
        },
    );

    let (msg, resp, ok) = lsptestutil::send_request(
        &mut client,
        &*lsproto::TextDocumentCompletionInfo,
        lsproto::CompletionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: b_uri },
            position: lsproto::Position {
                line: 0,
                character: 1,
            },
            context: Some(lsproto::CompletionContext::default()),
            ..Default::default()
        },
    );
    assert!(ok, "expected a response");
    assert!(msg.as_response().error.is_none());
    let items = completion_items(resp);
    let item = find_completion_item(&items, "someVar").expect("expected someVar completion");
    assert!(item.data.as_ref().unwrap().auto_import.is_some());
    assert_eq!(
        item.data
            .as_ref()
            .unwrap()
            .auto_import
            .as_ref()
            .unwrap()
            .module_specifier,
        "./a"
    );
}

// Completion request is enqueued first, then a close notification is sent.
// This guarantees the completion enters the input channel before the close.
#[test]
fn test_completion_with_concurrent_file_close() {
    if !bundled::embedded() {
        eprintln!("bundled files are not embedded");
        return;
    }

    let prefs = lsutil::UserPreferences {
        include_completions_for_module_exports: core::TSTrue,
        include_completions_for_import_statements: core::TSTrue,
        ..Default::default()
    };
    let mut client = init_completion_client(
        HashMap::from([
            (
                "/home/projects/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"module": "esnext", "target": "esnext"}}"#.to_string(),
            ),
            (
                "/home/projects/a.ts".to_string(),
                "export const someVar = 10;".to_string(),
            ),
            ("/home/projects/b.ts".to_string(), "s".to_string()),
        ]),
        prefs,
    );

    let a_uri = lsconv::file_name_to_document_uri("/home/projects/a.ts");
    let b_uri = lsconv::file_name_to_document_uri("/home/projects/b.ts");
    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidOpenInfo,
        lsproto::DidOpenTextDocumentParams {
            text_document: lsproto::TextDocumentItem {
                uri: a_uri,
                language_id: "typescript".to_string(),
                text: "export const someVar = 10;".to_string(),
                ..Default::default()
            },
        },
    );
    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidOpenInfo,
        lsproto::DidOpenTextDocumentParams {
            text_document: lsproto::TextDocumentItem {
                uri: b_uri.clone(),
                language_id: "typescript".to_string(),
                text: "s".to_string(),
                ..Default::default()
            },
        },
    );

    let wait_for_completion = lsptestutil::send_request_async(
        &mut client,
        &*lsproto::TextDocumentCompletionInfo,
        lsproto::CompletionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: b_uri.clone() },
            position: lsproto::Position {
                line: 0,
                character: 1,
            },
            context: Some(lsproto::CompletionContext::default()),
            ..Default::default()
        },
    );

    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidCloseInfo,
        lsproto::DidCloseTextDocumentParams {
            text_document: lsproto::TextDocumentIdentifier { uri: b_uri },
        },
    );

    let (msg, resp, ok) = wait_for_completion(&mut client);
    assert!(ok, "expected a response");
    assert!(msg.as_response().error.is_none());
    let items = completion_items(resp);
    let item = find_completion_item(&items, "someVar").expect("expected someVar completion");
    assert!(item.data.as_ref().unwrap().auto_import.is_some());
    assert_eq!(
        item.data
            .as_ref()
            .unwrap()
            .auto_import
            .as_ref()
            .unwrap()
            .module_specifier,
        "./a"
    );
}

#[test]
fn test_completion_for_unopened_file() {
    if !bundled::embedded() {
        eprintln!("bundled files are not embedded");
        return;
    }

    let prefs = lsutil::UserPreferences::default();
    let mut client = init_completion_client(
        HashMap::from([
            (
                "/home/projects/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"module": "esnext", "target": "esnext"}}"#.to_string(),
            ),
            (
                "/home/projects/c.ts".to_string(),
                "let xyz = 1;\nxy".to_string(),
            ),
        ]),
        prefs,
    );

    let c_uri = lsconv::file_name_to_document_uri("/home/projects/c.ts");
    let (msg, resp, ok) = lsptestutil::send_request(
        &mut client,
        &*lsproto::TextDocumentCompletionInfo,
        lsproto::CompletionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: c_uri },
            position: lsproto::Position {
                line: 1,
                character: 2,
            },
            context: Some(lsproto::CompletionContext::default()),
            ..Default::default()
        },
    );
    assert!(ok, "expected a response");
    assert!(msg.as_response().error.is_none());
    assert!(find_completion_item(&completion_items(resp), "xyz").is_some());
}

#[test]
fn test_auto_import_completion_for_unopened_file() {
    if !bundled::embedded() {
        eprintln!("bundled files are not embedded");
        return;
    }

    let prefs = lsutil::UserPreferences {
        include_completions_for_module_exports: core::TSTrue,
        include_completions_for_import_statements: core::TSTrue,
        ..Default::default()
    };
    let mut client = init_completion_client(
        HashMap::from([
            (
                "/home/projects/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"module": "esnext", "target": "esnext"}}"#.to_string(),
            ),
            (
                "/home/projects/a.ts".to_string(),
                "export const someVar = 10;".to_string(),
            ),
            ("/home/projects/c.ts".to_string(), "s".to_string()),
        ]),
        prefs,
    );

    let c_uri = lsconv::file_name_to_document_uri("/home/projects/c.ts");
    let (msg, resp, ok) = lsptestutil::send_request(
        &mut client,
        &*lsproto::TextDocumentCompletionInfo,
        lsproto::CompletionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: c_uri },
            position: lsproto::Position {
                line: 0,
                character: 1,
            },
            context: Some(lsproto::CompletionContext::default()),
            ..Default::default()
        },
    );
    assert!(ok, "expected a response");
    assert!(msg.as_response().error.is_none());
    let items = completion_items(resp);
    let item = find_completion_item(&items, "someVar").expect("expected someVar completion");
    assert!(item.data.as_ref().unwrap().auto_import.is_some());
    assert_eq!(
        item.data
            .as_ref()
            .unwrap()
            .auto_import
            .as_ref()
            .unwrap()
            .module_specifier,
        "./a"
    );
}

// TestCompletionSnapshotFreezing verifies that the auto-import retry uses the
// snapshot captured in the sync phase, not a newer one that includes a
// concurrent DidChange. Without snapshot freezing the retry would flush the
// pending change, making position/prefix inconsistent with the request.
#[test]
fn test_completion_snapshot_freezing() {
    if !bundled::embedded() {
        eprintln!("bundled files are not embedded");
        return;
    }

    let prefs = lsutil::UserPreferences {
        include_completions_for_module_exports: core::TSTrue,
        include_completions_for_import_statements: core::TSTrue,
        ..Default::default()
    };
    let mut client = init_completion_client(
        HashMap::from([
            (
                "/home/projects/tsconfig.json".to_string(),
                r#"{"compilerOptions": {"module": "esnext", "target": "esnext"}}"#.to_string(),
            ),
            (
                "/home/projects/a.ts".to_string(),
                "export const someVar = 10;".to_string(),
            ),
            ("/home/projects/b.ts".to_string(), "someV".to_string()),
        ]),
        prefs,
    );

    let a_uri = lsconv::file_name_to_document_uri("/home/projects/a.ts");
    let b_uri = lsconv::file_name_to_document_uri("/home/projects/b.ts");
    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidOpenInfo,
        lsproto::DidOpenTextDocumentParams {
            text_document: lsproto::TextDocumentItem {
                uri: a_uri,
                language_id: "typescript".to_string(),
                text: "export const someVar = 10;".to_string(),
                ..Default::default()
            },
        },
    );
    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidOpenInfo,
        lsproto::DidOpenTextDocumentParams {
            text_document: lsproto::TextDocumentItem {
                uri: b_uri.clone(),
                language_id: "typescript".to_string(),
                text: "someV".to_string(),
                ..Default::default()
            },
        },
    );

    let wait_for_completion = lsptestutil::send_request_async(
        &mut client,
        &*lsproto::TextDocumentCompletionInfo,
        lsproto::CompletionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: b_uri.clone() },
            position: lsproto::Position {
                line: 0,
                character: 5,
            },
            context: Some(lsproto::CompletionContext::default()),
            ..Default::default()
        },
    );

    lsptestutil::send_notification(
        &client,
        &*lsproto::TextDocumentDidChangeInfo,
        lsproto::DidChangeTextDocumentParams {
            text_document: lsproto::VersionedTextDocumentIdentifier {
                uri: b_uri,
                version: 2,
            },
            content_changes: vec![lsproto::TextDocumentContentChangePartialOrWholeDocument {
                whole_document: Some(lsproto::TextDocumentContentChangeWholeDocument {
                    text: "notMatching".to_string(),
                }),
                ..Default::default()
            }],
        },
    );

    let (msg, resp, ok) = wait_for_completion(&mut client);
    assert!(ok, "expected a response");
    assert!(msg.as_response().error.is_none());
    let items = completion_items(resp);
    let item = find_completion_item(&items, "someVar").expect(
        "expected someVar in completions (snapshot freezing should preserve original content)",
    );
    assert!(item.data.as_ref().unwrap().auto_import.is_some());
    assert_eq!(
        item.data
            .as_ref()
            .unwrap()
            .auto_import
            .as_ref()
            .unwrap()
            .module_specifier,
        "./a"
    );
}
