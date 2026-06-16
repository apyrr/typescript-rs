use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

use ts_bundled as bundled;
use ts_core::context;
use ts_locale as locale;
use ts_project as project;
use ts_vfs::vfstest;

use crate::{Reader, ServerOptions, Writer, lsproto, new_server};

struct ShutdownTestReader;

impl Reader for ShutdownTestReader {
    fn read(&self) -> Result<lsproto::Message, io::Error> {
        Err(io::Error::from(io::ErrorKind::UnexpectedEof))
    }
}

struct ShutdownTestWriter;

impl Writer for ShutdownTestWriter {
    fn write(&self, _msg: &lsproto::Message) -> Result<(), io::Error> {
        Ok(())
    }
}

// TestServerShutdownNoDeadlock verifies that operations after shutdown
// don't block.
#[test]
fn test_server_shutdown_no_deadlock() {
    if !bundled::EMBEDDED {
        return;
    }

    let fs = Arc::new(bundled::wrap_fs(vfstest::from_map(
        HashMap::from([
            ("/test/tsconfig.json", "{}"),
            ("/test/index.ts", "const x = 1;"),
        ]),
        false,
    )));

    let mut server = new_server(ServerOptions {
        r#in: Some(Box::new(ShutdownTestReader)),
        out: Some(Box::new(ShutdownTestWriter)),
        err: Some(Box::new(io::sink())),
        cwd: "/test".to_string(),
        fs: Some(fs.clone()),
        default_library_path: bundled::lib_path(),
        typings_location: String::new(),
        parse_cache: None,
        npm_install: None,
        progress_delay: Duration::default(),
        set_parent_process_id: None,
    });

    let ctx = context::background();
    server.background_ctx = Some(ctx.clone());

    // Start write loop to drain queue.
    let _ = server.write_loop(ctx.clone());

    // Create session with the server's lifecycle context.
    server.init_started.store(true, Ordering::SeqCst);
    server.session = Some(Arc::new(Mutex::new(project::new_session(
        project::SessionInit {
            background_ctx: ctx.clone(),
            options: project::SessionOptions {
                current_directory: "/test".to_string(),
                default_library_path: bundled::lib_path(),
                typings_location: String::new(),
                position_encoding: lsproto::PositionEncodingKind::UTF8,
                watch_enabled: false,
                logging_enabled: true,
                telemetry_enabled: false,
                push_diagnostics_enabled: true,
                debounce_delay: Duration::from_millis(500),
                locale: locale::Locale::default(),
            },
            fs: fs.clone(),
            client: None,
            logger: Arc::new(server.logger.take().unwrap()),
            npm_executor: None,
            parse_cache: None,
        },
    ))));

    // Open a file to establish a project.
    let mut session = server
        .session
        .as_ref()
        .unwrap()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    session.did_open_file(
        ctx.clone(),
        "file:///test/index.ts".to_string(),
        1,
        "const x = 1;".to_string(),
        "typescript".to_string(),
    );
    session.wait_for_background_tasks();
    drop(session);

    // Shutdown (drain the write loop before post-shutdown operations).
    let _ = server.write_loop(ctx.clone());

    // Fill the queue so any logging attempt would block.
    let dummy_msg = lsproto::WindowLogMessageInfo
        .new_notification_message(lsproto::LogMessageParams {
            r#type: lsproto::MessageType::Info,
            message: "fill".to_string(),
        })
        .message();
    let outgoing_queue_capacity = server
        .outgoing_queue
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .capacity();
    for _ in 0..outgoing_queue_capacity.max(1) {
        server
            .outgoing_queue
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(dummy_msg.clone());
    }

    // Trigger operations that would log (these should not block).
    let mut session = server
        .session
        .as_ref()
        .unwrap()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    session.did_change_file(
        ctx.clone(),
        "file:///test/index.ts".to_string(),
        2,
        vec![lsproto::TextDocumentContentChangePartialOrWholeDocument {
            whole_document: Some(lsproto::TextDocumentContentChangeWholeDocument {
                text: "const x = 2;".to_string(),
            }),
            ..Default::default()
        }],
    );
    let _ = session.get_language_service(ctx, "file:///test/index.ts".to_string());
    session.wait_for_background_tasks();

    session.close();
}
