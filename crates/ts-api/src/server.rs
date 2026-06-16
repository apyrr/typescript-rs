use std::io;
use std::panic;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use ts_bundled as bundled;
use ts_core as context;
use ts_lsproto as lsproto;
use ts_project as project;
use ts_vfs::{self as vfs, osvfs};

use crate::{
    CallbackClient, CallbackFs, Conn, Error, ReadClose, SessionOptions, Transport, WriteClose,
    new_async_conn, new_async_conn_with_protocol, new_callback_fs, new_jsonrpc_protocol,
    new_message_pack_protocol, new_session, new_sync_conn,
};

// StdioServerOptions configures the STDIO-based API server.
#[derive(Default)]
pub struct StdioServerOptions {
    pub r#in: Option<Box<dyn ReadClose>>,
    pub out: Option<Box<dyn WriteClose>>,
    pub err: Option<Box<dyn io::Write>>,
    pub cwd: String,
    pub default_library_path: String,
    // PipePath, if set, listens on a named pipe (Windows) or Unix domain
    // socket instead of using In/Out for communication.
    pub pipe_path: String,
    // Callbacks specifies which filesystem operations should be delegated
    // to the client (e.g., "readFile", "fileExists"). Empty means no callbacks.
    pub callbacks: Vec<String>,
    // Async enables JSON-RPC protocol with async connection handling.
    // When false (default), uses MessagePack protocol with sync connection.
    pub r#async: bool,
}

// StdioServer runs an API session over STDIO using MessagePack protocol.
// This is the entry point for the synchronous STDIO-based API used by
// native TypeScript tooling integration.
pub struct StdioServer {
    options: StdioServerOptions,
}

// NewStdioServer creates a new STDIO-based API server.
pub fn new_stdio_server(options: StdioServerOptions) -> StdioServer {
    if options.cwd.is_empty() {
        panic!("StdioServerOptions.Cwd is required");
    }

    StdioServer { options }
}

pub fn spawn_pipe_session(
    ctx: context::Context,
    mut transport: crate::PipeTransport,
    api_session: Arc<crate::Session>,
    log_error: Arc<dyn Fn(String)>,
    on_done: Box<dyn FnOnce(String)>,
) {
    let session_id = api_session.id().to_string();
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let rwc = match transport.accept() {
            Ok(rwc) => rwc,
            Err(err) => {
                let _ = transport.close();
                log_error(format!(
                    "API session {session_id}: failed to accept connection: {err}"
                ));
                return;
            }
        };
        let _ = transport.close();
        let conn = new_async_conn(rwc, api_session.clone());
        if let Err(err) = conn.run(ctx) {
            log_error(format!("API session {session_id}: {err}"));
        }
    }));
    if let Err(err) = result {
        log_error(format!("API session {session_id}: panic: {err:?}"));
    }
    api_session.close();
    on_done(session_id);
}

#[derive(Clone)]
struct SharedCallbackFs {
    // PORT NOTE: reshaped for borrowck; Go stores one *callbackFS both in the
    // vfs.FS interface and in callbackFS so SetConnection can run after Accept.
    fs: Arc<Mutex<CallbackFs>>,
}

impl SharedCallbackFs {
    fn new(fs: CallbackFs) -> Self {
        Self {
            fs: Arc::new(Mutex::new(fs)),
        }
    }

    fn set_connection(&self, ctx: context::Context, conn: Box<dyn CallbackClient + Send + Sync>) {
        self.fs.lock().unwrap().set_connection(ctx, conn);
    }
}

impl vfs::Fs for SharedCallbackFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.lock().unwrap().use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.fs.lock().unwrap().file_exists(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.fs.lock().unwrap().read_file(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.lock().unwrap().write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.lock().unwrap().append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.fs.lock().unwrap().remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.fs.lock().unwrap().chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.fs.lock().unwrap().directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        self.fs.lock().unwrap().get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        self.fs.lock().unwrap().stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        self.fs.lock().unwrap().walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        self.fs.lock().unwrap().realpath(path)
    }
}

impl StdioServer {
    // Run starts the server and blocks until the connection closes.
    pub fn run(&mut self, ctx: context::Context) -> Result<(), Error> {
        let mut transport: Box<dyn crate::Transport> = if !self.options.pipe_path.is_empty() {
            let t = crate::new_pipe_transport(&self.options.pipe_path)
                .map_err(|err| Error::new(format!("failed to create pipe transport: {err}")))?;
            Box::new(t)
        } else {
            let t = crate::new_stdio_transport(
                self.options.r#in.take().expect("stdin is required"),
                self.options.out.take().expect("stdout is required"),
            );
            Box::new(t)
        };

        let mut fs: Arc<dyn vfs::Fs + Send + Sync> = Arc::new(bundled::wrap_fs(osvfs::os::fs()));

        // Wrap the base FS with callbackFS if callbacks are requested
        let mut callback_fs: Option<SharedCallbackFs> = None;
        if !self.options.callbacks.is_empty() {
            let shared_callback_fs =
                SharedCallbackFs::new(new_callback_fs(fs.clone(), &self.options.callbacks));
            fs = Arc::new(shared_callback_fs.clone());
            callback_fs = Some(shared_callback_fs);
        }

        let project_session = project::new_session(project::SessionInit {
            background_ctx: ctx.clone(),
            logger: project::new_log_tree(String::new()),
            fs,
            options: project::SessionOptions {
                current_directory: self.options.cwd.clone(),
                default_library_path: self.options.default_library_path.clone(),
                typings_location: String::new(),
                position_encoding: lsproto::PositionEncodingKind::UTF8,
                watch_enabled: false,
                logging_enabled: false,
                telemetry_enabled: false,
                push_diagnostics_enabled: false,
                debounce_delay: Duration::default(),
                locale: project::Locale::default(),
            },
            client: None,
            npm_executor: None,
            parse_cache: None,
        });

        let session = Arc::new(new_session(
            project_session,
            SessionOptions {
                use_binary_responses: !self.options.r#async, // Only msgpack uses binary responses
                ..Default::default()
            },
        ));

        // Accept connection from transport
        let rwc = match transport.accept() {
            Ok(rwc) => rwc,
            Err(err) => {
                session.close();
                let _ = transport.close();
                return Err(Error::new(format!("failed to accept connection: {err}")));
            }
        };

        // Create protocol and connection based on async mode
        let (conn, callback_client): (Arc<dyn Conn>, Box<dyn CallbackClient + Send + Sync>) =
            if self.options.r#async {
                let protocol = Arc::new(new_jsonrpc_protocol(rwc.clone_reader_writer()));
                let conn = Arc::new(new_async_conn_with_protocol(rwc, protocol, session.clone()));
                let callback_client = conn.callback_client();
                (conn, Box::new(callback_client))
            } else {
                let protocol = Arc::new(new_message_pack_protocol(rwc.clone_reader_writer()));
                let conn = Arc::new(new_sync_conn(rwc, protocol, session.clone()));
                let callback_client = conn.callback_client();
                (conn, Box::new(callback_client))
            };

        // If callbacks are enabled, set the connection on the FS
        if let Some(callback_fs) = callback_fs.as_ref() {
            callback_fs.set_connection(ctx.clone(), callback_client);
        }

        let result = conn.run(&ctx);
        session.close();
        let _ = transport.close();
        result
    }
}
