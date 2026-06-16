use std::{
    collections::{HashMap, VecDeque},
    io::{self, Read, Write},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use serde::{Serialize, de::DeserializeOwned};
use ts_api as api;
use ts_collections as collections;
use ts_core as core;
use ts_core::context;
use ts_diagnostics as diagnostics;
use ts_json as json;
use ts_jsonrpc as jsonrpc;
use ts_locale as locale;
use ts_ls as ls;
#[cfg(feature = "pprof")]
use ts_pprof as pprof;
use ts_project as project;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::lsproto::{ClientCapabilitiesExt, DocumentUriExt};
use crate::{
    logger::new_logger_with_parts, lsproto, progress::new_project_loading_progress,
    sanitize_stack_trace,
};

pub struct ServerOptions {
    pub r#in: Option<Box<dyn Reader + Send>>,
    pub out: Option<Box<dyn Writer + Send>>,
    pub err: Option<Box<dyn io::Write + Send + Sync>>,
    pub cwd: String,
    pub fs: Option<vfs::FS>,
    pub default_library_path: String,
    pub typings_location: String,
    pub parse_cache: Option<project::ParseCache>,
    pub npm_install: Option<fn(cwd: String, args: Vec<String>) -> Result<Vec<u8>, io::Error>>,
    pub progress_delay: Duration,
    pub set_parent_process_id: Option<fn(parent_pid: i32)>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            r#in: None,
            out: None,
            err: None,
            cwd: String::new(),
            fs: None,
            default_library_path: String::new(),
            typings_location: String::new(),
            parse_cache: None,
            npm_install: None,
            progress_delay: Duration::default(),
            set_parent_process_id: None,
        }
    }
}

pub fn new_server(opts: ServerOptions) -> Server {
    if opts.cwd.is_empty() {
        panic!("Cwd is required");
    }

    let stderr = Arc::new(Mutex::new(opts.err.expect("ServerOptions.Err is required")));
    let init_started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let init_complete_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = Arc::new(Mutex::new(opts.r#in.expect("ServerOptions.In is required")));
    let outgoing_queue = Arc::new(Mutex::new(Vec::new()));
    let request_queue = Arc::new(Mutex::new(VecDeque::with_capacity(100)));
    let pending_server_requests = Arc::new(Mutex::new(HashMap::new()));
    let client_seq = Arc::new(std::sync::atomic::AtomicI32::new(0));
    let watchers = collections::SyncSet::default();
    let last_request_time_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let npm_install = opts.npm_install;
    let callback_state = Arc::new(Mutex::new(ServerCallbackState {
        reader: reader.clone(),
        client_seq: client_seq.clone(),
        request_queue: request_queue.clone(),
        outgoing_queue: outgoing_queue.clone(),
        pending_server_requests: pending_server_requests.clone(),
        watchers: watchers.clone(),
        last_request_time_ms: last_request_time_ms.clone(),
        client_capabilities: None,
        telemetry_enabled: false,
        project_progress: None,
        npm_install,
    }));

    let mut server = Server {
        r: reader,
        w: Arc::new(Mutex::new(opts.out.expect("ServerOptions.Out is required"))),
        background_ctx: None,
        stderr: stderr.clone(),
        logger: None,
        init_started: init_started.clone(),
        client_seq,
        request_queue,
        outgoing_queue: outgoing_queue.clone(),
        callback_state,
        pending_client_requests: HashMap::new(),
        pending_server_requests,
        cwd: opts.cwd,
        fs: opts.fs,
        default_library_path: opts.default_library_path,
        typings_location: opts.typings_location,
        initialize_params: None,
        client_capabilities: None,
        position_encoding: None,
        locale: None,
        watch_enabled: false,
        telemetry_enabled: false,
        watcher_id: std::sync::atomic::AtomicU32::new(0),
        watchers,
        last_request_time_ms,
        session: None,
        api_sessions: Arc::new(Mutex::new(HashMap::new())),
        client: None,
        init_complete: false,
        init_complete_signal,
        compiler_options_for_inferred_projects: None,
        parse_cache: opts.parse_cache,
        npm_install,
        #[cfg(feature = "pprof")]
        cpu_profiler: pprof::CpuProfiler::default(),
        progress_delay: opts.progress_delay,
        project_progress: None,
        start_watchdog: opts.set_parent_process_id,
    };
    server.logger = Some(new_logger_with_parts(
        stderr,
        Some(init_started),
        Some(outgoing_queue),
    ));
    server
}

pub fn file_rename_filters() -> Vec<lsproto::FileOperationFilter> {
    vec![lsproto::FileOperationFilter {
        scheme: Some("file".to_string()),
        pattern: lsproto::FileOperationPattern {
            glob: "**/*.{ts,tsx,js,jsx,cts,cjs,mts,mjs,json}".to_string(),
            ..Default::default()
        },
        ..Default::default()
    }]
}

pub struct PendingClientRequest {
    pub req: Box<lsproto::RequestMessage>,
    pub cancel: Option<Box<dyn Fn() + Send + Sync>>,
}

struct ServerCallbackState {
    reader: Arc<Mutex<Box<dyn Reader + Send>>>,
    client_seq: Arc<std::sync::atomic::AtomicI32>,
    request_queue: Arc<Mutex<VecDeque<lsproto::RequestMessage>>>,
    outgoing_queue: Arc<Mutex<Vec<lsproto::Message>>>,
    pending_server_requests: Arc<Mutex<HashMap<jsonrpc::Id, Vec<lsproto::ResponseMessage>>>>,
    watchers: collections::SyncSet<project::WatcherID>,
    last_request_time_ms: Arc<std::sync::atomic::AtomicI64>,
    client_capabilities: Option<lsproto::ResolvedClientCapabilities>,
    telemetry_enabled: bool,
    project_progress: Option<crate::ProjectLoadingProgress>,
    npm_install: Option<fn(cwd: String, args: Vec<String>) -> Result<Vec<u8>, io::Error>>,
}

impl ServerCallbackState {
    fn send(&self, msg: &lsproto::Message) {
        self.outgoing_queue
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(msg.clone());
    }

    fn read(&self) -> Result<lsproto::Message, io::Error> {
        self.reader
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .read()
    }

    fn send_client_request<Req, Resp>(
        &self,
        info: lsproto::RequestInfo<Req, Resp>,
        params: Req,
    ) -> Result<Resp, Box<dyn std::error::Error + Send + Sync>>
    where
        Req: Serialize,
        Resp: DeserializeOwned,
    {
        let seq = self
            .client_seq
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let id = jsonrpc::Id::new_string(format!("ts{seq}"));
        let req = info.new_request_message(Some(id.clone()), params);
        self.pending_server_requests
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(id.clone(), Vec::new());
        self.send(&req.message());

        loop {
            {
                let mut pending = self
                    .pending_server_requests
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                if let Some(responses) = pending.get_mut(&id) {
                    if let Some(resp) = responses.pop() {
                        pending.remove(&id);
                        if let Some(error) = resp.error {
                            return Err(Box::new(SimpleError(format!(
                                "request failed: {}",
                                error.message
                            ))));
                        }
                        return info.unmarshal_result(resp.result).map_err(|err| {
                            Box::new(SimpleError(err)) as Box<dyn std::error::Error + Send + Sync>
                        });
                    }
                } else {
                    return Err(Box::new(SimpleError("request cancelled".to_string())));
                }
            }

            match self.read() {
                Ok(msg) if msg.kind == jsonrpc::MessageKind::Response => {
                    let resp = msg.as_response().clone();
                    if resp.id.as_ref() == Some(&id) {
                        let mut pending = self
                            .pending_server_requests
                            .lock()
                            .unwrap_or_else(|err| err.into_inner());
                        pending.remove(&id);
                        if let Some(error) = resp.error {
                            return Err(Box::new(SimpleError(format!(
                                "request failed: {}",
                                error.message
                            ))));
                        }
                        return info.unmarshal_result(resp.result).map_err(|err| {
                            Box::new(SimpleError(err)) as Box<dyn std::error::Error + Send + Sync>
                        });
                    }
                }
                Ok(msg) => {
                    if msg.kind != jsonrpc::MessageKind::Response {
                        self.request_queue
                            .lock()
                            .unwrap_or_else(|err| err.into_inner())
                            .push_back(msg.as_request().clone());
                    }
                }
                Err(err) => {
                    self.pending_server_requests
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .remove(&id);
                    return Err(Box::new(err));
                }
            }
        }
    }

    fn send_client_request_fire_and_forget<Req, Resp>(
        &self,
        info: lsproto::RequestInfo<Req, Resp>,
        params: Req,
    ) where
        Req: Serialize,
    {
        let seq = self
            .client_seq
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let id = jsonrpc::Id::new_string(format!("ts{seq}"));
        self.send(&info.new_request_message(Some(id), params).message());
    }

    fn send_notification<Params>(&self, info: lsproto::NotificationInfo<Params>, params: Params)
    where
        Params: Serialize,
    {
        self.send(&info.new_notification_message(params).message());
    }

    fn watch_files(
        &self,
        id: project::WatcherID,
        watchers: Vec<lsproto::FileSystemWatcher>,
    ) -> Result<(), io::Error> {
        self.send_client_request(
            lsproto::ClientRegisterCapabilityInfo.clone(),
            lsproto::RegistrationParams {
                registrations: vec![lsproto::Registration {
                    id: id.to_string(),
                    method: lsproto::MethodWorkspaceDidChangeWatchedFiles.to_string(),
                    register_options: Some(lsproto::RegisterOptions {
                        workspace_did_change_watched_files: Some(
                            lsproto::DidChangeWatchedFilesRegistrationOptions { watchers },
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
            },
        )
        .map(|_: lsproto::Null| ())
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to register file watcher: {err}"),
            )
        })?;
        self.watchers.add(id);
        Ok(())
    }

    fn unwatch_files(&self, id: project::WatcherID) -> Result<(), io::Error> {
        if self.watchers.has(&id) {
            self.send_client_request(
                lsproto::ClientUnregisterCapabilityInfo.clone(),
                lsproto::UnregistrationParams {
                    unregisterations: vec![lsproto::Unregistration {
                        id: id.to_string(),
                        method: lsproto::MethodWorkspaceDidChangeWatchedFiles.to_string(),
                    }],
                },
            )
            .map(|_: lsproto::Null| ())
            .map_err(|err| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("failed to unregister file watcher: {err}"),
                )
            })?;
            self.watchers.delete(&id);
            return Ok(());
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no file watcher exists with ID {id}"),
        ))
    }

    fn refresh_diagnostics(&self) -> Result<(), io::Error> {
        if !self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.workspace.diagnostics.refresh_support)
            .unwrap_or_default()
        {
            return Ok(());
        }
        self.send_client_request_fire_and_forget(
            lsproto::WorkspaceDiagnosticRefreshInfo.clone(),
            lsproto::NoParams {},
        );
        Ok(())
    }

    fn publish_diagnostics(&self, params: lsproto::PublishDiagnosticsParams) {
        self.send_notification(lsproto::TextDocumentPublishDiagnosticsInfo.clone(), params);
    }

    fn send_telemetry(&self, telemetry: lsproto::TelemetryEvent) {
        if !self.telemetry_enabled {
            panic!("SendTelemetry called with telemetry disabled");
        }
        self.send_notification(lsproto::TelemetryEventInfo.clone(), telemetry);
    }

    fn refresh_inlay_hints(&self) -> Result<(), io::Error> {
        if !self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.workspace.inlay_hint.refresh_support)
            .unwrap_or_default()
        {
            return Ok(());
        }
        self.send_client_request(
            lsproto::WorkspaceInlayHintRefreshInfo.clone(),
            lsproto::NoParams {},
        )
        .map(|_: lsproto::Null| ())
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to refresh inlay hints: {err}"),
            )
        })
    }

    fn refresh_code_lens(&self) -> Result<(), io::Error> {
        if !self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.workspace.code_lens.refresh_support)
            .unwrap_or_default()
        {
            return Ok(());
        }
        self.send_client_request(
            lsproto::WorkspaceCodeLensRefreshInfo.clone(),
            lsproto::NoParams {},
        )
        .map(|_: lsproto::Null| ())
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to refresh code lens: {err}"),
            )
        })
    }

    fn progress_start(&self, message: &diagnostics::Message, args: Vec<String>) {
        if let Some(project_progress) = &self.project_progress {
            let args = args
                .into_iter()
                .map(|arg| Box::new(arg) as crate::ProgressArg)
                .collect();
            project_progress.start_blocking(message, args);
        }
    }

    fn progress_finish(&self, message: &diagnostics::Message, args: Vec<String>) {
        if let Some(project_progress) = &self.project_progress {
            let args = args
                .into_iter()
                .map(|arg| Box::new(arg) as crate::ProgressArg)
                .collect();
            project_progress.finish_blocking(message, args);
        }
    }

    fn is_active(&self) -> bool {
        let last = self
            .last_request_time_ms
            .load(std::sync::atomic::Ordering::SeqCst);
        last == 0
            || std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH + Duration::from_millis(last as u64))
                .unwrap_or_default()
                <= Duration::from_secs(60)
    }

    fn npm_install(&self, cwd: String, args: Vec<String>) -> Result<Vec<u8>, io::Error> {
        (self.npm_install.expect("npmInstall callback is required"))(cwd, args)
    }
}

pub trait Reader {
    fn read(&self) -> Result<lsproto::Message, io::Error>;
}

pub trait Writer {
    fn write(&self, msg: &lsproto::Message) -> Result<(), io::Error>;
}

pub struct LspReader<R> {
    pub r: lsproto::BaseReader<R>,
}

pub struct LspWriter<W: Write> {
    pub w: lsproto::BaseWriter<W>,
}

impl<R> Reader for LspReader<R>
where
    R: Read,
{
    fn read(&self) -> Result<lsproto::Message, io::Error> {
        let data = self.r.read()?;
        let mut req = None::<lsproto::Message>;
        if let Err(err) = json::unmarshal(&data, &mut req, &[]) {
            if err.to_string().contains("InvalidParams") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{:?}: {err}", lsproto::ErrorCodeInvalidParams),
                ));
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{:?}: {err}", lsproto::ErrorCodeInvalidRequest),
            ));
        }
        Ok(req.expect("lsproto::Message should deserialize"))
    }
}

pub fn to_reader<R>(r: R) -> LspReader<R>
where
    R: Read,
{
    LspReader {
        r: lsproto::new_base_reader(r),
    }
}

impl<W> Writer for LspWriter<W>
where
    W: Write,
{
    fn write(&self, msg: &lsproto::Message) -> Result<(), io::Error> {
        let data = json::marshal(msg, &[]).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to marshal message: {err}"),
            )
        })?;
        self.w.write(&data)
    }
}

pub fn to_writer<W>(w: W) -> LspWriter<W>
where
    W: Write,
{
    LspWriter {
        w: lsproto::new_base_writer(w),
    }
}

pub struct Server {
    pub r: Arc<Mutex<Box<dyn Reader + Send>>>,
    pub w: Arc<Mutex<Box<dyn Writer + Send>>>,
    pub background_ctx: Option<context::Context>,
    pub stderr: Arc<Mutex<Box<dyn io::Write + Send + Sync>>>,
    pub logger: Option<crate::Logger>,
    pub init_started: Arc<std::sync::atomic::AtomicBool>,
    pub client_seq: Arc<std::sync::atomic::AtomicI32>,
    pub request_queue: Arc<Mutex<VecDeque<lsproto::RequestMessage>>>,
    pub outgoing_queue: Arc<Mutex<Vec<lsproto::Message>>>,
    callback_state: Arc<Mutex<ServerCallbackState>>,
    pub pending_client_requests: HashMap<jsonrpc::Id, PendingClientRequest>,
    pub pending_server_requests: Arc<Mutex<HashMap<jsonrpc::Id, Vec<lsproto::ResponseMessage>>>>,
    pub cwd: String,
    pub fs: Option<vfs::FS>,
    pub default_library_path: String,
    pub typings_location: String,
    pub initialize_params: Option<lsproto::InitializeParams>,
    pub client_capabilities: Option<lsproto::ResolvedClientCapabilities>,
    pub position_encoding: Option<lsproto::PositionEncodingKind>,
    pub locale: Option<locale::Locale>,
    pub watch_enabled: bool,
    pub telemetry_enabled: bool,
    pub watcher_id: std::sync::atomic::AtomicU32,
    pub watchers: collections::SyncSet<project::WatcherID>,
    pub last_request_time_ms: Arc<std::sync::atomic::AtomicI64>,
    pub(crate) session: Option<Arc<Mutex<project::Session>>>,
    pub api_sessions: Arc<Mutex<HashMap<String, Arc<api::Session>>>>,
    pub client: Option<Arc<dyn project::Client>>,
    pub init_complete: bool,
    pub init_complete_signal: Arc<std::sync::atomic::AtomicBool>,
    pub compiler_options_for_inferred_projects: Option<core::CompilerOptions>,
    pub parse_cache: Option<project::ParseCache>,
    pub npm_install: Option<fn(cwd: String, args: Vec<String>) -> Result<Vec<u8>, io::Error>>,
    #[cfg(feature = "pprof")]
    pub cpu_profiler: pprof::CpuProfiler,
    pub progress_delay: Duration,
    pub project_progress: Option<crate::ProjectLoadingProgress>,
    pub start_watchdog: Option<fn(parent_pid: i32)>,
}

impl Server {
    pub fn session(&self) -> Option<MutexGuard<'_, project::Session>> {
        self.session
            .as_ref()
            .map(|session| session.lock().unwrap_or_else(|err| err.into_inner()))
    }

    pub fn init_complete(&self) -> bool {
        self.init_complete
    }

    pub fn watch_files(
        &mut self,
        ctx: context::Context,
        id: project::WatcherID,
        watchers: Vec<lsproto::FileSystemWatcher>,
    ) -> Result<(), io::Error> {
        send_client_request(
            ctx,
            self,
            lsproto::ClientRegisterCapabilityInfo.clone(),
            lsproto::RegistrationParams {
                registrations: vec![lsproto::Registration {
                    id: id.to_string(),
                    method: lsproto::MethodWorkspaceDidChangeWatchedFiles.to_string(),
                    register_options: Some(lsproto::RegisterOptions {
                        workspace_did_change_watched_files: Some(
                            lsproto::DidChangeWatchedFilesRegistrationOptions { watchers },
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
            },
        )
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to register file watcher: {err}"),
            )
        })?;
        self.watchers.add(id);
        Ok(())
    }

    pub fn unwatch_files(
        &mut self,
        ctx: context::Context,
        id: project::WatcherID,
    ) -> Result<(), io::Error> {
        if self.watchers.has(&id) {
            send_client_request(
                ctx,
                self,
                lsproto::ClientUnregisterCapabilityInfo.clone(),
                lsproto::UnregistrationParams {
                    unregisterations: vec![lsproto::Unregistration {
                        id: id.to_string(),
                        method: lsproto::MethodWorkspaceDidChangeWatchedFiles.to_string(),
                    }],
                },
            )
            .map_err(|err| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("failed to unregister file watcher: {err}"),
                )
            })?;
            self.watchers.delete(&id);
            return Ok(());
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no file watcher exists with ID {id}"),
        ))
    }

    pub fn refresh_diagnostics(&mut self, _ctx: context::Context) -> Result<(), io::Error> {
        if !self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.workspace.diagnostics.refresh_support)
            .unwrap_or_default()
        {
            return Ok(());
        }

        send_client_request_fire_and_forget(
            self,
            lsproto::WorkspaceDiagnosticRefreshInfo.clone(),
            lsproto::NoParams {},
        )
        .map_err(|err| io::Error::new(err.kind(), format!("failed to refresh diagnostics: {err}")))
    }

    pub fn publish_diagnostics(
        &mut self,
        ctx: context::Context,
        params: &lsproto::PublishDiagnosticsParams,
    ) -> Result<(), io::Error> {
        let _ = ctx;
        send_notification(
            self,
            lsproto::TextDocumentPublishDiagnosticsInfo.clone(),
            params.clone(),
        )
    }

    pub fn send_telemetry(
        &mut self,
        ctx: context::Context,
        telemetry: lsproto::TelemetryEvent,
    ) -> Result<(), io::Error> {
        let _ = ctx;
        if !self.telemetry_enabled {
            panic!("SendTelemetry called with telemetry disabled");
        }
        send_notification(self, lsproto::TelemetryEventInfo.clone(), telemetry)
    }

    pub fn is_active(&self) -> bool {
        let last = self
            .last_request_time_ms
            .load(std::sync::atomic::Ordering::SeqCst);
        last == 0
            || std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH + Duration::from_millis(last as u64))
                .unwrap_or_default()
                <= Duration::from_secs(60)
    }

    pub fn refresh_inlay_hints(&mut self, ctx: context::Context) -> Result<(), io::Error> {
        if !self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.workspace.inlay_hint.refresh_support)
            .unwrap_or_default()
        {
            return Ok(());
        }
        send_client_request(
            ctx,
            self,
            lsproto::WorkspaceInlayHintRefreshInfo.clone(),
            lsproto::NoParams {},
        )
        .map(|_: lsproto::Null| ())
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to refresh inlay hints: {err}"),
            )
        })
    }

    pub fn refresh_code_lens(&mut self, ctx: context::Context) -> Result<(), io::Error> {
        if !self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.workspace.code_lens.refresh_support)
            .unwrap_or_default()
        {
            return Ok(());
        }
        send_client_request(
            ctx,
            self,
            lsproto::WorkspaceCodeLensRefreshInfo.clone(),
            lsproto::NoParams {},
        )
        .map(|_: lsproto::Null| ())
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to refresh code lens: {err}"),
            )
        })
    }

    pub fn progress_start(&mut self, message: &diagnostics::Message, args: Vec<String>) {
        if let Some(project_progress) = &self.project_progress {
            let args = args
                .into_iter()
                .map(|arg| Box::new(arg) as crate::ProgressArg)
                .collect();
            project_progress.start_blocking(message, args);
        }
    }

    pub fn progress_finish(&mut self, message: &diagnostics::Message, args: Vec<String>) {
        if let Some(project_progress) = &self.project_progress {
            let args = args
                .into_iter()
                .map(|arg| Box::new(arg) as crate::ProgressArg)
                .collect();
            project_progress.finish_blocking(message, args);
        }
    }

    pub fn request_configuration(
        &mut self,
        ctx: context::Context,
    ) -> Result<ls::UserPreferences, io::Error> {
        let caps = self
            .client_capabilities
            .as_ref()
            .cloned()
            .unwrap_or_default();
        if !caps.workspace.configuration {
            if let Some(params) = &self.initialize_params {
                if let Some(init_options) = &params.initialization_options {
                    if let Some(user_preferences) = &init_options.user_preferences {
                        if let Some(logger) = &self.logger {
                            logger.logf(format!(
                                "received formatting options from initialization: {user_preferences:?}"
                            ));
                        }
                        if let serde_json::Value::Object(config) = user_preferences.clone() {
                            let mut map = HashMap::new();
                            map.insert("js/ts".to_string(), serde_json::Value::Object(config));
                            return Ok(ls::parse_user_preferences(map));
                        }
                    }
                }
            }
            return Ok(ls::new_default_user_preferences());
        }

        let configs: Vec<serde_json::Value> = send_client_request(
            ctx,
            self,
            lsproto::WorkspaceConfigurationInfo.clone(),
            lsproto::ConfigurationParams {
                items: vec![
                    lsproto::ConfigurationItem {
                        section: Some("js/ts".to_string()),
                        ..Default::default()
                    },
                    lsproto::ConfigurationItem {
                        section: Some("typescript".to_string()),
                        ..Default::default()
                    },
                    lsproto::ConfigurationItem {
                        section: Some("javascript".to_string()),
                        ..Default::default()
                    },
                    lsproto::ConfigurationItem {
                        section: Some("editor".to_string()),
                        ..Default::default()
                    },
                ],
            },
        )
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("configure request failed: {err}"),
            )
        })?;

        let mut config_map = HashMap::new();
        for (i, config) in configs.into_iter().enumerate() {
            match i {
                0 => {
                    config_map.insert("js/ts".to_string(), config);
                }
                1 => {
                    config_map.insert("typescript".to_string(), config);
                }
                2 => {
                    config_map.insert("javascript".to_string(), config);
                }
                3 => {
                    config_map.insert("editor".to_string(), config);
                }
                _ => {}
            }
        }
        if let Some(logger) = &self.logger {
            logger.logf(format!(
                "received options from workspace/configuration request: {config_map:?}"
            ));
        }
        Ok(ls::parse_user_preferences(config_map))
    }

    pub fn run(&mut self, ctx: context::Context) -> Result<(), io::Error> {
        self.background_ctx = Some(ctx.clone());
        let write_running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let write_thread = {
            let running = write_running.clone();
            let queue = self.outgoing_queue.clone();
            let writer = self.w.clone();
            std::thread::spawn(move || {
                while running.load(std::sync::atomic::Ordering::SeqCst)
                    || !queue
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .is_empty()
                {
                    let msg = {
                        let mut queue = queue.lock().unwrap_or_else(|err| err.into_inner());
                        if queue.is_empty() {
                            None
                        } else {
                            Some(queue.remove(0))
                        }
                    };
                    if let Some(msg) = msg {
                        writer
                            .lock()
                            .unwrap_or_else(|err| err.into_inner())
                            .write(&msg)
                            .map_err(|err| {
                                io::Error::new(
                                    err.kind(),
                                    format!("failed to write message: {err}"),
                                )
                            })?;
                    } else {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Ok::<(), io::Error>(())
            })
        };
        let result = loop {
            let msg = match self.read() {
                Ok(msg) => msg,
                Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break Ok(()),
                Err(err) => break Err(err),
            };
            self.process_incoming_message(ctx.clone(), msg)?;
            self.dispatch_loop(ctx.clone())?;
        };
        write_running.store(false, std::sync::atomic::Ordering::SeqCst);
        let write_result = write_thread.join().unwrap_or_else(|_| {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "write thread panicked",
            ))
        });
        result.and(write_result)
    }

    pub fn read_loop(&mut self, ctx: context::Context) -> Result<(), io::Error> {
        loop {
            let msg = self.read()?;
            self.process_incoming_message(ctx.clone(), msg)?;
        }
    }

    fn process_incoming_message(
        &mut self,
        ctx: context::Context,
        msg: lsproto::Message,
    ) -> Result<(), io::Error> {
        if self.initialize_params.is_none() && msg.kind == jsonrpc::MessageKind::Request {
            let req = msg.as_request();
            if req.method == lsproto::MethodInitialize {
                let params = serde_json::from_value(req.params.clone())
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
                let resp = self
                    .handle_initialize(ctx, &params, req)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
                self.send_result(
                    req.id.clone(),
                    serde_json::to_value(resp).unwrap_or_default(),
                )?;
            } else {
                self.send_error(
                    req.id.clone(),
                    &SimpleError(lsproto::ErrorCodeServerNotInitialized.to_string()),
                )?;
            }
            return Ok(());
        }

        if msg.kind == jsonrpc::MessageKind::Response {
            let resp = msg.as_response().clone();
            if let Some(id) = &resp.id {
                let mut pending = self
                    .pending_server_requests
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                if let Some(queue) = pending.get_mut(id) {
                    queue.push(resp);
                }
            }
        } else {
            let req = msg.as_request().clone();
            if req.method == lsproto::MethodCancelRequest {
                let params: lsproto::CancelParams = serde_json::from_value(req.params)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
                self.cancel_request(params.id);
            } else {
                self.request_queue
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push_back(req);
            }
        }
        Ok(())
    }

    pub fn cancel_request(&mut self, raw_id: lsproto::IntegerOrString) {
        let id = lsproto::new_id(raw_id);
        if let Some(pending_req) = self.pending_client_requests.remove(&id) {
            if let Some(cancel) = pending_req.cancel {
                cancel();
            }
        }
    }

    pub fn read(&self) -> Result<lsproto::Message, io::Error> {
        self.r.lock().unwrap_or_else(|err| err.into_inner()).read()
    }

    pub fn dispatch_loop(&mut self, ctx: context::Context) -> Result<(), io::Error> {
        loop {
            let req = self
                .request_queue
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .pop_front();
            let Some(req) = req else {
                break;
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            self.last_request_time_ms
                .store(now, std::sync::atomic::Ordering::SeqCst);
            let request_ctx = if let Some(id) = &req.id {
                let request_ctx = core::context::with_request_id(ctx.clone(), id.to_string());
                self.pending_client_requests.insert(
                    id.clone(),
                    PendingClientRequest {
                        req: Box::new(req.clone()),
                        cancel: None,
                    },
                );
                request_ctx
            } else {
                ctx.clone()
            };

            match self.handle_request_or_notification(request_ctx, &req) {
                Ok(Some(do_async_work)) => {
                    if let Err(err) = do_async_work() {
                        self.send_error(req.id.clone(), err.as_ref())?;
                    }
                    if let Some(id) = &req.id {
                        self.pending_client_requests.remove(id);
                    }
                }
                Ok(None) => {
                    if let Some(id) = &req.id {
                        self.pending_client_requests.remove(id);
                    }
                }
                Err(err) => {
                    self.send_error(req.id.clone(), err.as_ref())?;
                    if let Some(id) = &req.id {
                        self.pending_client_requests.remove(id);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn write_loop(&mut self, ctx: context::Context) -> Result<(), io::Error> {
        let _ = ctx;
        loop {
            let msg = {
                let mut queue = self
                    .outgoing_queue
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                if queue.is_empty() {
                    return Ok(());
                }
                queue.remove(0)
            };
            self.w
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .write(&msg)
                .map_err(|err| {
                    io::Error::new(err.kind(), format!("failed to write message: {err}"))
                })?;
        }
    }

    pub fn send_result(
        &mut self,
        id: Option<jsonrpc::Id>,
        result: serde_json::Value,
    ) -> Result<(), io::Error> {
        self.send_response(&lsproto::ResponseMessage {
            id,
            result,
            ..Default::default()
        })
    }

    pub fn send_error(
        &mut self,
        id: Option<jsonrpc::Id>,
        err: &dyn std::error::Error,
    ) -> Result<(), io::Error> {
        if id.is_none() {
            if let Some(logger) = &self.logger {
                logger.errorf(format!("error handling notification: {err}"));
            }
            return Ok(());
        }
        self.send_response(&lsproto::ResponseMessage {
            id,
            error: Some(jsonrpc::ResponseError {
                code: lsproto::ErrorCodeInternalError,
                message: err.to_string(),
                data: None,
            }),
            ..Default::default()
        })
    }

    pub fn send_response(&mut self, resp: &lsproto::ResponseMessage) -> Result<(), io::Error> {
        self.send(&resp.message())
    }

    pub fn send(&mut self, msg: &lsproto::Message) -> Result<(), io::Error> {
        self.outgoing_queue
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(msg.clone());
        Ok(())
    }

    pub fn handle_request_or_notification(
        &mut self,
        ctx: context::Context,
        req: &lsproto::RequestMessage,
    ) -> Result<Option<AsyncHandler>, Box<dyn std::error::Error + Send + Sync>> {
        let handlers = handlers();
        if let Some(handler) = handlers.get(&req.method) {
            let start = std::time::Instant::now();
            let id_str = req
                .id
                .as_ref()
                .map(|id| format!(" ({id})"))
                .unwrap_or_default();
            let do_async_work = handler(self, ctx, req)?;
            if let Some(logger) = &self.logger {
                logger.infof(format!(
                    "handled method '{}'{} in {:?}",
                    req.method,
                    id_str,
                    start.elapsed()
                ));
            }
            return Ok(do_async_work);
        }
        if let Some(logger) = &self.logger {
            logger.warnf(format!("unknown method '{}'", req.method));
        }
        if req.id.is_some() {
            self.send_error(
                req.id.clone(),
                &SimpleError(lsproto::ErrorCodeInvalidRequest.to_string()),
            )?;
        }
        Ok(None)
    }

    pub fn get_language_service_and_cross_project_orchestrator(
        &mut self,
        ctx: context::Context,
        uri: lsproto::DocumentUri,
        req: &lsproto::RequestMessage,
    ) -> Result<
        (ls::LanguageService<'static>, CrossProjectOrchestrator),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let session = self
            .session
            .as_ref()
            .cloned()
            .ok_or_else(|| SimpleError("server not initialized".to_string()))?;
        let (default_project, default_ls, all_projects) = {
            let mut session_guard = session.lock().unwrap_or_else(|err| err.into_inner());
            let (default_project, default_ls, all_projects) = session_guard
                .get_language_service_and_projects_for_file(ctx, uri)
                .map_err(SimpleError)?;
            let all_projects = all_projects
                .into_iter()
                .map(|project| project.clone())
                .collect::<Vec<_>>();
            (default_project.clone(), default_ls, all_projects)
        };
        let default_project = Arc::new(default_project.clone()) as Arc<dyn ls::Project>;
        let all_projects = all_projects
            .into_iter()
            .map(|project| Arc::new(project) as Arc<dyn ls::Project>)
            .collect();
        Ok((
            default_ls,
            CrossProjectOrchestrator {
                session,
                req: req.clone(),
                default_project,
                all_projects,
            },
        ))
    }

    pub fn recover(&mut self, req: &lsproto::RequestMessage) {
        if let Some(logger) = &self.logger {
            logger.errorf(format!("panic handling request {}", req.method));
        }
        if req.id.is_some() {
            let _ = self.send_error(
                req.id.clone(),
                &SimpleError(format!(
                    "{}: panic handling request {}",
                    lsproto::ErrorCodeInternalError,
                    req.method
                )),
            );
        } else if let Some(logger) = &self.logger {
            logger.errorf(format!("unhandled panic in notification {}", req.method));
        }

        if self.telemetry_enabled {
            let _ = send_notification(
                self,
                lsproto::TelemetryEventInfo.clone(),
                lsproto::TelemetryEvent {
                    request_failure_telemetry_event: Some(lsproto::RequestFailureTelemetryEvent {
                        properties: Some(lsproto::RequestFailureTelemetryProperties {
                            error_code: lsproto::ErrorCodeInternalError.to_string(),
                            request_method: req.method.replace('/', "."),
                            stack: sanitize_stack_trace(""),
                        }),
                    }),
                    performance_stats_telemetry_event: None,
                    project_info_telemetry_event: None,
                },
            );
        }
    }

    pub fn handle_initialize(
        &mut self,
        ctx: context::Context,
        params: &lsproto::InitializeParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::InitializeResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = (ctx, req);
        if self.initialize_params.is_some() {
            return Err(Box::new(SimpleError(
                lsproto::ErrorCodeInvalidRequest.to_string(),
            )));
        }
        self.init_started
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.initialize_params = Some(params.clone());
        self.client_capabilities = Some(params.capabilities.resolve());
        self.callback_state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .client_capabilities = self.client_capabilities.clone();
        if let Some(logger) = &self.logger {
            let capabilities_json =
                json::marshal_indent(self.client_capabilities.as_ref().unwrap(), "", "\t")?;
            logger.infof(format!(
                "Resolved client capabilities: {}",
                String::from_utf8_lossy(&capabilities_json)
            ));
        }
        self.position_encoding = Some(lsproto::PositionEncodingKindUTF16);
        if self
            .client_capabilities
            .as_ref()
            .map(|caps| {
                caps.general
                    .position_encodings
                    .contains(&lsproto::PositionEncodingKindUTF8)
            })
            .unwrap_or_default()
        {
            self.position_encoding = Some(lsproto::PositionEncodingKindUTF8);
        }
        if let Some(locale) = &params.locale {
            let (parsed, ok) = locale::parse(locale);
            if ok {
                self.locale = Some(parsed);
            }
        }
        if self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.window.work_done_progress)
            .unwrap_or_default()
        {
            self.project_progress = Some(new_project_loading_progress(self, self.progress_delay));
            self.callback_state
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .project_progress = self.project_progress.clone();
        }
        if params.trace.as_ref().map(|trace| trace.as_str()) == Some("verbose") {
            if let Some(logger) = &mut self.logger {
                logger.set_verbose(true);
            }
        }
        if let (Some(start_watchdog), Some(process_id)) = (self.start_watchdog, params.process_id) {
            start_watchdog(process_id as i32);
        }

        Ok(lsproto::InitializeResponse {
            initialize_result: Some(lsproto::InitializeResult {
                server_info: Some(lsproto::ServerInfo {
                    name: "typescript-go".to_string(),
                    version: Some(core::version().to_string()),
                }),
                capabilities: lsproto::ServerCapabilities {
                    position_encoding: self.position_encoding.clone().map(
                        |encoding| match encoding {
                            lsproto::PositionEncodingKind::Utf8 => {
                                lsp_types_full::PositionEncodingKind::UTF8
                            }
                            lsproto::PositionEncodingKind::Utf16 => {
                                lsp_types_full::PositionEncodingKind::UTF16
                            }
                            lsproto::PositionEncodingKind::Utf32 => {
                                lsp_types_full::PositionEncodingKind::UTF32
                            }
                        },
                    ),
                    text_document_sync: Some(lsproto::TextDocumentSyncOptionsOrKind::Options(
                        lsproto::TextDocumentSyncOptions {
                            open_close: Some(true),
                            change: Some(lsproto::TextDocumentSyncKind::INCREMENTAL),
                            save: Some(lsproto::BooleanOrSaveOptions::Supported(true)),
                            ..Default::default()
                        },
                    )),
                    hover_provider: Some(lsproto::BooleanOrHoverOptions::Simple(true)),
                    definition_provider: Some(lsproto::BooleanOrDefinitionOptions::Left(true)),
                    references_provider: Some(lsproto::BooleanOrReferenceOptions::Left(true)),
                    workspace: Some(lsproto::WorkspaceOptions {
                        file_operations: Some(lsproto::FileOperationOptions {
                            did_create: None,
                            will_create: None,
                            did_rename: None,
                            will_rename: Some(lsproto::FileOperationRegistrationOptions {
                                filters: file_rename_filters(),
                            }),
                            did_delete: None,
                            will_delete: None,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            }),
        })
    }

    pub fn handle_initialized(
        &mut self,
        ctx: context::Context,
        params: &lsproto::InitializedParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = params;
        let initialize_params = self
            .initialize_params
            .clone()
            .ok_or_else(|| SimpleError("server not initialized".to_string()))?;
        if self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.workspace.did_change_watched_files.dynamic_registration)
            .unwrap_or_default()
        {
            self.watch_enabled = true;
        }

        let mut cwd = self.cwd.clone();
        let single_workspace_folder =
            initialize_params
                .workspace_folders
                .as_deref()
                .and_then(|folders| match folders {
                    [Some(folder)] => Some(folder),
                    _ => None,
                });
        let client_supports_workspace_folders = self
            .client_capabilities
            .as_ref()
            .map(|caps| caps.workspace.workspace_folders)
            .unwrap_or_default();
        if let (true, Some(workspace_folder)) =
            (client_supports_workspace_folders, single_workspace_folder)
        {
            cwd = workspace_folder.uri.to_string().file_name();
        } else if let Some(root_uri) = initialize_params.root_uri {
            cwd = root_uri.file_name();
        } else if let Some(root_path) = initialize_params.root_path {
            cwd = root_path;
        }
        if !tspath::path_is_absolute(&cwd) {
            cwd = self.cwd.clone();
        }

        let disable_push_diagnostics = initialize_params
            .initialization_options
            .as_ref()
            .and_then(|opts| opts.disable_push_diagnostics)
            .unwrap_or_default();
        let enable_telemetry = initialize_params
            .initialization_options
            .as_ref()
            .and_then(|opts| opts.enable_telemetry)
            .unwrap_or_default();
        self.telemetry_enabled = enable_telemetry;
        self.callback_state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .telemetry_enabled = enable_telemetry;

        self.session = Some(Arc::new(Mutex::new(project::new_session(
            project::SessionInit {
                background_ctx: self.background_ctx.clone().unwrap_or_default(),
                options: project::SessionOptions {
                    current_directory: cwd,
                    default_library_path: self.default_library_path.clone(),
                    typings_location: self.typings_location.clone(),
                    position_encoding: self
                        .position_encoding
                        .clone()
                        .unwrap_or(lsproto::PositionEncodingKindUTF16),
                    watch_enabled: self.watch_enabled,
                    logging_enabled: true,
                    telemetry_enabled: enable_telemetry,
                    debounce_delay: Duration::from_millis(500),
                    push_diagnostics_enabled: !disable_push_diagnostics,
                    locale: self.locale.clone().unwrap_or_default(),
                },
                fs: self
                    .fs
                    .clone()
                    .unwrap_or_else(|| Arc::new(vfs::osvfs::os::OsFs::default())),
                logger: self
                    .logger
                    .clone()
                    .map(|logger| Arc::new(logger) as Arc<dyn project::Logger + Send + Sync>)
                    .unwrap_or_else(|| {
                        project::new_log_tree(String::new())
                            as Arc<dyn project::Logger + Send + Sync>
                    }),
                client: Some(
                    self.client
                        .take()
                        .unwrap_or_else(|| Arc::new(self.clone_for_client())),
                ),
                npm_executor: self.clone_for_npm_executor(),
                parse_cache: self.parse_cache.clone(),
            },
        ))));

        let user_preferences = self.request_configuration(ctx.clone())?;
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .initialize_with_user_config(user_preferences);
        }

        send_client_request(
            ctx.clone(),
            self,
            lsproto::ClientRegisterCapabilityInfo.clone(),
            lsproto::RegistrationParams {
                registrations: vec![lsproto::Registration {
                    id: "typescript-config-watch-id".to_string(),
                    method: lsproto::MethodWorkspaceDidChangeConfiguration.to_string(),
                    register_options: Some(lsproto::RegisterOptions {
                        workspace_did_change_configuration: Some(
                            lsproto::DidChangeConfigurationRegistrationOptions {
                                section: Some(lsproto::StringOrStrings::Strings(vec![
                                    "js/ts".to_string(),
                                    "typescript".to_string(),
                                    "javascript".to_string(),
                                    "editor".to_string(),
                                ])),
                            },
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
            },
        )?;

        if let (Some(session), Some(options)) =
            (&self.session, &self.compiler_options_for_inferred_projects)
        {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .did_change_compiler_options_for_inferred_projects(ctx, options.clone());
        }
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .start_performance_telemetry();
        }
        self.init_complete = true;
        self.init_complete_signal
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    pub fn handle_shutdown(
        &mut self,
        ctx: context::Context,
        params: lsproto::NoParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::ShutdownResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = (ctx, params, req);
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .close();
        }
        Ok(lsproto::ShutdownResponse::default())
    }
    pub fn handle_exit(
        &mut self,
        ctx: context::Context,
        params: lsproto::NoParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = (ctx, params);
        Err(Box::new(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "exit",
        )))
    }
    pub fn handle_did_change_workspace_configuration(
        &mut self,
        ctx: context::Context,
        params: &lsproto::DidChangeConfigurationParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = ctx;
        if !params.settings.is_null() {
            if let Some(session) = &self.session {
                let mut config_map = HashMap::new();
                if let serde_json::Value::Object(settings) = params.settings.clone() {
                    config_map.extend(settings);
                }
                session
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .configure(ls::parse_user_preferences(config_map));
            }
        }
        Ok(())
    }
    pub fn handle_did_open(
        &mut self,
        ctx: context::Context,
        params: &lsproto::DidOpenTextDocumentParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .did_open_file(
                    ctx,
                    params.text_document.uri.to_string(),
                    params.text_document.version,
                    params.text_document.text.clone(),
                    params.text_document.language_id.clone(),
                );
        }
        Ok(())
    }
    pub fn handle_did_change(
        &mut self,
        ctx: context::Context,
        params: &lsproto::DidChangeTextDocumentParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .did_change_file(
                    ctx,
                    params.text_document.uri.to_string(),
                    params.text_document.version,
                    params.content_changes.clone(),
                );
        }
        Ok(())
    }
    pub fn handle_did_save(
        &mut self,
        ctx: context::Context,
        params: &lsproto::DidSaveTextDocumentParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .did_save_file(ctx, params.text_document.uri.to_string());
        }
        Ok(())
    }
    pub fn handle_did_close(
        &mut self,
        ctx: context::Context,
        params: &lsproto::DidCloseTextDocumentParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .did_close_file(ctx, params.text_document.uri.to_string());
        }
        Ok(())
    }
    pub fn handle_did_change_watched_files(
        &mut self,
        ctx: context::Context,
        params: &lsproto::DidChangeWatchedFilesParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .did_change_watched_files(ctx, params.changes.clone());
        }
        Ok(())
    }
    pub fn handle_set_trace(
        &mut self,
        ctx: context::Context,
        params: &lsproto::SetTraceParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = ctx;
        match params.value.as_str() {
            "verbose" => self.logger.as_mut().map(|logger| logger.set_verbose(true)),
            "messages" | "off" => self.logger.as_mut().map(|logger| logger.set_verbose(false)),
            _ => {
                return Err(Box::new(SimpleError(format!(
                    "unknown trace value: {}",
                    params.value.as_str()
                ))));
            }
        };
        Ok(())
    }
    pub fn handle_document_diagnostic(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::DocumentDiagnosticParams,
    ) -> Result<lsproto::DocumentDiagnosticResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_diagnostics(&ctx, params.text_document.uri.to_string())
            .map_err(box_string)
    }
    pub fn handle_hover(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::HoverParams,
    ) -> Result<lsproto::HoverResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_hover(&ctx, params).map_err(box_string)
    }
    pub fn handle_prepare_rename(
        &mut self,
        ctx: context::Context,
        language_service: &ls::LanguageService,
        params: &lsproto::PrepareRenameParams,
    ) -> Result<lsproto::PrepareRenameResponse, Box<dyn std::error::Error + Send + Sync>> {
        let info = language_service.get_rename_info(
            &ctx,
            "",
            params.text_document.uri.to_string(),
            params.position,
        )?;
        if !info.can_rename {
            return Err(Box::new(UserFacingRequestFailedError(
                info.localized_error_message,
            )));
        }
        Ok(lsproto::PrepareRenameResponse::PrepareRenamePlaceholder(
            lsproto::PrepareRenamePlaceholder {
                range: info.trigger_span,
                placeholder: info.display_name,
            },
        ))
    }
    pub fn handle_rename(
        &mut self,
        ctx: context::Context,
        params: &lsproto::RenameParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::RenameResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (default_ls, orchestrator) = self.get_language_service_and_cross_project_orchestrator(
            ctx.clone(),
            params.text_document.uri.clone(),
            req,
        )?;
        let info = default_ls.get_rename_info(
            &ctx,
            &params.new_name,
            params.text_document.uri.clone(),
            params.position,
        )?;
        if info.can_rename && !info.file_to_rename.is_empty() {
            if ls::client_supports_will_rename_files(&ctx) {
                return Ok(lsproto::WorkspaceEditOrNull::WorkspaceEdit(
                    lsproto::WorkspaceEdit {
                        document_changes: Some(vec![
                            lsproto::TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile {
                                rename_file: Some(lsproto::RenameFile {
                                    kind: lsproto::StringLiteralRename {},
                                    old_uri: ls::file_name_to_document_uri(&info.file_to_rename),
                                    new_uri: ls::file_name_to_document_uri(&info.new_file_name),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        ]),
                        ..Default::default()
                    },
                ));
            }
            let rename_files_params = lsproto::RenameFilesParams {
                files: vec![lsproto::FileRename {
                    old_uri: ls::file_name_to_document_uri(&info.file_to_rename).to_string(),
                    new_uri: ls::file_name_to_document_uri(&info.new_file_name).to_string(),
                }],
            };
            return self.handle_will_rename_files_worker(ctx, &rename_files_params, req, true);
        }
        default_ls
            .provide_rename(&ctx, params, Some(&orchestrator))
            .map_err(box_string)
    }
    pub fn handle_will_rename_files(
        &mut self,
        ctx: context::Context,
        params: &lsproto::RenameFilesParams,
        msg: &lsproto::RequestMessage,
    ) -> Result<lsproto::WillRenameFilesResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.handle_will_rename_files_worker(ctx, params, msg, false)
    }
    pub fn handle_will_rename_files_worker(
        &mut self,
        ctx: context::Context,
        params: &lsproto::RenameFilesParams,
        msg: &lsproto::RequestMessage,
        send_rename_file: bool,
    ) -> Result<lsproto::WillRenameFilesResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = msg;
        if params.files.is_empty() {
            return Ok(lsproto::WillRenameFilesResponse::default());
        }

        let uris = params
            .files
            .iter()
            .map(|file| file.old_uri.clone())
            .collect::<Vec<_>>();
        if uris.is_empty() {
            return Ok(lsproto::WillRenameFilesResponse::default());
        }

        let services = {
            let session = self
                .session
                .as_ref()
                .cloned()
                .ok_or_else(|| SimpleError("server not initialized".to_string()))?;
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .get_language_services_for_documents(ctx.clone(), uris)
                .into_iter()
                .collect::<Vec<_>>()
        };

        let mut seen_edits: HashMap<(lsproto::DocumentUri, lsproto::Range), String> =
            HashMap::new();
        let mut seen_renames: HashMap<lsproto::DocumentUri, bool> = HashMap::new();
        let mut document_changes = Vec::new();

        for language_service in &services {
            for file in &params.files {
                let changes = language_service.get_edits_for_file_rename(
                    &ctx,
                    file.old_uri.clone(),
                    file.new_uri.clone(),
                )?;
                for change in changes {
                    if let Some(rename_file) = &change.rename_file {
                        if !seen_renames.contains_key(&rename_file.old_uri) {
                            seen_renames.insert(rename_file.old_uri.clone(), true);
                            document_changes.push(change);
                        }
                    } else if let Some(text_document_edit) = &change.text_document_edit {
                        let uri = text_document_edit.text_document.uri.clone();
                        let mut deduped = Vec::new();
                        for edit in &text_document_edit.edits {
                            if let Some(text_edit) = &edit.text_edit {
                                let key = (uri.clone(), text_edit.range);
                                if seen_edits
                                    .get(&key)
                                    .map(|prev| prev == &text_edit.new_text)
                                    .unwrap_or_default()
                                {
                                    continue;
                                }
                                seen_edits.insert(key, text_edit.new_text.clone());
                            }
                            deduped.push(edit.clone());
                        }
                        if !deduped.is_empty() {
                            document_changes.push(
                                lsproto::TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile {
                                    text_document_edit: Some(lsproto::TextDocumentEdit {
                                        text_document: text_document_edit.text_document.clone(),
                                        edits: deduped,
                                    }),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }
        }

        if send_rename_file {
            for file in &params.files {
                document_changes.push(
                    lsproto::TextDocumentEditOrCreateFileOrRenameFileOrDeleteFile {
                        rename_file: Some(lsproto::RenameFile {
                            kind: lsproto::StringLiteralRename {},
                            old_uri: file.old_uri.clone(),
                            new_uri: file.new_uri.clone(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            }
        }

        if document_changes.is_empty() {
            return Ok(lsproto::WillRenameFilesResponse::default());
        }

        if ls::client_supports_document_changes(&ctx) {
            return Ok(lsproto::WillRenameFilesResponse {
                workspace_edit: Some(lsproto::WorkspaceEdit {
                    document_changes: Some(document_changes),
                    ..Default::default()
                }),
            });
        }

        let mut changes: HashMap<lsproto::DocumentUri, Vec<lsproto::TextEdit>> = HashMap::new();
        for change in document_changes {
            if let Some(text_document_edit) = change.text_document_edit {
                let uri = text_document_edit.text_document.uri;
                for edit in text_document_edit.edits {
                    if let Some(text_edit) = edit.text_edit {
                        changes.entry(uri.clone()).or_default().push(text_edit);
                    }
                }
            }
        }
        Ok(lsproto::WillRenameFilesResponse {
            workspace_edit: Some(lsproto::WorkspaceEdit {
                changes: Some(changes),
                ..Default::default()
            }),
        })
    }
    pub fn handle_signature_help(
        &mut self,
        ctx: context::Context,
        language_service: &ls::LanguageService,
        params: &lsproto::SignatureHelpParams,
    ) -> Result<lsproto::SignatureHelpResponse, Box<dyn std::error::Error + Send + Sync>> {
        language_service
            .provide_signature_help(
                &ctx,
                params.text_document.uri.clone(),
                params.position,
                params.context.as_ref(),
            )
            .map_err(box_string)
    }
    pub fn handle_folding_range(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::FoldingRangeParams,
    ) -> Result<lsproto::FoldingRangeResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_folding_range(&ctx, params.text_document.uri.to_string())
            .map_err(box_string)
    }
    pub fn handle_vs_on_auto_insert(
        &mut self,
        _ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::VsOnAutoInsertParams,
    ) -> Result<lsproto::VsOnAutoInsertResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_on_auto_insert(params)
            .map_err(|err| SimpleError(format!("{err:?}")).into())
    }
    pub fn handle_linked_editing_range(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::LinkedEditingRangeParams,
    ) -> Result<lsproto::LinkedEditingRangeResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_linked_editing_range(&ctx, params)
            .map_err(box_string)
    }
    pub fn handle_definition(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::DefinitionParams,
    ) -> Result<lsproto::DefinitionResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_definition(&ctx, params.text_document.uri.clone(), params.position)
            .map_err(box_string)
    }
    pub fn handle_source_definition(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::TextDocumentPositionParams,
    ) -> Result<
        lsproto::CustomTextDocumentSourceDefinitionResponse,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let resp = ls
            .provide_source_definition(&ctx, params.text_document.uri.clone(), params.position)
            .map_err(box_string)?;
        Ok(resp)
    }
    pub fn handle_type_definition(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::TypeDefinitionParams,
    ) -> Result<lsproto::TypeDefinitionResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_type_definition(&ctx, params.text_document.uri.to_string(), params.position)
            .map_err(box_string)
    }
    pub fn handle_completion(
        &mut self,
        ctx: context::Context,
        language_service: &ls::LanguageService,
        params: &lsproto::CompletionParams,
    ) -> Result<lsproto::CompletionResponse, Box<dyn std::error::Error + Send + Sync>> {
        language_service
            .provide_completion(
                ctx,
                params.text_document.uri.clone(),
                params.position,
                params.context.as_ref(),
            )
            .map_err(box_string)
    }
    pub fn handle_completion_item_resolve(
        &mut self,
        ctx: context::Context,
        params: &lsproto::CompletionItem,
        req_msg: &lsproto::RequestMessage,
    ) -> Result<lsproto::CompletionResolveResponse, Box<dyn std::error::Error + Send + Sync>> {
        let data = params.data.as_ref();
        let Some(data) = data else {
            return Err(Box::new(SimpleError(
                "completion item data is nil".to_string(),
            )));
        };
        let language_service = {
            let session = self
                .session
                .as_ref()
                .cloned()
                .ok_or_else(|| SimpleError("server not initialized".to_string()))?;
            let language_service = session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .get_language_service(ctx.clone(), ls::file_name_to_document_uri(&data.file_name))
                .map_err(SimpleError)?;
            language_service
        };
        self.recover(req_msg);
        language_service
            .resolve_completion_item(&ctx, &mut params.clone(), Some(data))
            .map(|completion_item| lsproto::CompletionItemOrNull {
                completion_item: Some(completion_item),
            })
            .map_err(box_string)
    }
    pub fn handle_document_format(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::DocumentFormattingParams,
    ) -> Result<lsproto::DocumentFormattingResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_format_document(&ctx, params.text_document.uri.to_string(), &params.options)
            .map_err(box_string)
    }
    pub fn handle_document_range_format(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::DocumentRangeFormattingParams,
    ) -> Result<lsproto::DocumentRangeFormattingResponse, Box<dyn std::error::Error + Send + Sync>>
    {
        ls.provide_format_document_range(
            &ctx,
            params.text_document.uri.to_string(),
            &params.options,
            params.range,
        )
        .map_err(box_string)
    }
    pub fn handle_document_on_type_format(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::DocumentOnTypeFormattingParams,
    ) -> Result<lsproto::DocumentOnTypeFormattingResponse, Box<dyn std::error::Error + Send + Sync>>
    {
        ls.provide_format_document_on_type(
            &ctx,
            params.text_document.uri.to_string(),
            &params.options,
            params.position,
            params.ch.clone(),
        )
        .map_err(box_string)
    }
    pub fn handle_workspace_symbol(
        &mut self,
        ctx: context::Context,
        params: &lsproto::WorkspaceSymbolParams,
        req_msg: &lsproto::RequestMessage,
    ) -> Result<lsproto::WorkspaceSymbolResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut resp = Default::default();
        let mut ls_err = None;
        self.recover(req_msg);
        if let Some(session) = &self.session {
            match session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .provide_workspace_symbols_loading_project_tree(
                    ctx.clone(),
                    project::ProjectTreeRequest::default(),
                    &params.query,
                ) {
                Ok(value) => resp = value,
                Err(err) => ls_err = Some(err),
            }
        }
        if let Some(err) = ls_err {
            return Err(box_string(err));
        }
        Ok(resp)
    }
    pub fn handle_document_symbol(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::DocumentSymbolParams,
    ) -> Result<lsproto::DocumentSymbolResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_document_symbols(&ctx, params.text_document.uri.to_string())
            .map_err(box_string)
    }
    pub fn handle_document_highlight(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::DocumentHighlightParams,
    ) -> Result<lsproto::DocumentHighlightResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_document_highlights(
            &ctx,
            params
                .text_document_position_params
                .text_document
                .uri
                .to_string(),
            params.text_document_position_params.position,
        )
        .map_err(box_string)
    }
    pub fn handle_multi_document_highlight(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::MultiDocumentHighlightParams,
    ) -> Result<
        lsproto::CustomMultiDocumentHighlightResponse,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        ls.provide_multi_document_highlights(
            &ctx,
            params.text_document.uri.to_string(),
            params.position,
            params.files_to_search.clone(),
        )
        .map_err(box_string)
    }
    pub fn handle_selection_range(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::SelectionRangeParams,
    ) -> Result<lsproto::SelectionRangeResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_selection_ranges(&ctx, params)
            .map_err(box_string)
    }
    pub fn handle_code_action(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::CodeActionParams,
    ) -> Result<lsproto::CodeActionResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_code_actions(&ctx, params).map_err(box_string)
    }
    pub fn handle_inlay_hint(
        &mut self,
        ctx: context::Context,
        language_service: &ls::LanguageService,
        params: &lsproto::InlayHintParams,
    ) -> Result<lsproto::InlayHintResponse, Box<dyn std::error::Error + Send + Sync>> {
        language_service
            .provide_inlay_hint(&ctx, params)
            .map_err(box_string)
    }
    pub fn handle_code_lens(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::CodeLensParams,
    ) -> Result<lsproto::CodeLensResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_code_lenses(&ctx, params.text_document.uri.to_string())
            .map_err(box_string)
    }
    pub fn handle_code_lens_resolve(
        &mut self,
        ctx: context::Context,
        code_lens: &lsproto::CodeLens,
        req_msg: &lsproto::RequestMessage,
    ) -> Result<lsproto::CodeLens, Box<dyn std::error::Error + Send + Sync>> {
        let show_locations_command_name = self
            .initialize_params
            .as_ref()
            .and_then(|params| params.initialization_options.as_ref())
            .and_then(|opts| opts.code_lens_show_locations_command_name.clone());
        self.recover(req_msg);
        let (default_ls, orchestrator) = self
            .get_language_service_and_cross_project_orchestrator(
                ctx.clone(),
                code_lens
                    .data
                    .as_ref()
                    .ok_or_else(|| SimpleError("code lens data is nil".to_string()))?
                    .uri
                    .clone(),
                req_msg,
            )
            .map_err(|_| SimpleError(lsproto::ErrorCodeContentModified.to_string()))?;
        default_ls
            .resolve_code_lens(
                &ctx,
                code_lens.clone(),
                show_locations_command_name.as_ref(),
                Some(&orchestrator),
            )
            .map_err(box_string)
    }
    pub fn handle_prepare_call_hierarchy(
        &mut self,
        ctx: context::Context,
        language_service: &ls::LanguageService,
        params: &lsproto::CallHierarchyPrepareParams,
    ) -> Result<lsproto::CallHierarchyPrepareResponse, Box<dyn std::error::Error + Send + Sync>>
    {
        language_service
            .provide_prepare_call_hierarchy(&ctx, params.text_document.uri.clone(), params.position)
            .map_err(|err| SimpleError(format!("{err:?}")).into())
    }
    pub fn handle_call_hierarchy_incoming_calls(
        &mut self,
        ctx: context::Context,
        params: &lsproto::CallHierarchyIncomingCallsParams,
        req_msg: &lsproto::RequestMessage,
    ) -> Result<lsproto::CallHierarchyIncomingCallsResponse, Box<dyn std::error::Error + Send + Sync>>
    {
        let (default_ls, orchestrator) = self.get_language_service_and_cross_project_orchestrator(
            ctx.clone(),
            params.item.uri.clone(),
            req_msg,
        )?;
        default_ls
            .provide_call_hierarchy_incoming_calls(&ctx, &params.item, &orchestrator)
            .map_err(|err| SimpleError(format!("{err:?}")).into())
    }
    pub fn handle_call_hierarchy_outgoing_calls(
        &mut self,
        ctx: context::Context,
        params: &lsproto::CallHierarchyOutgoingCallsParams,
        req_msg: &lsproto::RequestMessage,
    ) -> Result<lsproto::CallHierarchyOutgoingCallsResponse, Box<dyn std::error::Error + Send + Sync>>
    {
        let _ = req_msg;
        let language_service = {
            let session = self
                .session
                .as_ref()
                .cloned()
                .ok_or_else(|| SimpleError("server not initialized".to_string()))?;
            let language_service = session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .get_language_service(ctx.clone(), params.item.uri.clone())
                .map_err(SimpleError)?;
            language_service
        };
        language_service
            .provide_call_hierarchy_outgoing_calls(&ctx, &params.item)
            .map_err(|err| SimpleError(format!("{err:?}")).into())
    }
    pub fn handle_semantic_tokens_full(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::SemanticTokensParams,
    ) -> Result<lsproto::SemanticTokensResponse, Box<dyn std::error::Error + Send + Sync>> {
        ls.provide_semantic_tokens(&ctx, params.text_document.uri.to_string())
            .map_err(box_string)
    }
    pub fn handle_semantic_tokens_range(
        &mut self,
        ctx: context::Context,
        ls: &ls::LanguageService,
        params: &lsproto::SemanticTokensRangeParams,
    ) -> Result<lsproto::SemanticTokensRangeResponse, Box<dyn std::error::Error + Send + Sync>>
    {
        ls.provide_semantic_tokens_range(&ctx, params.text_document.uri.to_string(), params.range)
            .map_err(box_string)
    }
    pub fn handle_initialize_api_session(
        &mut self,
        ctx: context::Context,
        params: &lsproto::InitializeAPISessionParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::CustomInitializeAPISessionResponse, Box<dyn std::error::Error + Send + Sync>>
    {
        let _ = req;
        let project_session = self
            .session
            .as_ref()
            .ok_or_else(|| SimpleError("server not initialized".to_string()))?
            .clone();
        let api_session = Arc::new(api::new_session_with_project_session(
            project_session,
            api::SessionOptions::default(),
        ));
        let pipe_path = params
            .pipe
            .as_ref()
            .filter(|pipe| !pipe.is_empty())
            .cloned()
            .unwrap_or_else(|| self.generate_api_pipe_path());
        let transport = api::new_pipe_transport(&pipe_path)
            .map_err(|err| SimpleError(format!("failed to create API transport: {err}")))?;
        let logger = self.logger.clone();
        let api_sessions = self.api_sessions.clone();
        api::spawn_pipe_session(
            self.background_ctx.clone().unwrap_or(ctx),
            transport,
            api_session.clone(),
            Arc::new(move |message| {
                if let Some(logger) = &logger {
                    logger.errorf(message);
                }
            }),
            Box::new(move |id| {
                api_sessions
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .remove(&id);
            }),
        );
        self.api_sessions
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(api_session.id().to_string(), api_session.clone());
        Ok(lsproto::InitializeAPISessionResultOrNull {
            initialize_api_session_result: Some(lsproto::InitializeAPISessionResult {
                session_id: api_session.id().to_string(),
                pipe: pipe_path,
            }),
        })
    }
    pub fn generate_api_pipe_path(&mut self) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let rnd = self
            .watcher_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        api::generate_pipe_path(&format!("tsgo-api-{now:x}-{rnd:x}"))
    }
    pub fn remove_api_session(&mut self, id: String) {
        self.api_sessions
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(&id);
    }
    pub fn set_compiler_options_for_inferred_projects(
        &mut self,
        ctx: context::Context,
        options: &core::CompilerOptions,
    ) {
        self.compiler_options_for_inferred_projects = Some(options.clone());
        if let Some(session) = &self.session {
            session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .did_change_compiler_options_for_inferred_projects(ctx, options.clone());
        }
    }
    pub fn npm_install(&mut self, cwd: String, args: Vec<String>) -> Result<Vec<u8>, io::Error> {
        (self.npm_install.expect("npmInstall callback is required"))(cwd, args)
    }
    pub fn handle_run_gc(
        &mut self,
        ctx: context::Context,
        params: lsproto::NoParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::RunGCResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = (ctx, params, req);
        #[cfg(feature = "pprof")]
        {
            pprof::run_gc();
            if let Some(logger) = &self.logger {
                logger.infof("GC triggered");
            }
            return Ok(lsproto::Null);
        }
        #[cfg(not(feature = "pprof"))]
        if let Some(logger) = &self.logger {
            logger.infof("GC profiling command ignored; pprof is disabled in this Rust build");
        }
        #[cfg(not(feature = "pprof"))]
        Ok(lsproto::Null)
    }
    pub fn handle_save_heap_profile(
        &mut self,
        ctx: context::Context,
        params: &lsproto::ProfileParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::SaveHeapProfileResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = (ctx, params, req);
        #[cfg(feature = "pprof")]
        {
            let file_path = pprof::save_heap_profile(&params.dir)?;
            if let Some(logger) = &self.logger {
                logger.infof(format!("Heap profile saved to: {}", file_path.display()));
            }
            return Ok(lsproto::ProfileResultOrNull {
                profile_result: Some(lsproto::ProfileResult {
                    file: file_path.to_string_lossy().into_owned(),
                }),
            });
        }
        #[cfg(not(feature = "pprof"))]
        Err(profile_disabled_error())
    }
    pub fn handle_save_alloc_profile(
        &mut self,
        ctx: context::Context,
        params: &lsproto::ProfileParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::SaveAllocProfileResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = (ctx, params, req);
        #[cfg(feature = "pprof")]
        {
            let file_path = pprof::save_alloc_profile(&params.dir)?;
            if let Some(logger) = &self.logger {
                logger.infof(format!(
                    "Allocation profile saved to: {}",
                    file_path.display()
                ));
            }
            return Ok(lsproto::ProfileResultOrNull {
                profile_result: Some(lsproto::ProfileResult {
                    file: file_path.to_string_lossy().into_owned(),
                }),
            });
        }
        #[cfg(not(feature = "pprof"))]
        Err(profile_disabled_error())
    }
    pub fn handle_start_cpu_profile(
        &mut self,
        ctx: context::Context,
        params: &lsproto::ProfileParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::StartCPUProfileResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = (ctx, params, req);
        #[cfg(feature = "pprof")]
        {
            self.cpu_profiler.start_cpu_profile(&params.dir)?;
            if let Some(logger) = &self.logger {
                logger.infof(format!(
                    "CPU profiling started, will save to: {}",
                    params.dir
                ));
            }
            return Ok(lsproto::Null);
        }
        #[cfg(not(feature = "pprof"))]
        Err(profile_disabled_error())
    }
    pub fn handle_stop_cpu_profile(
        &mut self,
        ctx: context::Context,
        params: lsproto::NoParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::StopCPUProfileResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = (ctx, params, req);
        #[cfg(feature = "pprof")]
        {
            let file_path = self.cpu_profiler.stop_cpu_profile()?;
            if let Some(logger) = &self.logger {
                logger.infof(format!("CPU profile saved to: {}", file_path.display()));
            }
            return Ok(lsproto::ProfileResultOrNull {
                profile_result: Some(lsproto::ProfileResult {
                    file: file_path.to_string_lossy().into_owned(),
                }),
            });
        }
        #[cfg(not(feature = "pprof"))]
        Err(profile_disabled_error())
    }
    pub fn handle_project_info(
        &mut self,
        ctx: context::Context,
        params: &lsproto::ProjectInfoParams,
        req: &lsproto::RequestMessage,
    ) -> Result<lsproto::CustomProjectInfoResponse, Box<dyn std::error::Error + Send + Sync>> {
        let _ = req;
        let config_file_path = {
            let session = self
                .session
                .as_ref()
                .cloned()
                .ok_or_else(|| SimpleError("server not initialized".to_string()))?;
            let (default_project, _, _) = session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .get_language_service_and_projects_for_file(
                    ctx,
                    params.text_document.uri.to_string(),
                )
                .map_err(SimpleError)?;
            if default_project.kind() == project::Kind::Configured {
                default_project.name()
            } else {
                String::new()
            }
        };
        Ok(lsproto::ProjectInfoResultOrNull {
            project_info_result: Some(lsproto::ProjectInfoResult { config_file_path }),
        })
    }
}

struct ServerClient {
    state: Arc<Mutex<ServerCallbackState>>,
}

impl project::Client for ServerClient {
    fn watch_files(
        &self,
        ctx: &core::Context,
        id: project::WatcherID,
        watchers: Vec<project::FileSystemWatcher>,
    ) -> Result<(), String> {
        let _ = ctx;
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .watch_files(id, watchers)
            .map_err(|err| err.to_string())
    }

    fn unwatch_files(&self, ctx: &core::Context, id: project::WatcherID) -> Result<(), String> {
        let _ = ctx;
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .unwatch_files(id)
            .map_err(|err| err.to_string())
    }

    fn refresh_diagnostics(&self, ctx: &core::Context) -> Result<(), String> {
        let _ = ctx;
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .refresh_diagnostics()
            .map_err(|err| err.to_string())
    }

    fn publish_diagnostics(
        &self,
        ctx: &core::Context,
        params: project::PublishDiagnosticsParams,
    ) -> Result<(), String> {
        let _ = ctx;
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .publish_diagnostics(params);
        Ok(())
    }

    fn refresh_inlay_hints(&self, ctx: &core::Context) -> Result<(), String> {
        let _ = ctx;
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .refresh_inlay_hints()
            .map_err(|err| err.to_string())
    }

    fn refresh_code_lens(&self, ctx: &core::Context) -> Result<(), String> {
        let _ = ctx;
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .refresh_code_lens()
            .map_err(|err| err.to_string())
    }

    fn progress_start(&self, message: &project::DiagnosticsMessage, args: &[String]) {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .progress_start(message, args.to_vec());
    }

    fn progress_finish(&self, message: &project::DiagnosticsMessage, args: &[String]) {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .progress_finish(message, args.to_vec());
    }

    fn send_telemetry(
        &self,
        ctx: &core::Context,
        telemetry: project::TelemetryEvent,
    ) -> Result<(), String> {
        let _ = ctx;
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .send_telemetry(telemetry);
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .is_active()
    }
}

struct ServerNpmExecutor {
    state: Arc<Mutex<ServerCallbackState>>,
}

impl project::NpmExecutor for ServerNpmExecutor {
    fn npm_install(&self, cwd: &str, args: &[String]) -> Result<Vec<u8>, String> {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .npm_install(cwd.to_string(), args.to_vec())
            .map_err(|err| err.to_string())
    }
}

impl Server {
    fn clone_for_client(&self) -> ServerClient {
        ServerClient {
            state: self.callback_state.clone(),
        }
    }

    fn clone_for_npm_executor(&self) -> Option<Box<dyn project::NpmExecutor>> {
        self.npm_install.map(|_| {
            Box::new(ServerNpmExecutor {
                state: self.callback_state.clone(),
            }) as Box<dyn project::NpmExecutor>
        })
    }
}

pub type AsyncHandler =
    Box<dyn FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send>;
pub type HandlerFn = Box<
    dyn Fn(
            &mut Server,
            context::Context,
            &lsproto::RequestMessage,
        ) -> Result<Option<AsyncHandler>, Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync,
>;
pub type HandlerMap = HashMap<lsproto::Method, HandlerFn>;

pub struct UserFacingRequestFailedError(pub String);

pub struct SimpleError(pub String);

impl std::fmt::Display for SimpleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Debug for SimpleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SimpleError").field(&self.0).finish()
    }
}

impl std::error::Error for SimpleError {}

fn box_string(err: impl ToString) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(SimpleError(err.to_string()))
}

fn is_needs_auto_imports_error(err: &(dyn std::error::Error + Send + Sync)) -> bool {
    err.to_string() == ls::ERR_NEEDS_AUTO_IMPORTS
}

#[cfg(not(feature = "pprof"))]
fn profile_disabled_error() -> Box<dyn std::error::Error + Send + Sync> {
    box_string("pprof profiling is disabled in this Rust build")
}

impl std::fmt::Display for UserFacingRequestFailedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Debug for UserFacingRequestFailedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("UserFacingRequestFailedError")
            .field(&self.0)
            .finish()
    }
}

impl std::error::Error for UserFacingRequestFailedError {}

pub fn handlers() -> HandlerMap {
    let mut handlers = HashMap::new();
    register_notification_handler(
        &mut handlers,
        lsproto::InitializedInfo.clone(),
        |s, ctx, params| s.handle_initialized(ctx, &params),
    );
    register_notification_handler(
        &mut handlers,
        lsproto::WorkspaceDidChangeConfigurationInfo.clone(),
        |s, ctx, params| s.handle_did_change_workspace_configuration(ctx, &params),
    );
    register_notification_handler(
        &mut handlers,
        lsproto::TextDocumentDidOpenInfo.clone(),
        |s, ctx, params| s.handle_did_open(ctx, &params),
    );
    register_notification_handler(
        &mut handlers,
        lsproto::TextDocumentDidChangeInfo.clone(),
        |s, ctx, params| s.handle_did_change(ctx, &params),
    );
    register_notification_handler(
        &mut handlers,
        lsproto::TextDocumentDidCloseInfo.clone(),
        |s, ctx, params| s.handle_did_close(ctx, &params),
    );
    register_notification_handler(
        &mut handlers,
        lsproto::WorkspaceDidChangeWatchedFilesInfo.clone(),
        |s, ctx, params| s.handle_did_change_watched_files(ctx, &params),
    );
    register_language_service_with_auto_imports_request_handler(
        &mut handlers,
        lsproto::TextDocumentCompletionInfo.clone(),
        |s, ctx, ls, params| s.handle_completion(ctx, ls, &params),
    );
    handlers.insert(
        lsproto::TextDocumentDocumentSymbolInfo.method.clone(),
        Box::new(|s, ctx, req| {
            let params: lsproto::DocumentSymbolParams = serde_json::from_value(req.params.clone())
                .map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
            let language_service = {
                let session = s.session.as_ref().cloned().ok_or_else(|| {
                    Box::new(SimpleError("server not initialized".to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                session
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .get_language_service(ctx.clone(), params.text_document.uri.to_string())
                    .map_err(box_string)?
            };
            let resp = s.handle_document_symbol(ctx, &language_service, &params)?;
            if let Some(session) = &s.session {
                session
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .enqueue_publish_global_diagnostics();
            }
            s.send_result(req.id.clone(), serde_json::to_value(resp)?)?;
            Ok(None)
        }),
    );
    register_multi_project_reference_request_handler(
        &mut handlers,
        lsproto::TextDocumentReferencesInfo.clone(),
        |ls, ctx, params, orchestrator| {
            ls.provide_references(&ctx, &params, Some(orchestrator))
                .map_err(box_string)
        },
    );
    handlers.insert(
        lsproto::TextDocumentSemanticTokensFullInfo.method.clone(),
        Box::new(|s, ctx, req| {
            let params: lsproto::SemanticTokensParams = serde_json::from_value(req.params.clone())
                .map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
            let language_service = {
                let session = s.session.as_ref().cloned().ok_or_else(|| {
                    Box::new(SimpleError("server not initialized".to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                session
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .get_language_service(ctx.clone(), params.text_document.uri.to_string())
                    .map_err(box_string)?
            };
            let resp = s.handle_semantic_tokens_full(ctx, &language_service, &params)?;
            if let Some(session) = &s.session {
                session
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .enqueue_publish_global_diagnostics();
            }
            s.send_result(req.id.clone(), serde_json::to_value(resp)?)?;
            Ok::<Option<AsyncHandler>, Box<dyn std::error::Error + Send + Sync>>(None)
        }),
    );
    register_request_handler(
        &mut handlers,
        lsproto::CustomProjectInfoInfo.clone(),
        |s, ctx, params, req| s.handle_project_info(ctx, &params, req),
    );
    register_request_handler(
        &mut handlers,
        lsproto::CustomRunGCInfo.clone(),
        Server::handle_run_gc,
    );
    handlers.insert(
        lsproto::CustomSaveHeapProfileInfo.method.clone(),
        Box::new(|s, ctx, req| {
            let params: lsproto::ProfileParams = serde_json::from_value(req.params.clone())?;
            let resp = s.handle_save_heap_profile(ctx, &params, req)?;
            s.send_result(req.id.clone(), serde_json::to_value(resp)?)?;
            Ok(None)
        }),
    );
    handlers.insert(
        lsproto::CustomSaveAllocProfileInfo.method.clone(),
        Box::new(|s, ctx, req| {
            let params: lsproto::ProfileParams = serde_json::from_value(req.params.clone())?;
            let resp = s.handle_save_alloc_profile(ctx, &params, req)?;
            s.send_result(req.id.clone(), serde_json::to_value(resp)?)?;
            Ok(None)
        }),
    );
    handlers.insert(
        lsproto::CustomStartCPUProfileInfo.method.clone(),
        Box::new(|s, ctx, req| {
            let params: lsproto::ProfileParams = serde_json::from_value(req.params.clone())?;
            let resp = s.handle_start_cpu_profile(ctx, &params, req)?;
            s.send_result(req.id.clone(), serde_json::to_value(resp)?)?;
            Ok(None)
        }),
    );
    register_request_handler(
        &mut handlers,
        lsproto::CustomStopCPUProfileInfo.clone(),
        Server::handle_stop_cpu_profile,
    );
    handlers
}

pub fn register_notification_handler<Req, F>(
    handlers: &mut HandlerMap,
    info: lsproto::NotificationInfo<Req>,
    fn_: F,
) where
    Req: DeserializeOwned + Default + 'static,
    F: Fn(
            &mut Server,
            context::Context,
            Req,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync
        + 'static,
{
    handlers.insert(
        info.method.clone(),
        Box::new(
            move |s: &mut Server, ctx: context::Context, req: &lsproto::RequestMessage| {
                if s.session.is_none() && req.method != lsproto::MethodInitialized {
                    return Err(Box::new(SimpleError(
                        lsproto::ErrorCodeServerNotInitialized.to_string(),
                    ))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                let params = if req.params.is_null() {
                    serde_json::from_value(serde_json::Value::Null).unwrap_or_default()
                } else {
                    serde_json::from_value(req.params.clone()).map_err(|err| {
                        Box::new(SimpleError(err.to_string()))
                            as Box<dyn std::error::Error + Send + Sync>
                    })?
                };
                fn_(s, ctx, params)?;
                Ok(None)
            },
        ),
    );
}

pub fn register_request_handler<Req, Resp, F>(
    handlers: &mut HandlerMap,
    info: lsproto::RequestInfo<Req, Resp>,
    fn_: F,
) where
    Req: DeserializeOwned + Default + 'static,
    Resp: Serialize + 'static,
    F: Fn(
            &mut Server,
            context::Context,
            Req,
            &lsproto::RequestMessage,
        ) -> Result<Resp, Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync
        + 'static,
{
    handlers.insert(
        info.method.clone(),
        Box::new(
            move |s: &mut Server, ctx: context::Context, req: &lsproto::RequestMessage| {
                if s.session.is_none() && req.method != lsproto::MethodInitialize {
                    return Err(Box::new(SimpleError(
                        lsproto::ErrorCodeServerNotInitialized.to_string(),
                    ))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                let params = if req.params.is_null() {
                    serde_json::from_value(serde_json::Value::Null).unwrap_or_default()
                } else {
                    serde_json::from_value(req.params.clone()).map_err(|err| {
                        Box::new(SimpleError(err.to_string()))
                            as Box<dyn std::error::Error + Send + Sync>
                    })?
                };
                let resp = fn_(s, ctx.clone(), params, req)?;
                let result = serde_json::to_value(resp).map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                s.send_result(req.id.clone(), result)?;
                Ok(None)
            },
        ),
    );
}

pub fn register_language_service_document_request_handler<Req, Resp>(
    handlers: &mut HandlerMap,
    info: lsproto::RequestInfo<Req, Resp>,
    fn_: fn(
        &mut Server,
        context::Context,
        &ls::LanguageService,
        Req,
    ) -> Result<Resp, Box<dyn std::error::Error + Send + Sync>>,
) where
    Req: DeserializeOwned + lsproto::HasTextDocumentUri + 'static,
    Resp: Serialize + 'static,
{
    handlers.insert(
        info.method.clone(),
        Box::new(
            move |s: &mut Server, ctx: context::Context, req: &lsproto::RequestMessage| {
                let params: Req = serde_json::from_value(req.params.clone()).map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                let language_service = {
                    let session = s.session.as_ref().cloned().ok_or_else(|| {
                        Box::new(SimpleError("server not initialized".to_string()))
                            as Box<dyn std::error::Error + Send + Sync>
                    })?;
                    let language_service = session
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .get_language_service(ctx.clone(), params.text_document_uri())
                        .map_err(|err| {
                            Box::new(SimpleError(err)) as Box<dyn std::error::Error + Send + Sync>
                        })?;
                    language_service
                };
                let req = req.clone();
                let resp = fn_(s, ctx.clone(), &language_service, params)?;
                if let Some(session) = &s.session {
                    session
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .enqueue_publish_global_diagnostics();
                }
                let result = serde_json::to_value(resp).map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                s.send_result(req.id.clone(), result)?;
                Ok(None)
            },
        ),
    );
}

pub fn register_language_service_with_auto_imports_request_handler<Req, Resp>(
    handlers: &mut HandlerMap,
    info: lsproto::RequestInfo<Req, Resp>,
    fn_: fn(
        &mut Server,
        context::Context,
        &ls::LanguageService,
        Req,
    ) -> Result<Resp, Box<dyn std::error::Error + Send + Sync>>,
) where
    Req: Clone + DeserializeOwned + lsproto::HasTextDocumentUri + 'static,
    Resp: Serialize + 'static,
{
    handlers.insert(
        info.method.clone(),
        Box::new(
            move |s: &mut Server, ctx: context::Context, req: &lsproto::RequestMessage| {
                let params: Req = serde_json::from_value(req.params.clone()).map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                let uri = params.text_document_uri();
                let session = s.session.as_ref().cloned().ok_or_else(|| {
                    Box::new(SimpleError("server not initialized".to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                let (language_service, base_snapshot) = {
                    let mut session_guard = session.lock().unwrap_or_else(|err| err.into_inner());
                    session_guard
                        .get_language_service_and_snapshot_handle(ctx.clone(), uri.clone())
                        .map_err(|err| {
                            Box::new(SimpleError(err)) as Box<dyn std::error::Error + Send + Sync>
                        })?
                };

                let response_result = (|| {
                    let resp = fn_(s, ctx.clone(), &language_service, params.clone());
                    if let Err(err) = resp {
                        if !is_needs_auto_imports_error(err.as_ref()) {
                            return Err(err);
                        }
                        let language_service = {
                            let mut session_guard =
                                session.lock().unwrap_or_else(|err| err.into_inner());
                            session_guard
                                .get_language_service_with_auto_imports(
                                    ctx.clone(),
                                    &base_snapshot,
                                    uri.clone(),
                                )
                                .map_err(|err| {
                                    Box::new(SimpleError(err))
                                        as Box<dyn std::error::Error + Send + Sync>
                                })?
                        };
                        if let Some(err) = ctx.err() {
                            return Err(Box::new(SimpleError(err))
                                as Box<dyn std::error::Error + Send + Sync>);
                        }
                        let resp = fn_(s, ctx.clone(), &language_service, params);
                        if let Err(err) = &resp {
                            if is_needs_auto_imports_error(err.as_ref()) {
                                return Err(Box::new(SimpleError(format!(
                                    "{} returned ErrNeedsAutoImports even after enabling auto imports",
                                    info.method
                                )))
                                    as Box<dyn std::error::Error + Send + Sync>);
                            }
                        }
                        return resp;
                    }
                    resp
                })();

                {
                    let mut session_guard = session.lock().unwrap_or_else(|err| err.into_inner());
                    session_guard.release_snapshot_handle(&base_snapshot);
                }

                let resp = response_result?;
                if let Some(err) = ctx.err() {
                    return Err(Box::new(SimpleError(err))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                let result = serde_json::to_value(resp).map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                s.send_result(req.id.clone(), result)?;
                Ok(None)
            },
        ),
    );
}

pub fn register_multi_project_reference_request_handler<Req, Resp>(
    handlers: &mut HandlerMap,
    info: lsproto::RequestInfo<Req, Resp>,
    fn_: fn(
        &ls::LanguageService,
        context::Context,
        Req,
        &dyn ls::CrossProjectOrchestrator,
    ) -> Result<Resp, Box<dyn std::error::Error + Send + Sync>>,
) where
    Req: DeserializeOwned + lsproto::HasTextDocumentPosition + 'static,
    Resp: Serialize + 'static,
{
    handlers.insert(
        info.method.clone(),
        Box::new(
            move |s: &mut Server, ctx: context::Context, req: &lsproto::RequestMessage| {
                let params: Req = serde_json::from_value(req.params.clone()).map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                let (default_ls, orchestrator) = s
                    .get_language_service_and_cross_project_orchestrator(
                        ctx.clone(),
                        params.text_document_uri(),
                        req,
                    )?;
                let resp = fn_(&default_ls, ctx.clone(), params, &orchestrator)?;
                let result = serde_json::to_value(resp).map_err(|err| {
                    Box::new(SimpleError(err.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                s.send_result(req.id.clone(), result)?;
                Ok(None)
            },
        ),
    );
}

pub struct CrossProjectOrchestrator {
    pub(crate) session: Arc<Mutex<project::Session>>,
    pub req: lsproto::RequestMessage,
    pub default_project: Arc<dyn ls::Project>,
    pub all_projects: Vec<Arc<dyn ls::Project>>,
}

impl CrossProjectOrchestrator {
    pub fn get_default_project(&self) -> Arc<dyn ls::Project> {
        self.default_project.clone()
    }
    pub fn get_all_projects_for_initial_request(&self) -> Vec<Arc<dyn ls::Project>> {
        self.all_projects.clone()
    }
    pub fn get_language_service_for_project_with_file(
        &self,
        ctx: &context::Context,
        p: &dyn ls::Project,
        uri: lsproto::DocumentUri,
    ) -> Option<ls::LanguageService<'static>> {
        self.session
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get_language_service_for_project_id_with_file(ctx.clone(), p.id(), uri)
    }
    pub fn get_projects_for_file(
        &self,
        ctx: &context::Context,
        uri: lsproto::DocumentUri,
    ) -> Result<Vec<Arc<dyn ls::Project>>, core::Error> {
        Ok(self
            .session
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get_projects_for_file(ctx.clone(), uri)
            .into_iter()
            .map(|project| Arc::new(project.clone()) as Arc<dyn ls::Project>)
            .collect())
    }
    pub fn get_projects_loading_project_tree(
        &self,
        ctx: &context::Context,
        requested_project_trees: &collections::Set<tspath::Path>,
    ) -> Box<dyn Iterator<Item = Arc<dyn ls::Project>> + '_> {
        let mut projects: Vec<Arc<dyn ls::Project>> = Vec::new();
        projects.extend(
            self.session
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .get_projects_loading_project_tree(
                    ctx.clone(),
                    project::ProjectTreeRequest {
                        referenced_projects: Some(requested_project_trees.clone()),
                    },
                )
                .into_iter()
                .map(|project| Arc::new(project) as Arc<dyn ls::Project>),
        );
        Box::new(projects.into_iter())
    }
}

impl ls::CrossProjectOrchestrator for CrossProjectOrchestrator {
    fn get_default_project(&self) -> Arc<dyn ls::Project> {
        CrossProjectOrchestrator::get_default_project(self)
    }

    fn get_all_projects_for_initial_request(&self) -> Vec<Arc<dyn ls::Project>> {
        CrossProjectOrchestrator::get_all_projects_for_initial_request(self)
    }

    fn get_language_service_for_project_with_file(
        &self,
        ctx: &core::Context,
        project: &dyn ls::Project,
        uri: lsproto::DocumentUri,
    ) -> Option<ls::LanguageService<'static>> {
        CrossProjectOrchestrator::get_language_service_for_project_with_file(
            self, ctx, project, uri,
        )
    }

    fn get_projects_for_file(
        &self,
        ctx: &core::Context,
        uri: lsproto::DocumentUri,
    ) -> Result<Vec<Arc<dyn ls::Project>>, core::Error> {
        CrossProjectOrchestrator::get_projects_for_file(self, ctx, uri)
    }

    fn get_projects_loading_project_tree(
        &self,
        ctx: &core::Context,
        requested_project_trees: &collections::Set<tspath::Path>,
    ) -> Box<dyn Iterator<Item = Arc<dyn ls::Project>> + '_> {
        CrossProjectOrchestrator::get_projects_loading_project_tree(
            self,
            ctx,
            requested_project_trees,
        )
    }
}

pub fn send_client_request<Req, Resp>(
    _ctx: context::Context,
    s: &mut Server,
    info: lsproto::RequestInfo<Req, Resp>,
    params: Req,
) -> Result<Resp, Box<dyn std::error::Error + Send + Sync>>
where
    Req: Serialize,
    Resp: DeserializeOwned,
{
    let seq = s
        .client_seq
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;
    let id = jsonrpc::Id::new_string(format!("ts{seq}"));
    let req = info.new_request_message(Some(id.clone()), params);
    s.pending_server_requests
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .insert(id.clone(), Vec::new());
    s.send(&req.message())?;

    loop {
        {
            let mut pending = s
                .pending_server_requests
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            if let Some(responses) = pending.get_mut(&id) {
                if let Some(resp) = responses.pop() {
                    pending.remove(&id);
                    if let Some(error) = resp.error {
                        return Err(Box::new(SimpleError(format!(
                            "request failed: {}",
                            error.message
                        ))));
                    }
                    return info.unmarshal_result(resp.result).map_err(|err| {
                        Box::new(SimpleError(err)) as Box<dyn std::error::Error + Send + Sync>
                    });
                }
            } else {
                return Err(Box::new(SimpleError("request cancelled".to_string())));
            }
        }

        match s.read() {
            Ok(msg) if msg.kind == jsonrpc::MessageKind::Response => {
                let resp = msg.as_response().clone();
                if resp.id.as_ref() == Some(&id) {
                    s.pending_server_requests
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .remove(&id);
                    if let Some(error) = resp.error {
                        return Err(Box::new(SimpleError(format!(
                            "request failed: {}",
                            error.message
                        ))));
                    }
                    return info.unmarshal_result(resp.result).map_err(|err| {
                        Box::new(SimpleError(err)) as Box<dyn std::error::Error + Send + Sync>
                    });
                }
            }
            Ok(msg) => {
                if msg.kind != jsonrpc::MessageKind::Response {
                    s.request_queue
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .push_back(msg.as_request().clone());
                }
            }
            Err(err) => {
                s.pending_server_requests
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .remove(&id);
                return Err(Box::new(err));
            }
        }
    }
}

pub fn send_client_request_fire_and_forget<Req, Resp>(
    s: &mut Server,
    info: lsproto::RequestInfo<Req, Resp>,
    params: Req,
) -> Result<(), io::Error>
where
    Req: serde::Serialize,
{
    let seq = s
        .client_seq
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;
    let id = jsonrpc::Id::new_string(format!("ts{seq}"));
    s.send(&info.new_request_message(Some(id), params).message())
}

pub fn send_notification<Params>(
    s: &mut Server,
    info: lsproto::NotificationInfo<Params>,
    params: Params,
) -> Result<(), io::Error>
where
    Params: serde::Serialize,
{
    s.send(&info.new_notification_message(params).message())
}
