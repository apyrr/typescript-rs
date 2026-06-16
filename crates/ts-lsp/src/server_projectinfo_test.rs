use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::json;
use ts_bundled as bundled;
use ts_core::context;
use ts_jsonrpc as jsonrpc;
use ts_vfs::vfstest;

use crate::{Reader, Server, ServerOptions, Writer, lsproto, new_server};

struct NullResponseReader {
    next_id: Mutex<i32>,
}

impl Reader for NullResponseReader {
    fn read(&self) -> Result<lsproto::Message, io::Error> {
        let mut next_id = self.next_id.lock().unwrap();
        *next_id += 1;
        Ok(lsproto::ResponseMessage {
            jsonrpc: jsonrpc::JsonRpcVersion::default(),
            id: Some(jsonrpc::Id::new_string(format!("ts{}", *next_id))),
            result: serde_json::Value::Null,
            error: None,
        }
        .message())
    }
}

#[derive(Default)]
struct DiscardWriter {
    messages: Mutex<Vec<lsproto::Message>>,
}

impl Writer for Arc<DiscardWriter> {
    fn write(&self, msg: &lsproto::Message) -> Result<(), io::Error> {
        self.messages.lock().unwrap().push(msg.clone());
        Ok(())
    }
}

fn init_project_info_client(files: HashMap<&'static str, &'static str>) -> Server {
    let fs = bundled::wrap_fs(vfstest::from_map(files, false));
    let out = Arc::new(DiscardWriter::default());
    let mut server = new_server(ServerOptions {
        r#in: Some(Box::new(NullResponseReader {
            next_id: Mutex::new(0),
        })),
        out: Some(Box::new(out)),
        err: Some(Box::new(io::sink())),
        cwd: "/home/projects".to_string(),
        fs: Some(Arc::new(fs)),
        default_library_path: bundled::lib_path(),
        typings_location: String::new(),
        parse_cache: None,
        npm_install: None,
        progress_delay: Duration::default(),
        set_parent_process_id: None,
    });

    let init_params: lsproto::InitializeParams = serde_json::from_value(json!({
        "processId": null,
        "rootUri": null,
        "capabilities": {}
    }))
    .unwrap();
    let ctx = context::background();
    let init_req = lsproto::RequestMessage {
        jsonrpc: jsonrpc::JsonRpcVersion::default(),
        id: Some(jsonrpc::Id::new_int(0)),
        method: "initialize".to_string(),
        params: serde_json::to_value(&init_params).unwrap(),
    };
    server
        .handle_initialize(ctx.clone(), &init_params, &init_req)
        .expect("Initialize failed");
    server
        .handle_initialized(ctx, &lsproto::InitializedParams::default())
        .expect("Initialized failed");
    assert!(server.init_complete(), "Initialize failed");

    server
}

#[test]
fn test_project_info_configured_project() {
    if !bundled::EMBEDDED {
        return;
    }

    let mut client = init_project_info_client(HashMap::from([
        ("/home/projects/tsconfig.json", "{}"),
        ("/home/projects/index.ts", "export const x = 1;"),
    ]));

    let ctx = context::background();
    let uri = "file:///home/projects/index.ts".to_string();
    client
        .handle_did_open(
            ctx.clone(),
            &lsproto::DidOpenTextDocumentParams {
                text_document: lsproto::TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "typescript".to_string(),
                    text: "export const x = 1;".to_string(),
                    ..Default::default()
                },
            },
        )
        .unwrap();

    let req = lsproto::RequestMessage {
        jsonrpc: jsonrpc::JsonRpcVersion::default(),
        id: Some(jsonrpc::Id::new_int(1)),
        method: "custom/projectInfo".to_string(),
        params: serde_json::Value::Null,
    };
    let resp = client
        .handle_project_info(
            ctx,
            &lsproto::ProjectInfoParams {
                text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
            },
            &req,
        )
        .expect("expected a response");
    assert_eq!(
        resp.project_info_result.unwrap().config_file_path,
        "/home/projects/tsconfig.json"
    );
}

#[test]
fn test_project_info_inferred_project() {
    if !bundled::EMBEDDED {
        return;
    }

    let mut client = init_project_info_client(HashMap::from([(
        "/home/projects/index.ts",
        "export const x = 1;",
    )]));

    let ctx = context::background();
    let uri = "file:///home/projects/index.ts".to_string();
    client
        .handle_did_open(
            ctx.clone(),
            &lsproto::DidOpenTextDocumentParams {
                text_document: lsproto::TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "typescript".to_string(),
                    text: "export const x = 1;".to_string(),
                    ..Default::default()
                },
            },
        )
        .unwrap();

    let req = lsproto::RequestMessage {
        jsonrpc: jsonrpc::JsonRpcVersion::default(),
        id: Some(jsonrpc::Id::new_int(1)),
        method: "custom/projectInfo".to_string(),
        params: serde_json::Value::Null,
    };
    let resp = client
        .handle_project_info(
            ctx,
            &lsproto::ProjectInfoParams {
                text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
            },
            &req,
        )
        .expect("expected a response");
    assert_eq!(resp.project_info_result.unwrap().config_file_path, "");
}
