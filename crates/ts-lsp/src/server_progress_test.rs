use std::sync::{Arc, Mutex, mpsc};

use ts_bundled as bundled;
use ts_testutil::lsptestutil;
use ts_vfs::vfstest;

use crate::{ServerOptions, lsproto};

#[test]
fn test_progress_notifications_end_to_end() {
    if !bundled::embedded() {
        eprintln!("bundled files are not embedded");
        return;
    }

    let fs = bundled::wrap_fs(vfstest::from_map(
        [
            ("/home/projects/tsconfig.json", "{}"),
            ("/home/projects/index.ts", "export const x = 1;"),
        ],
        false,
    ));

    // Collect $/progress notifications. Signal when "end" arrives.
    let progress_notifications = Arc::new(Mutex::new(Vec::<lsproto::ProgressParams>::new()));
    let (end_sender, end_received) = mpsc::sync_channel::<()>(1);

    let on_server_request =
        Box::new(
            |req: &lsptestutil::RequestMessage| match req.method.as_str() {
                "client/registerCapability"
                | "client/unregisterCapability"
                | "window/workDoneProgress/create" => Some(lsptestutil::ResponseMessage {
                    id: req.id.clone(),
                    jsonrpc: req.jsonrpc.clone(),
                    result: serde_json::Value::Null,
                    error: None,
                }),
                _ => None,
            },
        );

    let (mut client, close_client) = lsptestutil::new_lsp_client(
        ServerOptions {
            err: Some(Box::new(std::io::sink())),
            cwd: "/home/projects".to_string(),
            fs: Some(Arc::new(fs)),
            default_library_path: bundled::lib_path(),
            ..Default::default()
        },
        Some(on_server_request),
    );

    let notifications_for_handler = progress_notifications.clone();
    client.set_on_server_notification(Some(Box::new(move |req: &lsptestutil::RequestMessage| {
        if req.method == "$/progress" {
            let params = serde_json::from_value::<lsproto::ProgressParams>(req.params.clone())
                .expect("expected ProgressParams");
            let is_end = params.value.end.is_some();
            notifications_for_handler
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .push(params);
            if is_end {
                let _ = end_sender.try_send(());
            }
        }
    })));

    let (init_msg, _, ok) = lsptestutil::send_request(
        &mut client,
        &*lsproto::InitializeInfo,
        &lsproto::InitializeParams {
            capabilities: lsproto::ClientCapabilities {
                window: Some(lsproto::WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(
        ok && init_msg.as_response().error.is_none(),
        "Initialize failed"
    );
    lsptestutil::send_notification(
        &mut client,
        &*lsproto::InitializedInfo,
        &lsproto::InitializedParams {},
    );
    client.server.init_complete().recv().unwrap();

    let uri = "file:///home/projects/index.ts".to_string();
    lsptestutil::send_notification(
        &mut client,
        &*lsproto::TextDocumentDidOpenInfo,
        &lsproto::DidOpenTextDocumentParams {
            text_document: lsproto::TextDocumentItem {
                uri: uri.clone(),
                language_id: "typescript".to_string(),
                text: "export const x = 1;".to_string(),
                ..Default::default()
            },
        },
    );

    // Send a request to ensure the server has processed the didOpen and loaded the project.
    let (msg, resp, ok) = lsptestutil::send_request(
        &mut client,
        &*lsproto::CustomProjectInfoInfo,
        &lsproto::ProjectInfoParams {
            text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
        },
    );
    assert!(ok, "expected a response");
    assert!(msg.as_response().error.is_none());
    assert_eq!(
        resp.project_info_result.unwrap().config_file_path,
        "/home/projects/tsconfig.json"
    );

    // Wait for the "end" progress notification before reading.
    end_received
        .recv()
        .expect("timed out waiting for progress end notification");

    let notifications = progress_notifications
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone();

    assert!(
        notifications.len() >= 2,
        "expected at least begin+end progress notifications, got {}",
        notifications.len()
    );

    // First notification should be a "begin".
    assert!(
        notifications[0].value.begin.is_some(),
        "expected first progress notification to be 'begin'"
    );
    assert_eq!(
        notifications[0].value.begin.as_ref().unwrap().title,
        "Loading"
    );

    // Last notification should be an "end".
    let last = notifications.last().unwrap();
    assert!(
        last.value.end.is_some(),
        "expected last progress notification to be 'end'"
    );

    // All notifications should share the same token.
    let first_token = token_string(&notifications[0].token);
    assert!(!first_token.is_empty(), "expected non-empty progress token");
    for (i, notification) in notifications.iter().enumerate() {
        assert_eq!(
            token_string(&notification.token),
            first_token,
            "notification {i} has different token"
        );
    }

    close_client().expect("close client");
}

fn token_string(token: &lsproto::IntegerOrString) -> String {
    if let Some(string) = &token.string {
        return string.clone();
    }
    String::new()
}
