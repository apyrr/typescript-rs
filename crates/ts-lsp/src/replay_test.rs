use std::{
    fs::File,
    io::{BufRead, BufReader},
    process::Command,
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use ts_bundled as bundled;
use ts_core as core;
use ts_json as json;
use ts_jsonrpc as jsonrpc;
use ts_ls as lsconv;
use ts_testutil::lsptestutil;
use ts_vfs::osvfs::os as osvfs;

use crate::{ServerOptions, lsproto};

const REPLAY_FLAG: &str = "replay";
const TEST_DIR_FLAG: &str = "testDir";
const SIMPLE_FLAG: &str = "simple";
const SUPER_SIMPLE_FLAG: &str = "superSimple";

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitialArguments {
    root_dir_uri_placeholder: String,
    root_dir_placeholder: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawMessage {
    kind: String,
    method: String,
    params: json::Value,
}

#[test]
fn test_replay() {
    let Some(replay) = test_flag_value(REPLAY_FLAG) else {
        eprintln!("no replay file specified");
        return;
    };
    if replay.is_empty() {
        eprintln!("no replay file specified");
        return;
    }
    let test_dir = test_flag_value(TEST_DIR_FLAG).expect("testDir must be specified");
    if test_dir.is_empty() {
        panic!("testDir must be specified");
    }
    let test_dir_uri = lsconv::file_name_to_document_uri(&test_dir);

    let fs = bundled::wrap_fs(osvfs::fs());
    let default_library_path = bundled::lib_path();
    let typings_location = osvfs::get_global_typings_cache_location();
    let server_opts = ServerOptions {
        r#in: None,
        out: None,
        err: Some(Box::new(std::io::stderr())),
        cwd: core::must(std::env::current_dir().map(|path| path.to_string_lossy().into_owned())),
        fs: Some(Arc::new(fs)),
        default_library_path,
        typings_location,
        parse_cache: None,
        npm_install: Some(|cwd, args| {
            Command::new("npm")
                .args(args)
                .current_dir(cwd)
                .output()
                .map(|output| output.stdout)
        }),
        progress_delay: Duration::default(),
        set_parent_process_id: None,
    };

    let (mut client, close_client) = lsptestutil::new_lsp_client(server_opts, None);
    let file =
        File::open(&replay).unwrap_or_else(|err| panic!("failed to read replay file: {err}"));
    let mut scanner = BufReader::with_capacity(64 * 1024, file).lines();

    let first_line = scanner
        .next()
        .unwrap_or_else(|| panic!("replay file is empty"))
        .unwrap_or_else(|err| panic!("error scanning replay file: {err}"));

    let mut root_dir_placeholder = "@PROJECT_ROOT@".to_string();
    let mut root_dir_uri_placeholder = "@PROJECT_ROOT_URI@".to_string();
    let mut init_obj = InitialArguments::default();
    json::unmarshal(first_line.as_bytes(), &mut init_obj, &[])
        .unwrap_or_else(|err| panic!("failed to parse initial arguments: {err}"));

    if !init_obj.root_dir_placeholder.is_empty() {
        root_dir_placeholder = init_obj.root_dir_placeholder;
    }
    if !init_obj.root_dir_uri_placeholder.is_empty() {
        root_dir_uri_placeholder = init_obj.root_dir_uri_placeholder;
    }

    let mut messages = Vec::<RawMessage>::new();
    for line in scanner {
        let line = line.unwrap_or_else(|err| panic!("error scanning replay file: {err}"));
        let line = line
            .replace(&root_dir_placeholder, &test_dir)
            .replace(&root_dir_uri_placeholder, &test_dir_uri);
        let mut raw_msg = RawMessage::default();
        json::unmarshal(line.as_bytes(), &mut raw_msg, &[])
            .unwrap_or_else(|err| panic!("failed to parse message: {err}"));
        messages.push(raw_msg);
    }

    if test_flag_bool(SIMPLE_FLAG) {
        // Include only initialization, file opening/changing/closing, and shutdown messages, plus the final request.
        let mut new_messages = Vec::<RawMessage>::new();
        let mut i = 0;
        while i < messages.len() && is_initialization_message(&messages[i]) {
            new_messages.push(messages[i].clone());
            i += 1;
        }
        let mut j = messages.len() as isize - 1;
        while j >= 0 && is_exit_message(&messages[j as usize]) {
            j -= 1;
        }
        if j >= i as isize {
            for k in i..=(j as usize) {
                let msg = &messages[k];
                if msg.method == "textDocument/didOpen"
                    || msg.method == "textDocument/didChange"
                    || msg.method == "textDocument/didClose"
                {
                    new_messages.push(msg.clone());
                }
            }
        }
        let start = std::cmp::max(i as isize, j) as usize;
        for msg in messages.iter().skip(start) {
            new_messages.push(msg.clone());
        }
        messages = new_messages;
    } else if test_flag_bool(SUPER_SIMPLE_FLAG) {
        // Include only initialization, shutdown, the last file open and the final request.
        // We assume here the final request will be for the file that was opened last.
        let mut new_messages = Vec::<RawMessage>::new();
        let mut i = 0;
        while i < messages.len() && is_initialization_message(&messages[i]) {
            new_messages.push(messages[i].clone());
            i += 1;
        }

        let mut j = messages.len() as isize - 1;
        while j >= 0 && is_exit_message(&messages[j as usize]) {
            j -= 1;
        }
        let mut open_idx = j;
        while open_idx >= i as isize {
            let msg = &messages[open_idx as usize];
            if msg.method == "textDocument/didOpen" {
                new_messages.push(msg.clone());
                break;
            }
            open_idx -= 1;
        }
        let start = std::cmp::max(open_idx + 1, j).max(0) as usize;
        for msg in messages.iter().skip(start) {
            new_messages.push(msg.clone());
        }
        messages = new_messages;
    }

    for raw_msg in messages {
        let (kind, req_id) = match raw_msg.kind.as_str() {
            "request" => {
                let id = lsproto::new_id(lsproto::IntegerOrString::from(client.next_id()));
                (jsonrpc::MessageKind::Request, Some(id))
            }
            "notification" => (jsonrpc::MessageKind::Notification, None),
            _ => panic!("unknown message kind: {}", raw_msg.kind),
        };

        #[derive(Serialize)]
        struct RpcMessage<'a> {
            jsonrpc: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            id: Option<&'a jsonrpc::Id>,
            method: &'a str,
            params: &'a json::Value,
        }

        let rpc_msg = RpcMessage {
            jsonrpc: "2.0",
            id: req_id.as_ref(),
            method: &raw_msg.method,
            params: &raw_msg.params,
        };
        let rpc_data = json::marshal(&rpc_msg, &[])
            .unwrap_or_else(|err| panic!("failed to marshal rpc message: {err}"));

        let mut msg = None::<lsproto::Message>;
        json::unmarshal(&rpc_data, &mut msg, &[]).unwrap_or_else(|err| {
            panic!("failed to unmarshal rpc message into lsproto.Message: {err}")
        });
        let msg = msg.expect("lsproto.Message should deserialize");

        match kind {
            jsonrpc::MessageKind::Request => {
                let req_id = req_id.as_ref().unwrap();
                let (_msg, response, ok) =
                    client.send_request_worker(msg.as_request(), req_id.clone());
                if !ok {
                    panic!("failed to send request for method {}", raw_msg.method)
                }
                if let Some(error) = response.error {
                    panic!(
                        "server returned error for method {} params {}:\n{}",
                        raw_msg.method, raw_msg.params, error
                    );
                }
            }
            jsonrpc::MessageKind::Notification => {
                let _ = client.write_msg(msg);
            }
            _ => panic!("unknown message kind: {}", raw_msg.kind),
        }
    }

    if let Err(err) = close_client() {
        panic!("goroutine error: {err}");
    }
}

fn test_flag_value(name: &str) -> Option<String> {
    let long = format!("--{name}");
    let long_eq = format!("--{name}=");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == long {
            return args.next();
        }
        if let Some(value) = arg.strip_prefix(&long_eq) {
            return Some(value.to_string());
        }
    }
    None
}

fn test_flag_bool(name: &str) -> bool {
    let long = format!("--{name}");
    let long_eq = format!("--{name}=");
    for arg in std::env::args().skip(1) {
        if arg == long {
            return true;
        }
        if let Some(value) = arg.strip_prefix(&long_eq) {
            return value != "false" && value != "0";
        }
    }
    false
}

fn is_initialization_message(msg: &RawMessage) -> bool {
    msg.method == "initialize" || msg.method == "initialized"
}

fn is_exit_message(msg: &RawMessage) -> bool {
    msg.method == "exit" || msg.method == "shutdown"
}
