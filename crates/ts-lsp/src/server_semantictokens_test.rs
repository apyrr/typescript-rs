use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use lsp_types_full as lsp_types;
use ts_bundled as bundled;
use ts_core::context;
use ts_jsonrpc as jsonrpc;
use ts_vfs::vfstest;

use crate::{Reader, ServerOptions, Writer, lsproto, new_server};

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

// TestSemanticTokensCRLF reproduces a crash where semantic tokens panics with
// "token spans multiple lines" when the editor opens a file with CRLF line endings
// but the project originally loaded the file from disk with LF line endings.
//
// The SourceFile AST keeps positions from the LF text, but the converter's
// line map is recomputed from the CRLF overlay, causing a mismatch.
#[test]
fn test_semantic_tokens_crlf() {
    if !bundled::EMBEDDED {
        return;
    }

    // Enough lines so the cumulative \r\n vs \n offset difference
    // causes an LF-based position to land on a \r in the CRLF text.
    let file_on_disk = "var x\nvar x\nvar x\nvar x\nvar x\nvar x\nconst a = 1\n";
    let file_from_editor = file_on_disk.replace('\n', "\r\n");

    let files = HashMap::from([
        ("/home/projects/tsconfig.json", "{}"),
        ("/home/projects/test.ts", file_on_disk),
        ("/home/projects/other.ts", "export {}"),
    ]);
    let fs = bundled::wrap_fs(vfstest::from_map(files.clone(), false));

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

    let init_params = lsproto::InitializeParams {
        capabilities: lsproto::ClientCapabilities {
            text_document: Some(lsp_types::TextDocumentClientCapabilities {
                semantic_tokens: Some(lsp_types::SemanticTokensClientCapabilities {
                    token_types: [
                        "namespace",
                        "type",
                        "class",
                        "enum",
                        "interface",
                        "struct",
                        "typeParameter",
                        "parameter",
                        "variable",
                        "property",
                        "enumMember",
                        "event",
                        "function",
                        "method",
                        "macro",
                        "keyword",
                        "modifier",
                        "comment",
                        "string",
                        "number",
                        "regexp",
                        "operator",
                        "decorator",
                    ]
                    .into_iter()
                    .map(lsproto::SemanticTokenType::new)
                    .collect(),
                    token_modifiers: [
                        "declaration",
                        "definition",
                        "readonly",
                        "static",
                        "deprecated",
                        "abstract",
                        "async",
                        "modification",
                        "documentation",
                        "defaultLibrary",
                        "local",
                    ]
                    .into_iter()
                    .map(lsproto::SemanticTokenModifier::new)
                    .collect(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

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
        .handle_initialized(ctx.clone(), &lsproto::InitializedParams::default())
        .expect("Initialized failed");
    assert!(server.init_complete());

    // Open another project file to force the project to load test.ts from disk (LF).
    let other_uri = "file:///home/projects/other.ts".to_string();
    server
        .handle_did_open(
            ctx.clone(),
            &lsproto::DidOpenTextDocumentParams {
                text_document: lsproto::TextDocumentItem {
                    uri: other_uri.clone(),
                    language_id: "typescript".to_string(),
                    text: files["/home/projects/other.ts"].to_string(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let language_service = server
        .session
        .as_ref()
        .unwrap()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .get_language_service(ctx.clone(), other_uri.clone())
        .unwrap();
    let msg1 = server.handle_semantic_tokens_full(
        ctx.clone(),
        &language_service,
        &lsproto::SemanticTokensParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: other_uri.parse().unwrap(),
            },
            ..Default::default()
        },
    );
    assert!(msg1.is_ok(), "Initial request failed");

    // Open test.ts with CRLF content; the project already parsed it from disk (LF).
    let uri = "file:///home/projects/test.ts".to_string();
    server
        .handle_did_open(
            ctx.clone(),
            &lsproto::DidOpenTextDocumentParams {
                text_document: lsproto::TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "typescript".to_string(),
                    text: file_from_editor,
                    ..Default::default()
                },
            },
        )
        .unwrap();

    // This panics: AST positions are LF-based but the line map is CRLF-based.
    let language_service = server
        .session
        .as_ref()
        .unwrap()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .get_language_service(ctx.clone(), uri.clone())
        .unwrap();
    let msg = server.handle_semantic_tokens_full(
        ctx,
        &language_service,
        &lsproto::SemanticTokensParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            ..Default::default()
        },
    );
    if let Err(err) = msg {
        panic!("Semantic tokens request failed: {err}");
    }
}
