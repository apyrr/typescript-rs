use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use super::{
    ClientMock, FileSystemWatcher, Message, NpmExecutorMock, PublishDiagnosticsParams,
    TelemetryEvent,
};
use crate::baseline;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_project as project;
use ts_vfs::Fs;

pub use project::{Session, SessionInit, SessionOptions};

pub const TEST_TYPINGS_LOCATION: &str = "/home/src/Library/Caches/typescript";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypingsInstallerOptions {
    pub types_registry: Vec<String>,
    pub package_to_file: HashMap<String, String>,
}

pub struct SessionUtils {
    pub current_directory: String,
    pub fs_from_file_map: Option<ts_vfs::vfstest::MapFs>,
    pub fs: Arc<dyn Fs + Send + Sync>,
    pub client: Arc<Mutex<ClientMock>>,
    pub npm_executor: Arc<Mutex<NpmExecutorMock>>,
    pub ti_options: Option<TypingsInstallerOptions>,
    pub logger: Box<dyn project::LogCollector>,
}

struct ClientMockHandle {
    client: Arc<Mutex<ClientMock>>,
}

impl project::Client for ClientMockHandle {
    fn watch_files(
        &self,
        _ctx: &project::Context,
        _id: project::WatcherID,
        watchers: Vec<project::FileSystemWatcher>,
    ) -> Result<(), String> {
        let watchers = watchers
            .iter()
            .map(file_system_watcher_to_test_watcher)
            .collect::<Vec<_>>();
        self.client().watch_files(&_id.0, &watchers)
    }

    fn unwatch_files(&self, _ctx: &project::Context, id: project::WatcherID) -> Result<(), String> {
        self.client().unwatch_files(&id.0)
    }

    fn refresh_diagnostics(&self, _ctx: &project::Context) -> Result<(), String> {
        self.client().refresh_diagnostics()
    }

    fn publish_diagnostics(
        &self,
        _ctx: &project::Context,
        params: project::PublishDiagnosticsParams,
    ) -> Result<(), String> {
        self.client()
            .publish_diagnostics(&PublishDiagnosticsParams { uri: params.uri })
    }

    fn refresh_inlay_hints(&self, _ctx: &project::Context) -> Result<(), String> {
        self.client().refresh_inlay_hints()
    }

    fn refresh_code_lens(&self, _ctx: &project::Context) -> Result<(), String> {
        self.client().refresh_code_lens()
    }

    fn progress_start(&self, message: &project::DiagnosticsMessage, args: &[String]) {
        self.client().progress_start(
            &Message {
                text: message.to_string(),
            },
            args,
        );
    }

    fn progress_finish(&self, message: &project::DiagnosticsMessage, args: &[String]) {
        self.client().progress_finish(
            &Message {
                text: message.to_string(),
            },
            args,
        );
    }

    fn send_telemetry(
        &self,
        _ctx: &project::Context,
        telemetry: project::TelemetryEvent,
    ) -> Result<(), String> {
        self.client().send_telemetry(&TelemetryEvent {
            name: telemetry_event_name(&telemetry).to_string(),
        })
    }

    fn is_active(&self) -> bool {
        self.client().is_active()
    }
}

impl ClientMockHandle {
    fn client(&self) -> MutexGuard<'_, ClientMock> {
        self.client.lock().unwrap_or_else(|err| err.into_inner())
    }
}

struct NpmExecutorMockHandle {
    npm_executor: Arc<Mutex<NpmExecutorMock>>,
}

impl project::NpmExecutor for NpmExecutorMockHandle {
    fn npm_install(&self, cwd: &str, args: &[String]) -> Result<Vec<u8>, String> {
        self.npm_executor().npm_install(cwd, args)
    }
}

impl NpmExecutorMockHandle {
    fn npm_executor(&self) -> MutexGuard<'_, NpmExecutorMock> {
        self.npm_executor
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }
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

    /// WatchesFile reports whether any registered file watcher would match the given
    /// file path. It handles both absolute glob patterns and relative patterns with
    /// a base URI. On case-insensitive file systems the paths in glob patterns are
    /// lowercased, so callers should pass the lowercased path.
    pub fn watches_file(&self, file_path: &str) -> bool {
        self.client()
            .watch_files_calls()
            .into_iter()
            .any(|(_, watchers)| {
                watchers
                    .into_iter()
                    .any(|watcher| glob_matches(&watcher.glob_pattern, file_path))
            })
    }

    pub fn logs(&self) -> String {
        self.logger.to_string()
    }

    pub fn baseline_logs(&self, test_name: &str) {
        baseline::run(
            &format!("{test_name}.log"),
            &self.logs(),
            baseline::Options {
                subfolder: "project".to_owned(),
                ..baseline::Options::default()
            },
        )
        .unwrap_or_else(|err| panic!("failed to baseline project logs for {test_name}: {err}"));
    }

    pub fn create_types_registry_file_content(&self) -> String {
        let ti_options = self.ti_options.as_ref().unwrap();
        create_types_registry_file_content_for(ti_options)
    }

    pub fn append_types_registry_config(&self, builder: &mut String, index: usize, entry: &str) {
        append_types_registry_config(builder, index, entry);
    }
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

pub fn types_registry_config_text() -> String {
    static TYPES_REGISTRY_CONFIG_TEXT: OnceLock<String> = OnceLock::new();
    TYPES_REGISTRY_CONFIG_TEXT
        .get_or_init(|| {
            let mut result = String::new();
            for (key, value) in types_registry_config() {
                if !result.is_empty() {
                    result.push(',');
                }
                result.push_str(&format!("\n      \"{key}\": \"{value}\""));
            }
            result
        })
        .clone()
}

pub fn types_registry_config() -> HashMap<String, String> {
    static TYPES_REGISTRY_CONFIG: OnceLock<HashMap<String, String>> = OnceLock::new();
    TYPES_REGISTRY_CONFIG
        .get_or_init(|| {
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
        })
        .clone()
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

pub fn setup(files: HashMap<String, String>) -> (Session, SessionUtils) {
    setup_with_typings_installer(files, TypingsInstallerOptions::default())
}

pub fn setup_with_real_fs() -> (Session, SessionUtils) {
    let wd = env::current_dir()
        .unwrap_or_else(|err| panic!("failed to get current directory: {err}"))
        .to_string_lossy()
        .into_owned();
    let fs: Arc<dyn Fs + Send + Sync> = Arc::new(ts_bundled::wrap_fs(ts_vfs::osvfs::os::fs()));
    let client = Arc::new(Mutex::new(ClientMock::default()));
    let npm_executor = Arc::new(Mutex::new(NpmExecutorMock::default()));
    let session_utils = SessionUtils {
        current_directory: wd.clone(),
        fs_from_file_map: None,
        fs: Arc::clone(&fs),
        client: Arc::clone(&client),
        npm_executor: Arc::clone(&npm_executor),
        ti_options: None,
        logger: Box::new(project::new_test_logger()),
    };
    let init = SessionInit {
        background_ctx: ts_core::Context::default(),
        options: SessionOptions {
            current_directory: wd,
            default_library_path: bundled_lib_path(),
            typings_location: String::new(),
            position_encoding: lsproto::PositionEncodingKind::UTF8,
            watch_enabled: true,
            logging_enabled: true,
            telemetry_enabled: false,
            push_diagnostics_enabled: true,
            debounce_delay: Duration::default(),
            locale: ts_locale::Locale::default(),
        },
        fs,
        client: Some(Arc::new(ClientMockHandle {
            client: Arc::clone(&client),
        })),
        logger: Arc::new(project::new_test_logger()),
        npm_executor: Some(Box::new(NpmExecutorMockHandle {
            npm_executor: Arc::clone(&npm_executor),
        })),
        parse_cache: None,
    };
    (project::new_session(init), session_utils)
}

pub fn setup_with_options(
    files: HashMap<String, String>,
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
    let client = Arc::new(Mutex::new(ClientMock::default()));
    let npm_executor = Arc::new(Mutex::new(NpmExecutorMock::default()));
    let session_utils = SessionUtils {
        current_directory: options.current_directory.clone(),
        fs_from_file_map: Some(fs_from_file_map),
        fs: Arc::clone(&fs),
        client: Arc::clone(&client),
        npm_executor: Arc::clone(&npm_executor),
        ti_options: Some(TypingsInstallerOptions::default()),
        logger: Box::new(project::new_test_logger()),
    };
    (
        project::new_session(SessionInit {
            background_ctx: ts_core::Context::default(),
            options,
            fs,
            client: Some(Arc::new(ClientMockHandle {
                client: Arc::clone(&client),
            })),
            logger: Arc::new(project::new_test_logger()),
            npm_executor: Some(Box::new(NpmExecutorMockHandle {
                npm_executor: Arc::clone(&npm_executor),
            })),
            parse_cache: None,
        }),
        session_utils,
    )
}

pub fn setup_with_typings_installer(
    files: HashMap<String, String>,
    ti_options: TypingsInstallerOptions,
) -> (Session, SessionUtils) {
    setup_with_options_and_typings_installer(files, None, Some(ti_options))
}

pub fn setup_with_options_and_typings_installer(
    files: HashMap<String, String>,
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
    files: HashMap<String, String>,
    options: Option<SessionOptions>,
    ti_options: Option<TypingsInstallerOptions>,
) -> (SessionInit, SessionUtils) {
    let fs_from_file_map = ts_vfs::vfstest::from_map(files, false);
    let fs: Arc<dyn Fs + Send + Sync> = Arc::new(ts_bundled::wrap_fs(fs_from_file_map.clone()));
    let client = Arc::new(Mutex::new(ClientMock::default()));
    let npm_executor = Arc::new(Mutex::new(NpmExecutorMock::default()));
    let mut session_utils = SessionUtils {
        current_directory: "/".to_string(),
        fs_from_file_map: Some(fs_from_file_map),
        fs: Arc::clone(&fs),
        client: Arc::clone(&client),
        npm_executor: Arc::clone(&npm_executor),
        ti_options,
        logger: Box::new(project::new_test_logger()),
    };
    session_utils.setup_npm_executor_for_typings_installer();
    let options = options.unwrap_or_else(default_options);
    (
        SessionInit {
            background_ctx: ts_core::Context::default(),
            options,
            fs,
            client: Some(Arc::new(ClientMockHandle {
                client: Arc::clone(&client),
            })),
            logger: Arc::new(project::new_test_logger()),
            npm_executor: Some(Box::new(NpmExecutorMockHandle {
                npm_executor: Arc::clone(&npm_executor),
            })),
            parse_cache: None,
        },
        session_utils,
    )
}

fn default_options() -> SessionOptions {
    SessionOptions {
        current_directory: "/".to_string(),
        default_library_path: bundled_lib_path(),
        typings_location: TEST_TYPINGS_LOCATION.to_string(),
        position_encoding: lsproto::PositionEncodingKind::UTF8,
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

fn file_system_watcher_to_test_watcher(watcher: &project::FileSystemWatcher) -> FileSystemWatcher {
    FileSystemWatcher {
        glob_pattern: match &watcher.glob_pattern {
            lsproto::GlobPattern::String(pattern) => pattern.clone(),
            lsproto::GlobPattern::Relative(relative_pattern) => {
                let base_uri = match &relative_pattern.base_uri {
                    lsproto::OneOf::Left(workspace_folder) => workspace_folder.uri.as_str(),
                    lsproto::OneOf::Right(uri) => uri.as_str(),
                }
                .to_string();
                format!(
                    "{}{}",
                    ts_tspath::ensure_trailing_directory_separator(&base_uri.file_name()),
                    relative_pattern.pattern
                )
            }
        },
    }
}

fn telemetry_event_name(telemetry: &project::TelemetryEvent) -> &'static str {
    if telemetry.request_failure_telemetry_event.is_some() {
        "languageServer.errorResponse"
    } else if telemetry.performance_stats_telemetry_event.is_some() {
        "languageServer.performanceStats"
    } else if telemetry.project_info_telemetry_event.is_some() {
        "languageServer.projectInfo"
    } else {
        ""
    }
}

fn glob_matches(pattern: &str, file_path: &str) -> bool {
    ts_glob::parse(pattern)
        .map(|glob| glob.match_input(file_path))
        .unwrap_or(false)
}
