use std::collections::HashMap;
use std::env;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::{self as project, ata, logging};
use ts_compiler as compiler;
use ts_ls as ls;
use ts_tspath as tspath;
use ts_vfs::Fs;

pub use project::{Session, SessionInit, SessionOptions};

pub const TEST_TYPINGS_LOCATION: &str = "/home/src/Library/Caches/typescript";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypingsInstallerOptions {
    pub types_registry: Vec<String>,
    pub package_to_file: HashMap<String, String>,
}

pub type WatcherId = String;

pub struct LanguageService {
    inner: ls::LanguageService<'static>,
    program: Arc<compiler::Program>,
}

impl LanguageService {
    pub fn new(
        project_path: tspath::Path,
        program: Arc<compiler::Program>,
        host: Box<dyn ls::Host>,
        active_file: &str,
    ) -> Self {
        Self {
            inner: ls::new_language_service(project_path, program.clone(), host, active_file),
            program,
        }
    }

    pub fn get_program(&self) -> &compiler::Program {
        &self.program
    }
}

impl Deref for LanguageService {
    type Target = ls::LanguageService<'static>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for LanguageService {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Message {
    pub text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileSystemWatcher {
    pub glob_pattern: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TelemetryEvent {
    pub name: String,
}

#[derive(Clone)]
pub struct PublishDiagnosticsCall {
    pub ctx: project::Context,
    pub params: project::PublishDiagnosticsParams,
}

#[derive(Clone)]
pub struct ProgressCall {
    pub message: project::DiagnosticsMessage,
    pub args: Vec<String>,
}

#[derive(Clone)]
pub struct WatchFilesCall {
    pub ctx: project::Context,
    pub id: project::WatcherID,
    pub watchers: Vec<project::FileSystemWatcher>,
}

#[derive(Default)]
pub struct ClientMock {
    pub is_active_func: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    pub progress_finish_func:
        Option<Box<dyn Fn(&project::DiagnosticsMessage, &[String]) + Send + Sync>>,
    pub progress_start_func:
        Option<Box<dyn Fn(&project::DiagnosticsMessage, &[String]) + Send + Sync>>,
    pub publish_diagnostics_func:
        Option<Box<dyn Fn(&project::PublishDiagnosticsParams) -> Result<(), String> + Send + Sync>>,
    pub refresh_code_lens_func: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    pub refresh_diagnostics_func: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    pub refresh_inlay_hints_func: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    pub send_telemetry_func:
        Option<Box<dyn Fn(&project::TelemetryEvent) -> Result<(), String> + Send + Sync>>,
    pub unwatch_files_func:
        Option<Box<dyn Fn(&project::WatcherID) -> Result<(), String> + Send + Sync>>,
    pub watch_files_func: Option<
        Box<
            dyn Fn(&project::WatcherID, &[project::FileSystemWatcher]) -> Result<(), String>
                + Send
                + Sync,
        >,
    >,

    is_active_calls: Vec<()>,
    progress_finish_calls: Vec<ProgressCall>,
    progress_start_calls: Vec<ProgressCall>,
    publish_diagnostics_calls: Vec<PublishDiagnosticsCall>,
    refresh_code_lens_calls: Vec<()>,
    refresh_diagnostics_calls: Vec<()>,
    refresh_inlay_hints_calls: Vec<()>,
    send_telemetry_calls: Vec<project::TelemetryEvent>,
    unwatch_files_calls: Vec<project::WatcherID>,
    watch_files_calls: Vec<WatchFilesCall>,
}

impl ClientMock {
    pub fn is_active_calls(&self) -> Vec<()> {
        self.is_active_calls.clone()
    }

    pub fn progress_finish_calls(&self) -> Vec<ProgressCall> {
        self.progress_finish_calls.clone()
    }

    pub fn progress_start_calls(&self) -> Vec<ProgressCall> {
        self.progress_start_calls.clone()
    }

    pub fn publish_diagnostics_calls(&self) -> Vec<PublishDiagnosticsCall> {
        self.publish_diagnostics_calls.clone()
    }

    pub fn refresh_code_lens_calls(&self) -> Vec<()> {
        self.refresh_code_lens_calls.clone()
    }

    pub fn refresh_diagnostics_calls(&self) -> Vec<()> {
        self.refresh_diagnostics_calls.clone()
    }

    pub fn refresh_inlay_hints_calls(&self) -> Vec<()> {
        self.refresh_inlay_hints_calls.clone()
    }

    pub fn send_telemetry_calls(&self) -> Vec<project::TelemetryEvent> {
        self.send_telemetry_calls.clone()
    }

    pub fn unwatch_files_calls(&self) -> Vec<project::WatcherID> {
        self.unwatch_files_calls.clone()
    }

    pub fn watch_files_calls(&self) -> Vec<WatchFilesCall> {
        self.watch_files_calls.clone()
    }
}

impl project::Client for Mutex<ClientMock> {
    fn watch_files(
        &self,
        ctx: &project::Context,
        id: project::WatcherID,
        watchers: Vec<project::FileSystemWatcher>,
    ) -> Result<(), String> {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.watch_files_calls.push(WatchFilesCall {
            ctx: ctx.clone(),
            id: id.clone(),
            watchers: watchers.clone(),
        });
        if let Some(f) = &client.watch_files_func {
            return f(&id, &watchers);
        }
        Ok(())
    }

    fn unwatch_files(&self, _ctx: &project::Context, id: project::WatcherID) -> Result<(), String> {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.unwatch_files_calls.push(id.clone());
        if let Some(f) = &client.unwatch_files_func {
            return f(&id);
        }
        Ok(())
    }

    fn refresh_diagnostics(&self, _ctx: &project::Context) -> Result<(), String> {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.refresh_diagnostics_calls.push(());
        if let Some(f) = &client.refresh_diagnostics_func {
            return f();
        }
        Ok(())
    }

    fn publish_diagnostics(
        &self,
        ctx: &project::Context,
        params: project::PublishDiagnosticsParams,
    ) -> Result<(), String> {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client
            .publish_diagnostics_calls
            .push(PublishDiagnosticsCall {
                ctx: ctx.clone(),
                params: params.clone(),
            });
        if let Some(f) = &client.publish_diagnostics_func {
            return f(&params);
        }
        Ok(())
    }

    fn refresh_inlay_hints(&self, _ctx: &project::Context) -> Result<(), String> {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.refresh_inlay_hints_calls.push(());
        if let Some(f) = &client.refresh_inlay_hints_func {
            return f();
        }
        Ok(())
    }

    fn refresh_code_lens(&self, _ctx: &project::Context) -> Result<(), String> {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.refresh_code_lens_calls.push(());
        if let Some(f) = &client.refresh_code_lens_func {
            return f();
        }
        Ok(())
    }

    fn progress_start(&self, message: &project::DiagnosticsMessage, args: &[String]) {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.progress_start_calls.push(ProgressCall {
            message: message.clone(),
            args: args.to_vec(),
        });
        if let Some(f) = &client.progress_start_func {
            f(message, args);
        }
    }

    fn progress_finish(&self, message: &project::DiagnosticsMessage, args: &[String]) {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.progress_finish_calls.push(ProgressCall {
            message: message.clone(),
            args: args.to_vec(),
        });
        if let Some(f) = &client.progress_finish_func {
            f(message, args);
        }
    }

    fn send_telemetry(
        &self,
        _ctx: &project::Context,
        telemetry: project::TelemetryEvent,
    ) -> Result<(), String> {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.send_telemetry_calls.push(telemetry.clone());
        if let Some(f) = &client.send_telemetry_func {
            return f(&telemetry);
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        let mut client = self.lock().expect("client mock mutex poisoned");
        client.is_active_calls.push(());
        client.is_active_func.as_ref().is_some_and(|f| f())
    }
}

pub type NpmInstallFunc = Box<dyn Fn(&str, &[String]) -> Result<Vec<u8>, String> + Send + Sync>;

#[derive(Default)]
pub struct NpmExecutorMock {
    pub npm_install_func: Option<NpmInstallFunc>,
    npm_install_calls: Vec<NpmInstallCall>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NpmInstallCall {
    pub cwd: String,
    pub args: Vec<String>,
}

impl NpmExecutorMock {
    pub fn npm_install(&mut self, cwd: &str, args: &[String]) -> Result<Vec<u8>, String> {
        self.npm_install_calls.push(NpmInstallCall {
            cwd: cwd.to_string(),
            args: args.to_vec(),
        });
        if let Some(f) = &self.npm_install_func {
            return f(cwd, args);
        }
        Ok(Vec::new())
    }

    pub fn npm_install_calls(&self) -> Vec<NpmInstallCall> {
        self.npm_install_calls.clone()
    }
}

impl ata::NpmExecutor for Arc<Mutex<NpmExecutorMock>> {
    fn npm_install(&self, cwd: &str, args: &[String]) -> Result<Vec<u8>, String> {
        let mut mock = self.lock().unwrap_or_else(|err| err.into_inner());
        mock.npm_install_calls.push(NpmInstallCall {
            cwd: cwd.to_string(),
            args: args.to_vec(),
        });
        if let Some(f) = &mock.npm_install_func {
            return f(cwd, args);
        }
        Ok(Vec::new())
    }
}

pub struct SessionUtils {
    pub current_directory: String,
    pub fs_from_file_map: Option<ts_vfs::vfstest::MapFs>,
    pub fs: Arc<dyn Fs + Send + Sync>,
    pub client: Arc<Mutex<ClientMock>>,
    pub npm_executor: Arc<Mutex<NpmExecutorMock>>,
    pub ti_options: Option<TypingsInstallerOptions>,
    pub logger: Box<dyn logging::LogCollector>,
}

impl SessionUtils {
    pub fn fs_from_file_map(&self) -> Option<&ts_vfs::vfstest::MapFs> {
        self.fs_from_file_map.as_ref()
    }

    pub fn client(&self) -> MutexGuard<'_, ClientMock> {
        self.client.lock().unwrap_or_else(|err| err.into_inner())
    }

    pub fn npm_executor(&self) -> MutexGuard<'_, NpmExecutorMock> {
        self.npm_executor
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub fn setup_npm_executor_for_typings_installer(&mut self) {
        let Some(ti_options) = self.ti_options.clone() else {
            return;
        };
        let fs = Arc::clone(&self.fs);
        self.npm_executor().npm_install_func = Some(Box::new(move |cwd, npm_install_args| {
            if npm_install_args.len() < 3 {
                return Err(format!(
                    "unexpected npm install: {cwd} {npm_install_args:?}"
                ));
            }
            if npm_install_args.len() == 3 && npm_install_args[2] == "types-registry@latest" {
                fs.write_file(
                    &format!("{cwd}/node_modules/types-registry/index.json"),
                    &create_types_registry_file_content_for(&ti_options),
                )
                .map_err(|err| err.to_string())?;
                return Ok(Vec::new());
            }
            let package_end = npm_install_args
                .iter()
                .enumerate()
                .skip(2)
                .find_map(|(index, arg)| arg.starts_with("--").then_some(index))
                .unwrap_or(npm_install_args.len());
            for package in &npm_install_args[2..package_end] {
                let package = strip_at_types_version(package);
                let package_base_name = &package["@types/".len()..];
                let Some(content) = ti_options.package_to_file.get(package_base_name) else {
                    return Err(format!("content not provided for {package_base_name}"));
                };
                fs.write_file(
                    &format!("{cwd}/node_modules/@types/{package_base_name}/index.d.ts"),
                    content,
                )
                .map_err(|err| err.to_string())?;
            }
            Ok(Vec::new())
        }));
    }

    pub fn to_path(&self, file_name: &str) -> String {
        ts_tspath::to_path(
            file_name,
            &self.current_directory,
            self.fs.use_case_sensitive_file_names(),
        )
    }

    pub fn fs(&self) -> Arc<dyn Fs + Send + Sync> {
        Arc::clone(&self.fs)
    }

    pub fn watches_file(&self, file_path: &str) -> bool {
        self.client().watch_files_calls().into_iter().any(|call| {
            call.watchers
                .into_iter()
                .any(|watcher| glob_matches(&watcher.glob_pattern, file_path))
        })
    }

    pub fn logs(&self) -> String {
        self.logger.to_string()
    }

    pub fn baseline_logs(&self, test_name: &str) {
        ts_testutil::baseline::run(
            &format!("{test_name}.log"),
            &self.logs(),
            ts_testutil::baseline::Options {
                subfolder: "project".to_owned(),
                ..ts_testutil::baseline::Options::default()
            },
        )
        .unwrap_or_else(|err| panic!("failed to baseline project logs for {test_name}: {err}"));
    }

    pub fn create_types_registry_file_content(&self) -> String {
        create_types_registry_file_content_for(self.ti_options.as_ref().unwrap())
    }

    pub fn append_types_registry_config(&self, builder: &mut String, index: usize, entry: &str) {
        append_types_registry_config(builder, index, entry);
    }
}

pub fn setup<F>(files: HashMap<String, F>) -> (Session, SessionUtils)
where
    F: ts_vfs::vfstest::IntoMapFile,
{
    setup_with_typings_installer(files, TypingsInstallerOptions::default())
}

pub fn setup_with_real_fs() -> (Session, SessionUtils) {
    let wd = env::current_dir()
        .unwrap_or_else(|err| panic!("failed to get current directory: {err}"))
        .to_string_lossy()
        .into_owned();
    let fs: Arc<dyn Fs + Send + Sync> = Arc::new(ts_bundled::wrap_fs(ts_vfs::osvfs::os::fs()));
    let client: Arc<Mutex<ClientMock>> = Arc::new(Mutex::new(ClientMock::default()));
    let npm_executor = Arc::new(Mutex::new(NpmExecutorMock::default()));
    let session_utils = SessionUtils {
        current_directory: wd.clone(),
        fs_from_file_map: None,
        fs: Arc::clone(&fs),
        client: Arc::clone(&client),
        npm_executor: Arc::clone(&npm_executor),
        ti_options: None,
        logger: Box::new(logging::new_test_logger()),
    };
    let init = SessionInit {
        background_ctx: ts_core::Context::default(),
        options: SessionOptions {
            current_directory: wd,
            default_library_path: bundled_lib_path(),
            typings_location: String::new(),
            position_encoding: ts_lsproto::PositionEncodingKind::UTF8,
            watch_enabled: true,
            logging_enabled: true,
            telemetry_enabled: false,
            push_diagnostics_enabled: true,
            debounce_delay: Duration::default(),
            locale: ts_locale::Locale::default(),
        },
        fs,
        client: Some(client),
        logger: Arc::new(logging::new_test_logger()),
        npm_executor: Some(Box::new(npm_executor)),
        parse_cache: None,
    };
    (project::new_session(init), session_utils)
}

pub fn setup_with_options(
    files: HashMap<String, impl ts_vfs::vfstest::IntoMapFile>,
    options: SessionOptions,
) -> (Session, SessionUtils) {
    setup_with_options_and_typings_installer(
        files,
        Some(options),
        Some(TypingsInstallerOptions::default()),
    )
}

pub fn setup_map_files_with_options(
    files: HashMap<String, ts_vfs::vfstest::MapFile>,
    options: SessionOptions,
) -> (Session, SessionUtils) {
    let fs_from_file_map = ts_vfs::vfstest::from_map(files, false);
    let fs: Arc<dyn Fs + Send + Sync> = Arc::new(ts_bundled::wrap_fs(fs_from_file_map.clone()));
    let client: Arc<Mutex<ClientMock>> = Arc::new(Mutex::new(ClientMock::default()));
    let npm_executor = Arc::new(Mutex::new(NpmExecutorMock::default()));
    let session_utils = SessionUtils {
        current_directory: options.current_directory.clone(),
        fs_from_file_map: Some(fs_from_file_map),
        fs: Arc::clone(&fs),
        client: Arc::clone(&client),
        npm_executor: Arc::clone(&npm_executor),
        ti_options: Some(TypingsInstallerOptions::default()),
        logger: Box::new(logging::new_test_logger()),
    };
    (
        project::new_session(SessionInit {
            background_ctx: ts_core::Context::default(),
            options,
            fs,
            client: Some(client),
            logger: Arc::new(logging::new_test_logger()),
            npm_executor: Some(Box::new(npm_executor)),
            parse_cache: None,
        }),
        session_utils,
    )
}

pub fn setup_with_typings_installer(
    files: HashMap<String, impl ts_vfs::vfstest::IntoMapFile>,
    ti_options: TypingsInstallerOptions,
) -> (Session, SessionUtils) {
    setup_with_options_and_typings_installer(files, None, Some(ti_options))
}

pub fn setup_with_options_and_typings_installer(
    files: HashMap<String, impl ts_vfs::vfstest::IntoMapFile>,
    options: Option<SessionOptions>,
    ti_options: Option<TypingsInstallerOptions>,
) -> (Session, SessionUtils) {
    let (init, session_utils) = get_session_init_options(files, options, ti_options);
    (project::new_session(init), session_utils)
}

pub fn with_request_id(ctx: ts_core::Context) -> ts_core::Context {
    ts_core::with_request_id(ctx, "0".to_string())
}

pub fn get_session_init_options(
    files: HashMap<String, impl ts_vfs::vfstest::IntoMapFile>,
    options: Option<SessionOptions>,
    ti_options: Option<TypingsInstallerOptions>,
) -> (SessionInit, SessionUtils) {
    let fs_from_file_map = ts_vfs::vfstest::from_map(files, false);
    let fs: Arc<dyn Fs + Send + Sync> = Arc::new(ts_bundled::wrap_fs(fs_from_file_map.clone()));
    let client: Arc<Mutex<ClientMock>> = Arc::new(Mutex::new(ClientMock::default()));
    let npm_executor = Arc::new(Mutex::new(NpmExecutorMock::default()));
    let mut session_utils = SessionUtils {
        current_directory: "/".to_string(),
        fs_from_file_map: Some(fs_from_file_map),
        fs: Arc::clone(&fs),
        client: Arc::clone(&client),
        npm_executor: Arc::clone(&npm_executor),
        ti_options,
        logger: Box::new(logging::new_test_logger()),
    };
    session_utils.setup_npm_executor_for_typings_installer();
    let options = options.unwrap_or_else(default_options);
    (
        SessionInit {
            background_ctx: ts_core::Context::default(),
            options,
            fs,
            client: Some(client),
            logger: Arc::new(logging::new_test_logger()),
            npm_executor: Some(Box::new(npm_executor)),
            parse_cache: None,
        },
        session_utils,
    )
}

pub fn types_registry_config_text() -> String {
    let mut result = String::new();
    for (key, value) in types_registry_config() {
        if !result.is_empty() {
            result.push(',');
        }
        result.push_str(&format!("\n      \"{key}\": \"{value}\""));
    }
    result
}

pub fn types_registry_config() -> HashMap<String, String> {
    [
        ("latest", "1.3.0"),
        ("ts2.0", "1.0.0"),
        ("ts2.1", "1.0.0"),
        ("ts2.2", "1.2.0"),
        ("ts2.3", "1.3.0"),
        ("ts2.4", "1.3.0"),
        ("ts2.5", "1.3.0"),
        ("ts2.6", "1.3.0"),
        ("ts2.7", "1.3.0"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect()
}

pub fn append_types_registry_config(builder: &mut String, index: usize, entry: &str) {
    if index > 0 {
        builder.push(',');
    }
    builder.push_str(&format!(
        "\n    \"{entry}\": {{{}\n    }}",
        types_registry_config_text()
    ));
}

fn create_types_registry_file_content_for(ti_options: &TypingsInstallerOptions) -> String {
    let mut builder = String::from("{\n  \"entries\": {");
    for (index, entry) in ti_options.types_registry.iter().enumerate() {
        append_types_registry_config(&mut builder, index, entry);
    }
    let mut index = ti_options.types_registry.len();
    for key in ti_options.package_to_file.keys() {
        if !ti_options.types_registry.contains(key) {
            append_types_registry_config(&mut builder, index, key);
            index += 1;
        }
    }
    builder.push_str("\n  }\n}");
    builder
}

fn default_options() -> SessionOptions {
    SessionOptions {
        current_directory: "/".to_string(),
        default_library_path: bundled_lib_path(),
        typings_location: TEST_TYPINGS_LOCATION.to_string(),
        position_encoding: ts_lsproto::PositionEncodingKind::UTF8,
        watch_enabled: true,
        logging_enabled: true,
        telemetry_enabled: false,
        push_diagnostics_enabled: true,
        debounce_delay: Duration::default(),
        locale: ts_locale::Locale::default(),
    }
}

fn bundled_lib_path() -> String {
    ts_bundled::lib_path()
}

fn strip_at_types_version(package: &str) -> &str {
    if let Some(version_index) = package.rfind('@') {
        if version_index > 6 {
            return &package[..version_index];
        }
    }
    package
}

fn glob_matches(pattern: &ts_lsproto::GlobPattern, file_path: &str) -> bool {
    let pattern = match pattern {
        ts_lsproto::GlobPattern::String(pattern) => pattern,
        ts_lsproto::GlobPattern::Relative(relative) => &relative.pattern,
    };
    ts_glob::parse(pattern)
        .map(|pattern| pattern.match_input(file_path))
        .unwrap_or(false)
}
