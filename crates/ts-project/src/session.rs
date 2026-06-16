use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ts_ast as ast;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_locale as locale;
use ts_ls as ls;
use ts_ls as lsconv;
use ts_ls as lsutil;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::ata;
use crate::background;
use crate::client::{self, ClientArcExt};
use crate::diagnostics;
use crate::logging::{self, Logger};
use crate::tsoptions;
use crate::watch::file_system_watcher_glob_string;
use crate::{
    Client, DocumentUri, ExtendedConfigCache, FileChange, FileChangeKind, FileChangeSummary,
    LanguageKind, Overlay, OverlayFs, ParseCache, PatternsAndIgnored, ProgramCounter, Project,
    ProjectInfo, ProjectTreeRequest, RefCountCacheOptions, ResourceRequest, Snapshot,
    SnapshotChange, SnapshotFs, SnapshotHandle, TextDocumentContentChangePartialOrWholeDocument,
    WatchRegistry, WatchedFiles, WatcherId, get_recursive_glob_pattern, new_extended_config_cache,
    new_overlay_fs, new_parse_cache, new_snapshot, new_watch_registry,
};

#[cfg(not(test))]
type SessionLanguageService = ls::LanguageService<'static>;

#[cfg(test)]
type SessionLanguageService = crate::projecttestutil::LanguageService;

fn new_session_language_service(
    project_path: tspath::Path,
    program: Arc<compiler::Program>,
    host: Box<dyn ls::Host>,
    active_file: &str,
) -> SessionLanguageService {
    #[cfg(not(test))]
    {
        ls::new_language_service(project_path, program, host, active_file)
    }
    #[cfg(test)]
    {
        crate::projecttestutil::LanguageService::new(project_path, program, host, active_file)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum UpdateReason {
    Unknown = 0,
    DidOpenFile = 1,
    DidChangeCompilerOptionsForInferredProjects = 2,
    RequestedLanguageServicePendingChanges = 3,
    RequestedLanguageServiceProjectNotLoaded = 4,
    RequestedLanguageServiceForFileNotOpen = 5,
    RequestedLanguageServiceProjectDirty = 6,
    RequestedLoadProjectTree = 7,
    RequestedLanguageServiceWithAutoImports = 8,
    IdleCleanDiskCache = 9,
}

impl Default for UpdateReason {
    fn default() -> Self {
        Self::Unknown
    }
}

pub const WATCH_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
pub const IDLE_CACHE_CLEAN_DELAY: Duration = Duration::from_secs(30);
pub const PERFORMANCE_TELEMETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);

// SessionOptions are the immutable initialization options for a session.
// Snapshots may reference them since they never change.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    pub current_directory: String,
    pub default_library_path: String,
    pub typings_location: String,
    pub position_encoding: lsproto::PositionEncodingKind,
    pub watch_enabled: bool,
    pub logging_enabled: bool,
    pub telemetry_enabled: bool,
    pub push_diagnostics_enabled: bool,
    pub debounce_delay: Duration,
    pub locale: locale::Locale,
}

pub struct SessionInit {
    pub background_ctx: core::Context,
    pub options: SessionOptions,
    pub fs: Arc<dyn vfs::Fs + Send + Sync>,
    pub client: Option<Arc<dyn Client>>,
    pub logger: Arc<dyn Logger + Send + Sync>,
    pub npm_executor: Option<Box<dyn ata::NpmExecutor>>,
    pub parse_cache: Option<ParseCache>,
}

// Session manages the state of an LSP session. It receives textDocument events
// and requests for LanguageService objects from the LSP server and processes
// them into immutable snapshots as the data source for LanguageServices.
pub struct Session {
    pub(crate) background_ctx: core::Context,
    pub(crate) options: SessionOptions,
    pub(crate) start_time: Instant,
    pub(crate) to_path_current_directory: String,
    pub(crate) client: Option<Arc<dyn Client>>,
    pub(crate) logger: Arc<dyn Logger + Send + Sync>,
    pub(crate) npm_executor: Option<Box<dyn ata::NpmExecutor>>,
    pub(crate) fs: OverlayFs,
    pub(crate) parse_cache: ParseCache,
    pub(crate) extended_config_cache: ExtendedConfigCache,
    pub(crate) program_counter: ProgramCounter,
    pub(crate) initial_user_preferences: lsutil::UserPreferences,
    pub(crate) workspace_user_preferences: lsutil::UserPreferences,
    pub(crate) compiler_options_for_inferred_projects: Option<core::CompilerOptions>,
    pub(crate) typings_installer: Option<ata::TypingsInstaller>,
    pub(crate) background_queue: background::Queue,
    pub(crate) snapshot_id: AtomicU64,
    pub(crate) snapshot: Snapshot,
    pub(crate) pending_user_config_changes: bool,
    pub(crate) pending_file_changes: Vec<FileChange>,
    pub(crate) pending_ata_changes: HashMap<tspath::Path, crate::AtaStateChange>,
    pub(crate) diagnostics_refresh_cancel: Option<core::CancelFunc>,
    pub(crate) warm_auto_import_cancel: Option<core::CancelFunc>,
    pub(crate) idle_cache_clean_timer: Option<core::Timer>,
    pub(crate) idle_cache_clean_due: Arc<AtomicBool>,
    pub(crate) performance_telemetry_cancel: Option<core::CancelFunc>,
    pub(crate) seen_projects: HashMap<tspath::Path, bool>,
    pub(crate) watches: WatchRegistry,
    pub(crate) global_diag_publish_pending: AtomicBool,
}

pub(crate) struct SessionHandle<'a> {
    session: &'a mut Session,
}

impl std::ops::Deref for SessionHandle<'_> {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        self.session
    }
}

impl std::ops::DerefMut for SessionHandle<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.session
    }
}

pub fn new_session(mut init: SessionInit) -> Session {
    let current_directory = init.options.current_directory.clone();
    let use_case_sensitive_file_names = init.fs.use_case_sensitive_file_names();
    let to_path_current_directory = current_directory.clone();
    let snapshot_to_path = Arc::new(move |file_name: &str| {
        tspath::to_path(
            &file_name,
            &current_directory,
            use_case_sensitive_file_names,
        )
    });
    let overlay_current_directory = to_path_current_directory.clone();
    let overlay_to_path = move |file_name: String| {
        tspath::to_path(
            &file_name,
            &overlay_current_directory,
            use_case_sensitive_file_names,
        )
    };
    let overlay_fs = new_overlay_fs(
        init.fs.clone(),
        HashMap::new(),
        init.options.position_encoding,
        overlay_to_path,
    );
    let snapshot_fs = Arc::new(SnapshotFs {
        to_path: snapshot_to_path,
        fs: init.fs,
        overlays: HashMap::new(),
        overlay_directories: HashMap::new(),
        disk_files: Default::default(),
        disk_directories: Default::default(),
        read_files: Default::default(),
        node_modules_realpath_aliases: Default::default(),
    });
    let default_preferences = lsutil::new_default_user_preferences();
    let auto_imports_watch = crate::new_watched_files(
        "auto-import".to_string(),
        lsproto::WatchKind::Create | lsproto::WatchKind::Change | lsproto::WatchKind::Delete,
        false,
        |node_modules_dirs: HashMap<tspath::Path, String>| {
            let mut patterns = Vec::with_capacity(node_modules_dirs.len());
            for dir in node_modules_dirs.values() {
                patterns.push(get_recursive_glob_pattern(dir));
            }
            patterns.sort();
            PatternsAndIgnored {
                patterns_inside_workspace: patterns,
                ..Default::default()
            }
        },
    );
    let snapshot = new_snapshot(
        0,
        snapshot_fs,
        init.options.clone(),
        Some(Default::default()),
        None,
        default_preferences.clone(),
        None,
        Some(auto_imports_watch),
        to_path_current_directory.clone(),
    );
    let typings_installer =
        if !init.options.typings_location.is_empty() && init.npm_executor.is_some() {
            Some(ata::new_typings_installer(
                ata::TypingsInstallerOptions {
                    typings_location: init.options.typings_location.clone(),
                    throttle_limit: 5,
                },
                init.npm_executor.take().unwrap(),
            ))
        } else {
            None
        };
    Session {
        background_ctx: init.background_ctx,
        to_path_current_directory: init.options.current_directory.clone(),
        options: init.options,
        start_time: Instant::now(),
        client: init.client,
        logger: init.logger,
        npm_executor: init.npm_executor,
        fs: overlay_fs,
        parse_cache: init
            .parse_cache
            .unwrap_or_else(|| new_parse_cache(RefCountCacheOptions::default())),
        extended_config_cache: new_extended_config_cache(),
        program_counter: ProgramCounter::default(),
        initial_user_preferences: default_preferences.clone(),
        workspace_user_preferences: default_preferences,
        compiler_options_for_inferred_projects: None,
        typings_installer,
        background_queue: background::new_queue(),
        snapshot_id: AtomicU64::new(0),
        snapshot,
        pending_user_config_changes: false,
        pending_file_changes: Vec::new(),
        pending_ata_changes: HashMap::new(),
        diagnostics_refresh_cancel: None,
        warm_auto_import_cancel: None,
        idle_cache_clean_timer: None,
        idle_cache_clean_due: Arc::new(AtomicBool::new(false)),
        performance_telemetry_cancel: None,
        seen_projects: HashMap::new(),
        watches: new_watch_registry(),
        global_diag_publish_pending: AtomicBool::new(false),
    }
}

impl Session {
    pub(crate) fn clone_handle(&mut self) -> SessionHandle<'_> {
        SessionHandle { session: self }
    }

    pub fn to_path(&self, file_name: &str) -> tspath::Path {
        tspath::to_path(
            file_name,
            &self.to_path_current_directory,
            self.fs.fs.use_case_sensitive_file_names(),
        )
    }

    // FS implements module.ResolutionHost.
    pub fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync> {
        self.fs.fs.clone()
    }

    // GetCurrentDirectory implements module.ResolutionHost.
    pub fn get_current_directory(&self) -> &str {
        &self.options.current_directory
    }

    // Gets copy of current configuration.
    pub fn config(&self) -> lsutil::UserPreferences {
        self.workspace_user_preferences.clone()
    }

    // Trace implements module.ResolutionHost.
    pub fn trace(&self, _msg: &str) {
        panic!("ATA module resolution should not use tracing");
    }

    pub fn configure(&mut self, config: lsutil::UserPreferences) {
        self.pending_user_config_changes = true;
        let old_config = self.workspace_user_preferences.clone();
        self.workspace_user_preferences = config.clone();
        self.refresh_inlay_hints_if_needed(&old_config, &config);
        self.refresh_code_lens_if_needed(&old_config, &config);
        self.refresh_diagnostics_if_needed(&old_config, &config);
        self.refresh_ata_if_needed(&old_config, &config);
    }

    pub fn initialize_with_user_config(&mut self, config: lsutil::UserPreferences) {
        self.initial_user_preferences = config.clone();
        self.configure(config);
    }

    pub fn did_open_file(
        &mut self,
        ctx: core::Context,
        uri: DocumentUri,
        version: i32,
        content: String,
        language_kind: LanguageKind,
    ) {
        self.cancel_warm_auto_import_cache();
        self.schedule_idle_cache_clean();
        let requested_uri = uri.clone();
        self.pending_file_changes.push(FileChange {
            kind: crate::FileChangeKind::Open,
            uri,
            version,
            content,
            language_kind,
            changes: Vec::new(),
        });
        let (file_changes, overlays) = self.flush_changes_locked(ctx.clone());
        self.update_snapshot_inner(
            ctx,
            overlays,
            SnapshotChange {
                reason: UpdateReason::DidOpenFile,
                file_changes,
                resource_request: ResourceRequest {
                    documents: vec![requested_uri.clone()],
                    ..Default::default()
                },
                ..Default::default()
            },
            false,
        );
    }

    pub fn did_close_file(&mut self, _ctx: core::Context, uri: DocumentUri) {
        self.cancel_warm_auto_import_cache();
        self.schedule_idle_cache_clean();
        self.pending_file_changes.push(FileChange {
            kind: crate::FileChangeKind::Close,
            uri,
            version: 0,
            content: String::new(),
            language_kind: LanguageKind::default(),
            changes: Vec::new(),
        });
    }

    pub fn did_change_file(
        &mut self,
        _ctx: core::Context,
        uri: DocumentUri,
        version: i32,
        changes: Vec<TextDocumentContentChangePartialOrWholeDocument>,
    ) {
        self.cancel_diagnostics_refresh();
        self.cancel_warm_auto_import_cache();
        self.schedule_idle_cache_clean();
        self.pending_file_changes.push(FileChange {
            kind: FileChangeKind::Change,
            uri,
            version,
            content: String::new(),
            language_kind: LanguageKind::default(),
            changes,
        });
    }

    pub fn did_save_file(&mut self, _ctx: core::Context, uri: DocumentUri) {
        self.schedule_idle_cache_clean();
        self.pending_file_changes.push(FileChange {
            kind: FileChangeKind::Save,
            uri,
            version: 0,
            content: String::new(),
            language_kind: LanguageKind::default(),
            changes: Vec::new(),
        });
    }

    pub fn did_change_watched_files(
        &mut self,
        _ctx: core::Context,
        changes: Vec<lsproto::FileEvent>,
    ) {
        let mut file_changes = Vec::with_capacity(changes.len());
        for change in changes {
            let kind = match change.typ {
                lsproto::FileChangeType::CREATED => FileChangeKind::WatchCreate,
                lsproto::FileChangeType::CHANGED => FileChangeKind::WatchChange,
                lsproto::FileChangeType::DELETED => FileChangeKind::WatchDelete,
                _ => continue,
            };
            file_changes.push(FileChange {
                kind,
                uri: change.uri.to_string(),
                version: 0,
                content: String::new(),
                language_kind: LanguageKind::default(),
                changes: Vec::new(),
            });
        }
        self.pending_file_changes.extend(file_changes);
        self.schedule_diagnostics_refresh();
        self.cancel_warm_auto_import_cache();
        self.schedule_idle_cache_clean();
    }

    pub fn did_change_compiler_options_for_inferred_projects(
        &mut self,
        ctx: core::Context,
        options: core::CompilerOptions,
    ) {
        self.compiler_options_for_inferred_projects = Some(options.clone());
        self.update_snapshot_inner(
            ctx,
            self.fs.overlays(),
            SnapshotChange {
                reason: UpdateReason::DidChangeCompilerOptionsForInferredProjects,
                compiler_options_for_inferred_projects: Some(options),
                ..Default::default()
            },
            false,
        );
    }

    pub fn schedule_diagnostics_refresh(&mut self) {
        if let Some(cancel) = self.diagnostics_refresh_cancel.take() {
            cancel.cancel();
            self.logger
                .log(&["Delaying scheduled diagnostics refresh..."]);
        } else {
            self.logger.log(&["Scheduling new diagnostics refresh..."]);
        }

        let (debounce_ctx, cancel) = core::with_cancel(self.background_ctx.clone());
        self.diagnostics_refresh_cancel = Some(cancel.clone());
        let delay = self.options.debounce_delay;
        let logging_enabled = self.options.logging_enabled;
        let client = self.client.as_ref().map(|client| client.clone_handle());
        let logger = self.logger.clone();
        let background_ctx = self.background_ctx.clone();
        self.background_queue.enqueue(debounce_ctx, move |ctx| {
            if !core::sleep_or_done(delay, &ctx) {
                cancel.cancel();
                return;
            }
            cancel.cancel();
            if logging_enabled {
                logger.log(&["Running scheduled diagnostics refresh"]);
            }
            if let Some(client) = client {
                if let Err(err) = client.refresh_diagnostics(&background_ctx) {
                    if logging_enabled {
                        logger.logf(format!("Error refreshing diagnostics: {err}"));
                    }
                }
            }
        });
    }

    fn cancel_diagnostics_refresh(&mut self) {
        if let Some(cancel) = self.diagnostics_refresh_cancel.take() {
            cancel.cancel();
            self.logger.log(&["Canceled scheduled diagnostics refresh"]);
        }
    }

    fn cancel_warm_auto_import_cache(&mut self) {
        if let Some(cancel) = self.warm_auto_import_cancel.take() {
            cancel.cancel();
        }
    }

    fn schedule_idle_cache_clean(&mut self) {
        if let Some(timer) = self.idle_cache_clean_timer.take() {
            timer.stop();
        }
        self.idle_cache_clean_due.store(false, Ordering::SeqCst);
        let idle_cache_clean_due = self.idle_cache_clean_due.clone();
        self.idle_cache_clean_timer = Some(core::after_func(IDLE_CACHE_CLEAN_DELAY, move || {
            idle_cache_clean_due.store(true, Ordering::SeqCst);
        }));
    }

    fn run_idle_cache_clean(&mut self) {
        let ctx = self.background_ctx.clone();
        let (file_changes, overlays, ata_changes, new_config) = self.flush_changes(ctx.clone());
        self.update_snapshot_inner(
            ctx,
            overlays,
            SnapshotChange {
                reason: UpdateReason::IdleCleanDiskCache,
                file_changes,
                ata_changes,
                new_config,
                clean_disk_cache: true,
                ..Default::default()
            },
            false,
        );
    }

    fn run_idle_cache_clean_if_due(&mut self) {
        if self.idle_cache_clean_due.swap(false, Ordering::SeqCst) {
            self.idle_cache_clean_timer = None;
            self.run_idle_cache_clean();
        }
    }

    fn cancel_idle_cache_clean(&mut self) {
        let timer = self.idle_cache_clean_timer.take();
        if self.idle_cache_clean_due.swap(false, Ordering::SeqCst) {
            if let Some(timer) = timer {
                timer.stop();
            }
            self.run_idle_cache_clean();
        } else if let Some(timer) = timer {
            timer.stop();
        }
    }

    pub fn start_performance_telemetry(&mut self) {
        if !self.options.telemetry_enabled {
            return;
        }
        let (ctx, cancel) = core::with_cancel(self.background_ctx.clone());
        self.performance_telemetry_cancel = Some(cancel);
        let queue = self.background_queue.clone();
        let session = self.clone_handle();
        queue.enqueue(ctx, move |ctx| {
            let mut ticker = core::new_ticker(PERFORMANCE_TELEMETRY_INTERVAL);
            loop {
                match ticker.select(&ctx) {
                    core::Tick::Done => return,
                    core::Tick::Elapsed => {
                        if session
                            .client
                            .as_ref()
                            .is_none_or(|client| !client.is_active())
                        {
                            continue;
                        }
                        session.send_performance_telemetry(ctx.clone());
                    }
                }
            }
        });
    }

    fn stop_performance_telemetry(&mut self) {
        if let Some(cancel) = self.performance_telemetry_cancel.take() {
            cancel.cancel();
        }
    }

    fn send_performance_telemetry(&self, _ctx: core::Context) {
        let Some(client) = &self.client else {
            return;
        };
        if !self.options.telemetry_enabled {
            return;
        }
        let snapshot = &self.snapshot;

        let mut measurements = lsproto::PerformanceStatsTelemetryMeasurements {
            open_file_count: snapshot.fs.overlays.len() as f64,
            uptime_seconds: self.start_time.elapsed().as_secs_f64(),
            project_count: snapshot.project_collection.projects().len() as f64,
            config_count: snapshot
                .config_file_registry
                .as_ref()
                .map_or(0, |registry| registry.configs.len()) as f64,
            cached_disk_file_count: snapshot.fs.disk_files.len() as f64,
            ..Default::default()
        };
        measurements.go_max_procs = std::thread::available_parallelism()
            .map(|n| n.get() as f64)
            .unwrap_or(0.0);

        if let Some(registry) = snapshot.auto_import_registry() {
            let auto_import_stats = registry.get_cache_stats();
            measurements.auto_import_project_bucket_count =
                auto_import_stats.project_buckets.len() as f64;
            measurements.auto_import_node_modules_bucket_count =
                auto_import_stats.node_modules_buckets.len() as f64;
            measurements.auto_import_unique_package_count =
                auto_import_stats.unique_package_count as f64;
            for bucket in auto_import_stats.project_buckets {
                measurements.auto_import_project_export_count += bucket.export_count as f64;
                measurements.auto_import_project_file_count += bucket.file_count as f64;
            }
            for bucket in auto_import_stats.node_modules_buckets {
                measurements.auto_import_node_modules_export_count += bucket.export_count as f64;
                measurements.auto_import_node_modules_file_count += bucket.file_count as f64;
                if bucket.dependency_names.is_none() {
                    measurements.auto_import_node_modules_unfiltered_bucket_count += 1.0;
                }
            }
        }

        if let Err(err) = client.send_telemetry(
            &self.background_ctx,
            client::TelemetryEvent {
                performance_stats_telemetry_event: Some(lsproto::PerformanceStatsTelemetryEvent {
                    measurements: Some(measurements),
                }),
                ..Default::default()
            },
        ) {
            if self.options.logging_enabled {
                self.logger
                    .logf(format!("Error sending performance telemetry: {err}"));
            }
        }
    }

    fn send_project_info_telemetry_for_new_projects(
        &mut self,
        old_snapshot: &Snapshot,
        new_snapshot: &Snapshot,
    ) {
        if !self.options.telemetry_enabled {
            return;
        }
        let ctx = self.background_ctx.clone();
        let old_projects_by_path = old_snapshot.project_collection.projects_by_path();
        let new_projects_by_path = new_snapshot.project_collection.projects_by_path();
        collections::diff_ordered_maps(
            &old_projects_by_path,
            &new_projects_by_path,
            |_path, added_project| self.send_project_info_telemetry(ctx.clone(), added_project),
            |_path, _removed_project| {},
            |_path, _old_project, _new_project| {},
        );
    }

    fn send_project_info_telemetry(&mut self, ctx: core::Context, project: &Project) {
        let Some(client) = &self.client else {
            return;
        };
        if !self.options.telemetry_enabled
            || self.seen_projects.contains_key(&project.config_file_path)
        {
            return;
        }
        if project.program.is_none() || project.command_line.is_none() {
            return;
        }
        let info = self.collect_project_info_telemetry(project);
        if let Err(err) = client.send_telemetry(&ctx, info) {
            if self.options.logging_enabled {
                self.logger
                    .logf(format!("Error sending project info telemetry: {err}"));
            }
            return;
        }
        self.seen_projects
            .insert(project.config_file_path.clone(), true);
    }

    fn collect_project_info_telemetry(&self, project: &Project) -> lsproto::TelemetryEvent {
        let opts = project
            .command_line
            .as_ref()
            .map(|command_line| command_line.compiler_options())
            .unwrap_or_default();

        let mut config_file_name = "other".to_string();
        if project.kind == crate::Kind::Configured {
            let base_name = tspath::get_base_file_name(&project.config_file_name);
            if base_name == "tsconfig.json" || base_name == "jsconfig.json" {
                config_file_name = base_name;
            }
        }
        let project_type = if project.kind == crate::Kind::Configured {
            "configured"
        } else {
            "inferred"
        };
        let mut props = HashMap::from([
            ("configFileName".to_string(), config_file_name),
            ("projectType".to_string(), project_type.to_string()),
            ("version".to_string(), core::version().to_string()),
        ]);

        let mut compiler_options = serde_json::Map::new();
        set_tristate_json(&mut compiler_options, "strict", opts.strict);
        set_tristate_json(&mut compiler_options, "noImplicitAny", opts.no_implicit_any);
        set_tristate_json(
            &mut compiler_options,
            "noImplicitThis",
            opts.no_implicit_this,
        );
        set_tristate_json(
            &mut compiler_options,
            "strictNullChecks",
            opts.strict_null_checks,
        );
        set_tristate_json(
            &mut compiler_options,
            "strictFunctionTypes",
            opts.strict_function_types,
        );
        set_tristate_json(
            &mut compiler_options,
            "strictBindCallApply",
            opts.strict_bind_call_apply,
        );
        set_tristate_json(
            &mut compiler_options,
            "strictPropertyInitialization",
            opts.strict_property_initialization,
        );
        set_tristate_json(
            &mut compiler_options,
            "strictBuiltinIteratorReturn",
            opts.strict_builtin_iterator_return,
        );
        set_tristate_json(
            &mut compiler_options,
            "useUnknownInCatchVariables",
            opts.use_unknown_in_catch_variables,
        );
        set_tristate_json(
            &mut compiler_options,
            "exactOptionalPropertyTypes",
            opts.exact_optional_property_types,
        );
        set_tristate_json(&mut compiler_options, "allowJs", opts.allow_js);
        set_tristate_json(&mut compiler_options, "checkJs", opts.check_js);
        set_tristate_json(&mut compiler_options, "noEmit", opts.no_emit);
        set_tristate_json(&mut compiler_options, "declaration", opts.declaration);
        set_tristate_json(&mut compiler_options, "composite", opts.composite);
        set_tristate_json(
            &mut compiler_options,
            "isolatedModules",
            opts.isolated_modules,
        );
        set_tristate_json(&mut compiler_options, "skipLibCheck", opts.skip_lib_check);
        set_tristate_json(&mut compiler_options, "incremental", opts.incremental);
        if opts.target != core::ScriptTarget::None {
            compiler_options.insert(
                "target".to_string(),
                serde_json::Value::String(opts.target.to_string()),
            );
        }
        if opts.module != core::ModuleKind::None {
            compiler_options.insert(
                "module".to_string(),
                serde_json::Value::String(opts.module.to_string()),
            );
        }
        if opts.module_resolution != core::ModuleResolutionKind::Unknown {
            compiler_options.insert(
                "moduleResolution".to_string(),
                serde_json::Value::String(opts.module_resolution.string().to_string()),
            );
        }
        if opts.jsx != core::JsxEmit::None {
            compiler_options.insert(
                "jsx".to_string(),
                serde_json::Value::String(opts.jsx.string().to_string()),
            );
        }
        if let Ok(value) = serde_json::to_string(&compiler_options) {
            props.insert("compilerOptions".to_string(), value);
        }

        if let Some(raw) = project.command_line.as_ref().and_then(|command_line| {
            command_line
                .raw
                .as_ref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
                .and_then(|raw| raw.as_object().cloned())
        }) {
            props.insert(
                "extends".to_string(),
                bool_telemetry(raw.contains_key("extends")).to_string(),
            );
            props.insert(
                "files".to_string(),
                bool_telemetry(raw.contains_key("files")).to_string(),
            );
            props.insert(
                "include".to_string(),
                bool_telemetry(raw.contains_key("include")).to_string(),
            );
            props.insert(
                "exclude".to_string(),
                bool_telemetry(raw.contains_key("exclude")).to_string(),
            );
        }

        lsproto::TelemetryEvent {
            project_info_telemetry_event: Some(lsproto::ProjectInfoTelemetryEvent {
                properties: props,
                measurements: Some(count_file_stats(
                    project.program.as_ref().unwrap().get_source_files(),
                )),
            }),
            ..Default::default()
        }
    }

    pub(crate) fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn release_snapshot_handle(&mut self, snapshot: &SnapshotHandle) {
        snapshot.deref(self);
    }

    pub fn get_snapshot_default_project<'snapshot>(
        &self,
        snapshot: &'snapshot SnapshotHandle,
        uri: DocumentUri,
    ) -> Option<&'snapshot Project> {
        snapshot.get_default_project(uri)
    }

    pub fn get_snapshot_default_project_info(
        &self,
        snapshot: &SnapshotHandle,
        uri: DocumentUri,
    ) -> Option<ProjectInfo> {
        snapshot.get_default_project(uri).map(Project::info)
    }

    pub fn current_default_project_info(&self, uri: DocumentUri) -> Option<ProjectInfo> {
        self.snapshot.get_default_project(uri).map(Project::info)
    }

    pub fn current_auto_import_cache_stats(&self) -> Option<ls::AutoImportCacheStats> {
        self.snapshot
            .auto_import_registry()
            .map(|registry| registry.get_cache_stats())
    }

    pub fn current_auto_import_registry_is_prepared_for_importing_file(
        &self,
        importing_file_name: &str,
        project_path: tspath::Path,
        preferences: lsutil::UserPreferences,
    ) -> Option<bool> {
        self.snapshot.auto_import_registry().map(|registry| {
            registry.is_prepared_for_importing_file(importing_file_name, project_path, preferences)
        })
    }

    pub(crate) fn get_snapshot_project_by_path<'snapshot>(
        &self,
        snapshot: &'snapshot SnapshotHandle,
        project_path: tspath::Path,
    ) -> Option<&'snapshot Project> {
        snapshot.get_project_by_path(project_path)
    }

    pub fn get_snapshot_project_program<'snapshot>(
        &self,
        snapshot: &'snapshot SnapshotHandle,
        project_path: tspath::Path,
    ) -> Option<&'snapshot compiler::Program> {
        snapshot
            .get_project_by_path(project_path)
            .and_then(Project::get_program)
    }

    pub fn get_snapshot_project_info_by_path(
        &self,
        snapshot: &SnapshotHandle,
        project_path: tspath::Path,
    ) -> Option<ProjectInfo> {
        snapshot
            .get_project_by_path(project_path)
            .map(Project::info)
    }

    pub fn snapshot_projects<'snapshot>(
        &self,
        snapshot: &'snapshot SnapshotHandle,
    ) -> Vec<&'snapshot Project> {
        snapshot.projects()
    }

    pub fn snapshot_project_infos(&self, snapshot: &SnapshotHandle) -> Vec<ProjectInfo> {
        snapshot.projects().into_iter().map(Project::info).collect()
    }

    pub(crate) fn snapshot_projects_by_path<'snapshot>(
        &self,
        snapshot: &'snapshot SnapshotHandle,
    ) -> collections::OrderedMap<tspath::Path, &'snapshot Project> {
        snapshot.projects_by_path()
    }

    pub fn snapshot_project_infos_by_path(
        &self,
        snapshot: &SnapshotHandle,
    ) -> collections::OrderedMap<tspath::Path, ProjectInfo> {
        let mut projects = collections::OrderedMap::new();
        for (path, project) in snapshot.projects_by_path().entries() {
            projects.set(path.clone(), project.info());
        }
        projects
    }

    pub fn snapshot_config_file_registry<'snapshot>(
        &self,
        snapshot: &'snapshot SnapshotHandle,
    ) -> Option<&'snapshot crate::ConfigFileRegistry> {
        snapshot.config_file_registry()
    }

    fn get_snapshot(
        &mut self,
        ctx: core::Context,
        request: ResourceRequest,
        caller_ref: bool,
    ) -> &Snapshot {
        self.run_idle_cache_clean_if_due();
        let (file_changes, overlays, ata_changes, new_config) = self.flush_changes(ctx.clone());
        if !file_changes.is_empty() || !ata_changes.is_empty() || new_config.is_some() {
            return self.update_snapshot_inner(
                ctx,
                overlays,
                SnapshotChange {
                    reason: UpdateReason::RequestedLanguageServicePendingChanges,
                    file_changes,
                    ata_changes,
                    new_config,
                    resource_request: request,
                    ..Default::default()
                },
                caller_ref,
            );
        }
        let mut update_reason = UpdateReason::Unknown;
        if !request.projects.is_empty() {
            update_reason = UpdateReason::RequestedLanguageServiceProjectDirty;
        } else if request.project_tree.is_some() {
            update_reason = UpdateReason::RequestedLoadProjectTree;
        } else if !request.auto_imports.is_empty() {
            update_reason = UpdateReason::RequestedLanguageServiceWithAutoImports;
        } else {
            for document in &request.documents {
                match self.snapshot.get_default_project(document.clone()) {
                    None => update_reason = UpdateReason::RequestedLanguageServiceProjectNotLoaded,
                    Some(project) if project.dirty => {
                        update_reason = UpdateReason::RequestedLanguageServiceProjectDirty
                    }
                    _ => {}
                }
            }
            if update_reason == UpdateReason::Unknown {
                for document in &request.configured_project_documents {
                    if self.snapshot.fs.is_open_file(&document.file_name()) {
                        match self.snapshot.get_default_project(document.clone()) {
                            None => {
                                update_reason =
                                    UpdateReason::RequestedLanguageServiceProjectNotLoaded
                            }
                            Some(project) if project.dirty => {
                                update_reason = UpdateReason::RequestedLanguageServiceProjectDirty
                            }
                            _ => {}
                        }
                    } else {
                        update_reason = UpdateReason::RequestedLanguageServiceForFileNotOpen;
                    }
                }
            }
        }
        if update_reason == UpdateReason::Unknown {
            if caller_ref {
                self.snapshot.r#ref();
            }
            return &self.snapshot;
        }
        self.update_snapshot_inner(
            ctx,
            overlays,
            SnapshotChange {
                reason: update_reason,
                resource_request: request,
                ..Default::default()
            },
            caller_ref,
        )
    }

    fn get_snapshot_and_default_project(
        &mut self,
        ctx: core::Context,
        uri: DocumentUri,
        caller_ref: bool,
    ) -> Result<(&Snapshot, &Project, SessionLanguageService), String> {
        let snapshot = self.get_snapshot(
            ctx,
            ResourceRequest {
                documents: vec![uri.clone()],
                ..Default::default()
            },
            caller_ref,
        );
        let Some(project) = snapshot.get_default_project(uri.clone()) else {
            return Err(format!("no project found for URI {uri}"));
        };
        let language_service = new_session_language_service(
            project.config_file_path.clone(),
            project.program.clone().unwrap(),
            Box::new(snapshot.clone_host()),
            &uri.file_name(),
        );
        Ok((snapshot, project, language_service))
    }

    pub fn get_language_service(
        &mut self,
        ctx: core::Context,
        uri: DocumentUri,
    ) -> Result<SessionLanguageService, String> {
        let (_, _, language_service) = self.get_snapshot_and_default_project(ctx, uri, false)?;
        Ok(language_service)
    }

    // WithLanguageServiceAndSnapshot synchronously acquires a ref'd snapshot and
    // creates a language service for the given URI. The snapshot is kept alive
    // until the caller releases the returned handle.
    pub fn get_language_service_and_snapshot_handle(
        &mut self,
        ctx: core::Context,
        uri: DocumentUri,
    ) -> Result<(SessionLanguageService, SnapshotHandle), String> {
        let snapshot = self
            .get_snapshot(
                ctx,
                ResourceRequest {
                    documents: vec![uri.clone()],
                    ..Default::default()
                },
                false,
            )
            .clone_handle();
        let Some(project) = snapshot.get_default_project(uri.clone()) else {
            snapshot.deref(self);
            return Err(format!("no project found for URI {uri}"));
        };
        let language_service = new_session_language_service(
            project.config_file_path.clone(),
            project.program.clone().unwrap(),
            Box::new(snapshot.clone_host()),
            &uri.file_name(),
        );
        Ok((language_service, snapshot))
    }

    pub fn get_language_service_and_projects_for_file(
        &mut self,
        ctx: core::Context,
        uri: DocumentUri,
    ) -> Result<(Project, SessionLanguageService, Vec<Project>), String> {
        let (snapshot, project, default_ls) =
            self.get_snapshot_and_default_project(ctx, uri.clone(), false)?;
        let all_projects = snapshot
            .get_projects_containing_file(uri)
            .into_iter()
            .filter_map(|project| {
                snapshot
                    .project_collection
                    .get_project_by_path(ts_ls::Project::id(project))
                    .cloned()
            })
            .collect();
        Ok((project.clone(), default_ls, all_projects))
    }

    pub fn get_projects_for_file(&mut self, ctx: core::Context, uri: DocumentUri) -> Vec<Project> {
        let snapshot = self.get_snapshot(
            ctx,
            ResourceRequest {
                configured_project_documents: vec![uri.clone()],
                ..Default::default()
            },
            false,
        );
        snapshot
            .get_projects_containing_file(uri)
            .into_iter()
            .filter_map(|project| {
                snapshot
                    .project_collection
                    .get_project_by_path(ts_ls::Project::id(project))
                    .cloned()
            })
            .collect()
    }

    pub fn get_language_services_for_documents(
        &mut self,
        ctx: core::Context,
        uris: Vec<DocumentUri>,
    ) -> Vec<SessionLanguageService> {
        let active_file = uris
            .first()
            .map(|uri| uri.file_name().to_string())
            .unwrap_or_default();
        let snapshot = self.get_snapshot(
            ctx,
            ResourceRequest {
                documents: uris,
                ..Default::default()
            },
            false,
        );
        snapshot
            .project_collection
            .projects()
            .into_iter()
            .map(|project| {
                new_session_language_service(
                    project.config_file_path.clone(),
                    project.program.clone().unwrap(),
                    Box::new(snapshot.clone_host()),
                    &active_file,
                )
            })
            .collect()
    }

    pub fn get_language_service_for_project_with_file(
        &mut self,
        ctx: core::Context,
        project: &Project,
        uri: DocumentUri,
    ) -> Option<SessionLanguageService> {
        let project_id = project.id();
        let snapshot = self.get_snapshot(
            ctx,
            ResourceRequest {
                projects: vec![project_id.clone()],
                ..Default::default()
            },
            false,
        );
        let project = snapshot
            .project_collection
            .get_project_by_path(project_id)?;
        if !project.has_file(&uri.file_name()) {
            return None;
        }
        Some(new_session_language_service(
            project.config_file_path.clone(),
            project.program.clone().unwrap(),
            Box::new(snapshot.clone_host()),
            &uri.file_name(),
        ))
    }

    pub fn get_language_service_for_project_id_with_file(
        &mut self,
        ctx: core::Context,
        project_id: tspath::Path,
        uri: DocumentUri,
    ) -> Option<SessionLanguageService> {
        let snapshot = self.get_snapshot(
            ctx,
            ResourceRequest {
                projects: vec![project_id.clone()],
                ..Default::default()
            },
            false,
        );
        let project = snapshot
            .project_collection
            .get_project_by_path(project_id)?;
        if !project.has_file(&uri.file_name()) {
            return None;
        }
        Some(new_session_language_service(
            project.config_file_path.clone(),
            project.program.clone().unwrap(),
            Box::new(snapshot.clone_host()),
            &uri.file_name(),
        ))
    }

    pub(crate) fn with_snapshot_loading_project_tree(
        &mut self,
        ctx: core::Context,
        requested_project_trees: ProjectTreeRequest,
        mut f: impl FnMut(&Snapshot),
    ) {
        let snapshot = self
            .get_snapshot(
                ctx,
                ResourceRequest {
                    project_tree: Some(requested_project_trees),
                    ..Default::default()
                },
                false,
            )
            .clone_handle();
        f(snapshot.snapshot());
        snapshot.deref(self);
    }

    pub fn provide_workspace_symbols_loading_project_tree(
        &mut self,
        ctx: core::Context,
        requested_project_trees: ProjectTreeRequest,
        query: &str,
    ) -> Result<lsproto::WorkspaceSymbolResponse, core::Error> {
        let snapshot = self
            .get_snapshot(
                ctx.clone(),
                ResourceRequest {
                    project_tree: Some(requested_project_trees),
                    ..Default::default()
                },
                false,
            )
            .clone_handle();
        let result = {
            let programs = snapshot
                .projects()
                .into_iter()
                .map(|project| project.get_program().unwrap())
                .collect::<Vec<_>>();
            ls::provide_workspace_symbols(
                &ctx,
                &programs,
                snapshot
                    .converters()
                    .expect("snapshot converters initialized"),
                snapshot.snapshot().user_preferences(),
                query,
            )
        };
        snapshot.deref(self);
        result
    }

    pub fn get_projects_loading_project_tree(
        &mut self,
        ctx: core::Context,
        requested_project_trees: ProjectTreeRequest,
    ) -> Vec<Project> {
        let snapshot = self
            .get_snapshot(
                ctx,
                ResourceRequest {
                    project_tree: Some(requested_project_trees),
                    ..Default::default()
                },
                false,
            )
            .clone_handle();
        let projects = snapshot.projects().into_iter().cloned().collect::<Vec<_>>();
        snapshot.deref(self);
        projects
    }

    pub fn get_current_language_service_with_auto_imports(
        &mut self,
        ctx: core::Context,
        uri: DocumentUri,
    ) -> Result<SessionLanguageService, String> {
        let snapshot = self.get_snapshot(
            ctx,
            ResourceRequest {
                documents: vec![uri.clone()],
                auto_imports: uri.clone(),
                ..Default::default()
            },
            false,
        );
        let Some(project) = snapshot.get_default_project(uri.clone()) else {
            return Err(format!("no project found for URI {uri}"));
        };
        Ok(new_session_language_service(
            project.config_file_path.clone(),
            project.program.clone().unwrap(),
            Box::new(snapshot.clone_host()),
            &uri.file_name(),
        ))
    }

    pub(crate) fn with_language_service_and_snapshot<'s>(
        &'s mut self,
        ctx: core::Context,
        uri: DocumentUri,
        mut f: impl FnMut(
            SessionLanguageService,
            &Snapshot,
        )
            -> Result<Option<Box<dyn FnOnce() -> Result<(), String> + 's>>, String>,
    ) -> Result<Option<Box<dyn FnOnce() -> Result<(), String> + 's>>, String> {
        let snapshot = self
            .get_snapshot(
                ctx,
                ResourceRequest {
                    documents: vec![uri.clone()],
                    ..Default::default()
                },
                false,
            )
            .clone_handle();
        let Some(project) = snapshot.get_default_project(uri.clone()) else {
            snapshot.deref(self);
            return Err(format!("no project found for URI {uri}"));
        };
        let language_service = new_session_language_service(
            project.config_file_path.clone(),
            project.program.clone().unwrap(),
            Box::new(snapshot.clone_host()),
            &uri.file_name(),
        );
        let async_work = f(language_service, snapshot.snapshot())?;
        if async_work.is_none() {
            snapshot.deref(self);
            return Ok(None);
        }
        let mut session = self.clone_handle();
        Ok(Some(Box::new(move || {
            let result = async_work.unwrap()();
            snapshot.deref(&mut session);
            result
        })))
    }

    // GetLanguageServiceWithAutoImports clones the given snapshot with auto-import
    // preparation for the given URI, without flushing pending file changes.
    // The cloned snapshot will be adopted as the session's current snapshot if
    // other changes haven't been adopted in the meantime.
    pub fn get_language_service_with_auto_imports(
        &mut self,
        ctx: core::Context,
        base_snapshot: &SnapshotHandle,
        uri: DocumentUri,
    ) -> Result<SessionLanguageService, String> {
        let change = SnapshotChange {
            reason: UpdateReason::RequestedLanguageServiceWithAutoImports,
            resource_request: ResourceRequest {
                documents: vec![uri.clone()],
                auto_imports: uri.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        let new_snapshot = base_snapshot.snapshot().clone_snapshot(
            ctx,
            change,
            base_snapshot.snapshot().fs.overlays.clone(),
            self,
        );
        let Some(project) = new_snapshot.get_default_project(uri.clone()) else {
            new_snapshot.deref(self);
            return Err(format!("no project found for URI {uri}"));
        };
        let language_service = new_session_language_service(
            project.config_file_path.clone(),
            project.program.clone().unwrap(),
            Box::new(new_snapshot.clone_host()),
            &uri.file_name(),
        );
        if self.snapshot.id() == base_snapshot.id() {
            self.adopt_snapshot_change(new_snapshot);
        } else {
            new_snapshot.deref(self);
        }
        Ok(language_service)
    }

    fn adopt_snapshot_change(&mut self, new_snapshot: Snapshot) {
        let old_snapshot = std::mem::replace(&mut self.snapshot, new_snapshot);
        old_snapshot.deref(self);
    }

    // adoptSnapshotChange promotes a cloned snapshot as the session's current
    // snapshot so future requests benefit from the work already done. If the
    // session has moved on, the snapshot is discarded; the next request needing
    // auto-imports will redo the work on the latest snapshot.
    fn adopt_snapshot_change_if_current(&mut self, base_snapshot_id: u64, new_snapshot: Snapshot) {
        if self.snapshot.id() == base_snapshot_id {
            let old_snapshot = std::mem::replace(&mut self.snapshot, new_snapshot);
            if self.options.logging_enabled {
                self.logger.logf(format!(
                    "Adopted snapshot {} (parent {}) as current session snapshot (replacing {})",
                    self.snapshot.id(),
                    self.snapshot.parent_id(),
                    old_snapshot.id()
                ));
                self.logger.log(&[&self
                    .snapshot
                    .builder_logs
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default()]);
            }
            old_snapshot.deref(self);
        } else {
            if self.options.logging_enabled {
                self.logger.logf(format!(
                    "Discarded snapshot {} (parent {}); session has moved on to snapshot {}",
                    new_snapshot.id(),
                    new_snapshot.parent_id(),
                    self.snapshot.id()
                ));
                let logs = new_snapshot
                    .builder_logs()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                if !logs.is_empty() {
                    self.logger.logf(format!(
                        "--- Discarded snapshot {} builder logs (NOT adopted) ---",
                        new_snapshot.id()
                    ));
                    self.logger.log(&[&logs]);
                    self.logger.logf(format!(
                        "--- End discarded snapshot {} builder logs ---",
                        new_snapshot.id()
                    ));
                }
            }
            new_snapshot.deref(self);
        }
    }

    pub fn update_snapshot(
        &mut self,
        ctx: core::Context,
        overlays: HashMap<tspath::Path, Arc<Overlay>>,
        change: SnapshotChange,
    ) {
        self.update_snapshot_inner(ctx, overlays, change, false);
    }

    pub(crate) fn update_snapshot_ref(
        &mut self,
        ctx: core::Context,
        overlays: HashMap<tspath::Path, Arc<Overlay>>,
        change: SnapshotChange,
    ) -> &Snapshot {
        self.update_snapshot_inner(ctx, overlays, change, true)
    }

    fn update_snapshot_inner(
        &mut self,
        ctx: core::Context,
        overlays: HashMap<tspath::Path, Arc<Overlay>>,
        change: SnapshotChange,
        caller_ref: bool,
    ) -> &Snapshot {
        let old_snapshot = self.snapshot.clone_snapshot_value();
        let new_snapshot = old_snapshot.clone_snapshot(ctx, change.clone(), overlays, self);
        if caller_ref {
            new_snapshot.r#ref();
        }
        self.adopt_snapshot_change(new_snapshot);
        let new_snapshot = self.snapshot.clone_snapshot_value();

        if self.typings_installer.is_some() && !self.config().is_ata_disabled() {
            self.trigger_ata_for_updated_projects(&new_snapshot);
        }

        let queue = self.background_queue.clone();
        let mut session = self.clone_handle();
        queue.enqueue(core::Context::background(), move |_ctx| {
            if session.options.logging_enabled {
                session.logger.logf(format!(
                    "Adopted snapshot {} (parent {}) as current session snapshot (replacing {})",
                    new_snapshot.id(),
                    new_snapshot.parent_id(),
                    old_snapshot.id()
                ));
                session.logger.log(&[&new_snapshot
                    .builder_logs()
                    .map(ToString::to_string)
                    .unwrap_or_default()]);
                session.log_project_changes(&old_snapshot, &new_snapshot);
                session.log_runtime_metrics();
                session.logger.log(&[""]);
            }
            if session.options.watch_enabled {
                if let Err(err) = session.update_watches(&old_snapshot, &new_snapshot) {
                    if session.options.logging_enabled {
                        session.logger.logf(err.to_string());
                    }
                }
            }
            session.publish_program_diagnostics(&old_snapshot, &new_snapshot);
            session.send_project_info_telemetry_for_new_projects(&old_snapshot, &new_snapshot);
            let background_ctx = session.background_ctx.clone();
            session.warm_auto_import_cache(background_ctx, change, &old_snapshot, &new_snapshot);
        });
        &self.snapshot
    }

    pub fn wait_for_background_tasks(&mut self) {
        self.cancel_idle_cache_clean();
        self.background_queue.wait();
    }

    fn update_watches(
        &mut self,
        old_snapshot: &Snapshot,
        new_snapshot: &Snapshot,
    ) -> Result<(), String> {
        let mut errors = Vec::new();
        let start = Instant::now();
        let ctx = self.background_ctx.clone();
        let logger = self.logger.clone();
        let root_files_watch_id =
            |watch: &Option<WatchedFiles<PatternsAndIgnored>>| watch.as_ref().map(watched_files_id);
        let old_configs = &old_snapshot.config_file_registry.as_ref().unwrap().configs;
        let new_configs = &new_snapshot.config_file_registry.as_ref().unwrap().configs;

        // PORT NOTE: reshaped for borrowck. This preserves DiffMapsFunc order:
        // added entries in new map order, then changed/removed entries in old map order.
        for (path, added_entry) in new_configs {
            if !old_configs.contains_key(path) {
                if let Some(root_files_watch) = added_entry.root_files_watch.as_ref() {
                    errors.extend(update_watch(
                        &ctx,
                        self,
                        logger.as_ref(),
                        None,
                        Some(root_files_watch),
                    ));
                }
            }
        }
        for (path, old_entry) in old_configs {
            if let Some(new_entry) = new_configs.get(path) {
                if root_files_watch_id(&old_entry.root_files_watch)
                    != root_files_watch_id(&new_entry.root_files_watch)
                {
                    errors.extend(update_watch(
                        &ctx,
                        self,
                        logger.as_ref(),
                        old_entry.root_files_watch.as_ref(),
                        new_entry.root_files_watch.as_ref(),
                    ));
                }
            } else if let Some(root_files_watch) = old_entry.root_files_watch.as_ref() {
                errors.extend(update_watch(
                    &ctx,
                    self,
                    logger.as_ref(),
                    Some(root_files_watch),
                    None,
                ));
            }
        }
        for (path, new_entry) in &new_snapshot.config_file_registry.as_ref().unwrap().configs {
            if let Some(old_entry) = old_snapshot
                .config_file_registry
                .as_ref()
                .unwrap()
                .configs
                .get(path)
            {
                let new_watch_id = root_files_watch_id(&new_entry.root_files_watch);
                if root_files_watch_id(&old_entry.root_files_watch) == new_watch_id
                    && new_watch_id
                        .as_ref()
                        .is_some_and(|id| self.watches.is_pending(id))
                {
                    if let Some(root_files_watch) = new_entry.root_files_watch.as_ref() {
                        errors.extend(update_watch(
                            &ctx,
                            self,
                            logger.as_ref(),
                            None,
                            Some(root_files_watch),
                        ));
                    }
                }
            }
        }
        let old_projects_by_path = old_snapshot.project_collection.projects_by_path();
        let new_projects_by_path = new_snapshot.project_collection.projects_by_path();
        // PORT NOTE: reshaped for borrowck. This preserves DiffOrderedMaps order:
        // additions in new order, then modifications/removals in old order.
        for (path, added_project) in new_projects_by_path.entries() {
            if old_projects_by_path.get(path).is_none() {
                errors.extend(update_watch(
                    &ctx,
                    self,
                    logger.as_ref(),
                    None,
                    added_project.program_files_watch.as_ref(),
                ));
                errors.extend(update_watch(
                    &ctx,
                    self,
                    logger.as_ref(),
                    None,
                    added_project.typings_watch.as_ref(),
                ));
            }
        }
        for (path, old_project) in old_projects_by_path.entries() {
            if let Some(new_project) = new_projects_by_path.get(path) {
                if watched_files_id(old_project.program_files_watch.as_ref().unwrap())
                    != watched_files_id(new_project.program_files_watch.as_ref().unwrap())
                {
                    errors.extend(update_watch(
                        &ctx,
                        self,
                        logger.as_ref(),
                        old_project.program_files_watch.as_ref(),
                        new_project.program_files_watch.as_ref(),
                    ));
                } else {
                    let id = watched_files_id(new_project.program_files_watch.as_ref().unwrap());
                    if self.watches.is_pending(&id) {
                        errors.extend(update_watch(
                            &ctx,
                            self,
                            logger.as_ref(),
                            None,
                            new_project.program_files_watch.as_ref(),
                        ));
                    }
                }
                if watched_files_id(old_project.typings_watch.as_ref().unwrap())
                    != watched_files_id(new_project.typings_watch.as_ref().unwrap())
                {
                    errors.extend(update_watch(
                        &ctx,
                        self,
                        logger.as_ref(),
                        old_project.typings_watch.as_ref(),
                        new_project.typings_watch.as_ref(),
                    ));
                } else {
                    let id = watched_files_id(new_project.typings_watch.as_ref().unwrap());
                    if self.watches.is_pending(&id) {
                        errors.extend(update_watch(
                            &ctx,
                            self,
                            logger.as_ref(),
                            None,
                            new_project.typings_watch.as_ref(),
                        ));
                    }
                }
            } else {
                errors.extend(update_watch(
                    &ctx,
                    self,
                    logger.as_ref(),
                    old_project.program_files_watch.as_ref(),
                    None,
                ));
                errors.extend(update_watch(
                    &ctx,
                    self,
                    logger.as_ref(),
                    old_project.typings_watch.as_ref(),
                    None,
                ));
            }
        }
        let old_auto_imports_watch_id =
            watched_files_id(old_snapshot.auto_imports_watch.as_ref().unwrap());
        let new_auto_imports_watch_id =
            watched_files_id(new_snapshot.auto_imports_watch.as_ref().unwrap());
        if old_auto_imports_watch_id != new_auto_imports_watch_id {
            errors.extend(update_watch(
                &ctx,
                self,
                logger.as_ref(),
                old_snapshot.auto_imports_watch.as_ref(),
                new_snapshot.auto_imports_watch.as_ref(),
            ));
        } else if self.watches.is_pending(&new_auto_imports_watch_id) {
            errors.extend(update_watch(
                &ctx,
                self,
                logger.as_ref(),
                None,
                new_snapshot.auto_imports_watch.as_ref(),
            ));
        }
        if !errors.is_empty() {
            Err(format!("errors updating watches: {errors:?}"))
        } else {
            if self.options.logging_enabled {
                self.logger
                    .logf(format!("Updated watches in {:?}", start.elapsed()));
            }
            Ok(())
        }
    }

    pub fn close(&mut self) {
        self.cancel_diagnostics_refresh();
        self.cancel_warm_auto_import_cache();
        self.cancel_idle_cache_clean();
        self.stop_performance_telemetry();
        self.background_queue.close();
    }

    pub(crate) fn flush_changes(
        &mut self,
        ctx: core::Context,
    ) -> (
        FileChangeSummary,
        HashMap<tspath::Path, Arc<Overlay>>,
        HashMap<tspath::Path, crate::AtaStateChange>,
        Option<lsutil::UserPreferences>,
    ) {
        let ata_changes = std::mem::take(&mut self.pending_ata_changes);
        let (file_changes, overlays) = self.flush_changes_locked(ctx);
        let new_config = if self.pending_user_config_changes {
            Some(self.workspace_user_preferences.clone())
        } else {
            None
        };
        self.pending_user_config_changes = false;
        (file_changes, overlays, ata_changes, new_config)
    }

    fn flush_changes_locked(
        &mut self,
        _ctx: core::Context,
    ) -> (FileChangeSummary, HashMap<tspath::Path, Arc<Overlay>>) {
        if self.pending_file_changes.is_empty() {
            return (FileChangeSummary::default(), self.fs.overlays());
        }
        let start = Instant::now();
        let changes = std::mem::take(&mut self.pending_file_changes);
        let len = changes.len();
        let result = self.fs.process_changes(changes);
        if self.options.logging_enabled {
            self.logger.logf(format!(
                "Processed {len} file changes in {:?}",
                start.elapsed()
            ));
        }
        result
    }

    fn log_project_changes(&self, old_snapshot: &Snapshot, new_snapshot: &Snapshot) {
        let logged_project_changes = std::cell::Cell::new(false);
        let old_projects_by_path = old_snapshot.project_collection.projects_by_path();
        let new_projects_by_path = new_snapshot.project_collection.projects_by_path();
        collections::diff_ordered_maps(
            &old_projects_by_path,
            &new_projects_by_path,
            |_path, added_project| {
                let mut builder = String::new();
                added_project.print(
                    self.logger.is_verbose(),
                    self.logger.is_verbose(),
                    &mut builder,
                );
                self.logger.log(&[&builder]);
                logged_project_changes.set(true);
            },
            |_path, removed_project| {
                self.logger.logf(format!(
                    "\nProject '{}' removed\n{}",
                    removed_project.name(),
                    crate::HR
                ));
            },
            |_path, _old_project, new_project| {
                if new_project.program_update_kind == crate::ProgramUpdateKind::NewFiles {
                    let mut builder = String::new();
                    new_project.print(
                        self.logger.is_verbose(),
                        self.logger.is_verbose(),
                        &mut builder,
                    );
                    self.logger.log(&[&builder]);
                    logged_project_changes.set(true);
                }
            },
        );
        if logged_project_changes.get() || self.logger.is_verbose() {
            self.log_cache_stats(new_snapshot);
        }
    }

    fn log_runtime_metrics(&self) {
        let mut builder = String::new();
        builder.push_str("\n======== Runtime Metrics ========");
        let _ = write!(
            builder,
            "\navailableParallelism = {}",
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(0)
        );
        self.logger.log(&[&builder]);
    }

    fn log_cache_stats(&self, snapshot: &Snapshot) {
        let parse_cache_size = self.parse_cache.len();
        let extended_config_count = self.extended_config_cache.len();
        self.logger.log(&["\n======== Cache Statistics ========"]);
        self.logger.logf(format!(
            "Open file count:   {:6}",
            snapshot.fs.overlays.len()
        ));
        self.logger.logf(format!(
            "Cached disk files: {:6}",
            snapshot.fs.disk_files.len()
        ));
        self.logger.logf(format!(
            "Realpath aliases:  {:6}",
            snapshot.fs.node_modules_realpath_aliases.len()
        ));
        self.logger.logf(format!(
            "Project count:     {:6}",
            snapshot.project_collection.projects().len()
        ));
        self.logger.logf(format!(
            "Config count:      {:6}",
            snapshot
                .config_file_registry
                .as_ref()
                .map_or(0, |registry| registry.configs.len())
        ));
        if self.logger.is_verbose() {
            self.logger.logf(format!(
                "Parse cache size:           {:6}",
                parse_cache_size
            ));
            self.logger.logf(format!(
                "Program count:              {:6}",
                self.program_counter.len()
            ));
            self.logger.logf(format!(
                "Extended config cache size: {:6}",
                extended_config_count
            ));
            self.logger.log(&["Auto Imports:"]);
            let auto_import_stats = snapshot.auto_import_registry().unwrap().get_cache_stats();
            self.logger.logf(format!(
                "\tUnique packages (by realpath): {}",
                auto_import_stats.unique_package_count
            ));
            for bucket in auto_import_stats.project_buckets {
                self.logger.logf(format!(
                    "\t\t{}{}:",
                    bucket.path,
                    core::if_else(bucket.state.dirty(), " (dirty)", "")
                ));
                self.logger
                    .logf(format!("\t\t\tFiles: {}", bucket.file_count));
                self.logger
                    .logf(format!("\t\t\tExports: {}", bucket.export_count));
            }
            for bucket in auto_import_stats.node_modules_buckets {
                self.logger.logf(format!(
                    "\t\t{}{}:",
                    bucket.path,
                    core::if_else(bucket.state.dirty(), " (dirty)", "")
                ));
                if let Some(dirty_packages) = bucket.state.dirty_packages() {
                    for package_name in dirty_packages.keys().into_iter().flatten() {
                        self.logger
                            .logf(format!("\t\t\tNeeds granular update: {package_name}"));
                    }
                }
                if let Some(dependency_names) = &bucket.dependency_names {
                    self.logger.logf(format!(
                        "\t\t\tCollected packages: {}",
                        dependency_names.len()
                    ));
                } else {
                    self.logger
                        .log(&["\t\t\tCollected packages: all, due to no package.json!"]);
                }
                self.logger.logf(format!(
                    "\t\t\tTotal packages: {}",
                    bucket
                        .package_names
                        .as_ref()
                        .map_or(0, |packages| packages.len())
                ));
                self.logger
                    .logf(format!("\t\t\tFiles: {}", bucket.file_count));
                self.logger
                    .logf(format!("\t\t\tExports: {}", bucket.export_count));
                if bucket.state.recursive_search_packages().is_none() {
                    self.logger.log(&["\t\t\tRecursive search: all"]);
                } else if bucket.state.recursive_search_packages().unwrap().len() > 0 {
                    self.logger.logf(format!(
                        "\t\t\tRecursive search: {} packages",
                        bucket.state.recursive_search_packages().unwrap().len()
                    ));
                } else {
                    self.logger.log(&["\t\t\tRecursive search: none"]);
                }
            }
        }
    }

    pub fn npm_install(&self, cwd: &str, npm_install_args: &[String]) -> Result<Vec<u8>, String> {
        self.npm_executor
            .as_ref()
            .ok_or_else(|| "npm executor is not configured".to_string())?
            .npm_install(cwd, npm_install_args)
    }

    fn refresh_inlay_hints_if_needed(
        &self,
        old_prefs: &lsutil::UserPreferences,
        new_prefs: &lsutil::UserPreferences,
    ) {
        if old_prefs.inlay_hints != new_prefs.inlay_hints {
            if let Some(client) = &self.client {
                let _ = client.refresh_inlay_hints(&self.background_ctx);
            }
        }
    }

    fn refresh_code_lens_if_needed(
        &self,
        old_prefs: &lsutil::UserPreferences,
        new_prefs: &lsutil::UserPreferences,
    ) {
        if old_prefs.code_lens != new_prefs.code_lens {
            if let Some(client) = &self.client {
                let _ = client.refresh_code_lens(&self.background_ctx);
            }
        }
    }

    fn refresh_diagnostics_if_needed(
        &mut self,
        old_prefs: &lsutil::UserPreferences,
        new_prefs: &lsutil::UserPreferences,
    ) {
        if old_prefs.custom_config_file_name != new_prefs.custom_config_file_name {
            self.schedule_diagnostics_refresh();
        }
    }

    fn refresh_ata_if_needed(
        &mut self,
        old_prefs: &lsutil::UserPreferences,
        new_prefs: &lsutil::UserPreferences,
    ) {
        if old_prefs.is_ata_disabled() && !new_prefs.is_ata_disabled() {
            self.schedule_diagnostics_refresh();
        }
    }

    fn publish_program_diagnostics(&self, old_snapshot: &Snapshot, new_snapshot: &Snapshot) {
        if !self.options.push_diagnostics_enabled {
            return;
        }
        let ctx = self.background_ctx.clone();
        let old_projects_by_path = old_snapshot.project_collection.projects_by_path();
        let new_projects_by_path = new_snapshot.project_collection.projects_by_path();
        collections::diff_ordered_maps(
            &old_projects_by_path,
            &new_projects_by_path,
            |config_file_path, added_project| {
                if should_publish_program_diagnostics(added_project, new_snapshot.id()) {
                    self.publish_project_diagnostics(
                        &ctx,
                        config_file_path,
                        added_project.get_project_diagnostics(&ctx),
                        new_snapshot.converters.as_ref().unwrap(),
                    );
                }
            },
            |config_file_path, removed_project| {
                if removed_project.kind == crate::Kind::Configured {
                    self.publish_project_diagnostics(
                        &ctx,
                        config_file_path,
                        Vec::new(),
                        old_snapshot.converters.as_ref().unwrap(),
                    );
                }
            },
            |config_file_path, _old_project, new_project| {
                if should_publish_program_diagnostics(new_project, new_snapshot.id()) {
                    self.publish_project_diagnostics(
                        &ctx,
                        config_file_path,
                        new_project.get_project_diagnostics(&ctx),
                        new_snapshot.converters.as_ref().unwrap(),
                    );
                }
            },
        );
    }

    fn publish_project_diagnostics(
        &self,
        ctx: &core::Context,
        config_file_path: &str,
        diagnostics: Vec<ast::Diagnostic>,
        converters: &lsconv::Converters,
    ) {
        let lsp_diagnostics = diagnostics
            .iter()
            .map(|diag| lsconv::diagnostic_to_lsp_push(ctx, converters, diag))
            .collect::<Vec<_>>();
        if let Some(client) = &self.client {
            if let Err(err) = client.publish_diagnostics(
                ctx,
                client::PublishDiagnosticsParams {
                    uri: lsconv::file_name_to_document_uri(config_file_path),
                    diagnostics: lsp_diagnostics,
                    version: None,
                },
            ) {
                if self.options.logging_enabled {
                    self.logger
                        .logf(format!("Error publishing diagnostics: {err}"));
                }
            }
        }
    }

    pub fn enqueue_publish_global_diagnostics(&mut self) {
        if !self.options.push_diagnostics_enabled {
            return;
        }
        if self
            .global_diag_publish_pending
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let queue = self.background_queue.clone();
            let mut session = self.clone_handle();
            queue.enqueue(core::Context::background(), move |_ctx| {
                let background_ctx = session.background_ctx.clone();
                session.publish_global_diagnostics(background_ctx)
            });
        }
    }

    fn publish_global_diagnostics(&mut self, ctx: core::Context) {
        self.global_diag_publish_pending
            .store(false, Ordering::SeqCst);
        let snapshot = self.snapshot.clone_handle();
        snapshot.r#ref();
        for project in snapshot.project_collection().projects() {
            if project.kind != crate::Kind::Configured || project.checker_pool.is_none() {
                continue;
            }
            if project
                .checker_pool
                .as_ref()
                .unwrap()
                .take_new_global_diagnostics()
            {
                self.publish_project_diagnostics(
                    &ctx,
                    &project.config_file_path,
                    project.get_project_diagnostics(&ctx),
                    snapshot.converters().unwrap(),
                );
            }
        }
        snapshot.deref(self);
    }

    fn trigger_ata_for_updated_projects(&mut self, new_snapshot: &Snapshot) {
        for project in new_snapshot.project_collection.projects() {
            if !project.should_trigger_ata(new_snapshot.id()) {
                continue;
            }
            let queue = self.background_queue.clone();
            let mut session = self.clone_handle();
            let project = project.clone_project();
            queue.enqueue(core::Context::background(), move |_ctx| {
                let log_tree = if session.options.logging_enabled {
                    Some(logging::new_log_tree(format!(
                        "Triggering ATA for project {}",
                        project.name()
                    )))
                } else {
                    None
                };
                let typings_info = project.compute_typings_info();
                let request = ata::TypingsInstallRequest {
                    project_id: project.config_file_path.clone(),
                    typings_info: typings_info.clone(),
                    file_names: project
                        .program
                        .as_ref()
                        .unwrap()
                        .get_source_files()
                        .iter()
                        .map(|file| file.file_name())
                        .collect(),
                    project_root_path: project.current_directory.clone(),
                    compiler_options: project.command_line.as_ref().unwrap().compiler_options(),
                    current_directory: session.options.current_directory.clone(),
                    get_script_kind: core::get_script_kind_from_file_name,
                    fs: session.fs.fs.clone(),
                    logger: log_tree.clone(),
                };
                let project_display_name = project.display_name(&session.options.current_directory);
                if let Some(client) = &session.client {
                    client.progress_start(
                        &diagnostics::INSTALLING_TYPES_FOR_0,
                        &[project_display_name.clone()],
                    );
                }
                let result = session
                    .typings_installer
                    .as_mut()
                    .unwrap()
                    .install_typings(request);
                if let Some(client) = &session.client {
                    client.progress_finish(
                        &diagnostics::INSTALLING_TYPES_FOR_0,
                        &[project_display_name],
                    );
                }
                match result {
                    Err(err) => {
                        if let Some(log_tree) = log_tree {
                            session.logger.logf(format!(
                                "ATA installation failed for project {}: {err}",
                                project.name()
                            ));
                            session.logger.log(&[&log_tree.to_string()]);
                        }
                    }
                    Ok(result) => {
                        if result.typings_files != project.typings_files {
                            session.pending_ata_changes.insert(
                                project.config_file_path.clone(),
                                crate::AtaStateChange {
                                    typings_info: Some(typings_info),
                                    typings_files: result.typings_files,
                                    typings_files_to_watch: result.files_to_watch,
                                    logs: None,
                                    ..Default::default()
                                },
                            );
                            session.schedule_diagnostics_refresh();
                        }
                    }
                }
            });
        }
    }

    fn warm_auto_import_cache(
        &mut self,
        ctx: core::Context,
        change: SnapshotChange,
        _old_snapshot: &Snapshot,
        new_snapshot: &Snapshot,
    ) {
        if change.file_changes.changed.len() != 1 {
            return;
        }
        let changed_file = change.file_changes.changed.iter().next().unwrap().clone();
        if !new_snapshot.fs.is_open_file(&changed_file.file_name()) {
            return;
        }
        let prefs = new_snapshot.user_preferences();
        if prefs.include_completions_for_module_exports.is_false() {
            return;
        }
        let Some(project) = new_snapshot.get_default_project(changed_file.clone()) else {
            return;
        };
        if new_snapshot
            .auto_imports
            .as_ref()
            .unwrap()
            .is_prepared_for_importing_file(
                &changed_file.file_name(),
                project.config_file_path.clone(),
                prefs.clone(),
            )
        {
            return;
        }

        if let Some(cancel) = self.warm_auto_import_cancel.take() {
            cancel.cancel();
        }
        let (warm_ctx, cancel) = core::with_cancel(ctx);
        if warm_ctx.err().is_none() {
            self.warm_auto_import_cancel = Some(cancel.clone());
        }

        if warm_ctx.err().is_some() {
            cancel.cancel();
            return;
        }
        if !new_snapshot.try_ref() {
            return;
        }
        let warm_change = SnapshotChange {
            reason: UpdateReason::RequestedLanguageServiceWithAutoImports,
            resource_request: ResourceRequest {
                documents: vec![changed_file.clone()],
                auto_imports: changed_file,
                ..Default::default()
            },
            ..Default::default()
        };
        let cloned_snapshot = new_snapshot.clone_snapshot(
            warm_ctx.clone(),
            warm_change,
            new_snapshot.fs.overlays.clone(),
            self,
        );
        if warm_ctx.err().is_some() {
            cloned_snapshot.deref(self);
            new_snapshot.deref(self);
            return;
        }
        self.adopt_snapshot_change_if_current(new_snapshot.id(), cloned_snapshot);
        new_snapshot.deref(self);
        cancel.cancel();
    }
}

impl tsoptions::ParseConfigHost for Session {
    fn fs(&self) -> &dyn vfs::Fs {
        self.fs.fs.as_ref()
    }

    fn get_current_directory(&self) -> String {
        self.options.current_directory.clone()
    }
}

fn update_watch<T: Clone + Default>(
    ctx: &core::Context,
    session: &mut Session,
    logger: &dyn Logger,
    old_watcher: Option<&WatchedFiles<T>>,
    new_watcher: Option<&WatchedFiles<T>>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(new_watcher) = new_watcher {
        let mut new_watcher = new_watcher.clone();
        let w = new_watcher.watchers();
        let mut watchers = w.workspace_watchers;
        watchers.extend(w.outside_workspace_watchers);
        if !watchers.is_empty() {
            let mut new_watchers = collections::OrderedMap::new();
            for (i, watcher) in watchers.iter().enumerate() {
                let glob_id = WatcherId(format!("{}.{}", w.watcher_id.0, i));
                if session.watches.acquire(watcher, glob_id.clone()) {
                    new_watchers.set(glob_id, watcher.clone());
                }
            }
            let mut watch_errors = Vec::new();
            for (id, watcher) in new_watchers.entries() {
                let err = session.client.as_ref().unwrap().watch_files(
                    ctx,
                    id.clone(),
                    vec![watcher.clone()],
                );
                if let Err(err) = err {
                    watch_errors.push(err);
                } else if old_watcher.is_none() {
                    logger.logf(format!("Added new watch: {}", id.0));
                    logger.log(&["\t", &file_system_watcher_glob_string(watcher)]);
                    logger.log(&[""]);
                } else {
                    logger.logf(format!("Updated watch: {}", id.0));
                    logger.log(&["\t", &file_system_watcher_glob_string(watcher)]);
                    logger.log(&[""]);
                }
            }
            if !watch_errors.is_empty() {
                for (_, watcher) in new_watchers.entries() {
                    session.watches.release(watcher);
                }
                session.watches.mark_pending(w.watcher_id.clone());
                errors.extend(watch_errors);
            } else {
                session.watches.clear_pending(&w.watcher_id);
            }
            if !w.ignored_paths.is_empty() {
                logger.logf(format!(
                    "{} paths ineligible for watching",
                    w.ignored_paths.len()
                ));
                if logger.is_verbose() {
                    for path in w.ignored_paths {
                        logger.log(&["\t", &path]);
                    }
                }
            }
        }
    }
    if let Some(old_watcher) = old_watcher {
        let mut old_watcher = old_watcher.clone();
        let w = old_watcher.watchers();
        let mut watchers = w.workspace_watchers;
        watchers.extend(w.outside_workspace_watchers);
        if !watchers.is_empty() {
            let mut removed_ids = Vec::new();
            for watcher in &watchers {
                let (id, removed) = session.watches.release(watcher);
                if removed {
                    removed_ids.push(id);
                }
            }
            for id in removed_ids {
                let err = session
                    .client
                    .as_ref()
                    .unwrap()
                    .unwatch_files(ctx, id.clone());
                if let Err(err) = err {
                    errors.push(err);
                } else if new_watcher.is_none() {
                    logger.logf(format!("Removed watch: {}", id.0));
                }
            }
        }
    }
    errors
}

fn watched_files_id<T: Clone + Default>(watch: &WatchedFiles<T>) -> WatcherId {
    let mut watch = watch.clone();
    watch.id()
}

fn set_tristate(m: &mut HashMap<String, bool>, key: &str, v: core::Tristate) {
    if v == core::Tristate::True {
        m.insert(key.to_string(), true);
    } else if v == core::Tristate::False {
        m.insert(key.to_string(), false);
    }
}

fn set_tristate_json(
    m: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    v: core::Tristate,
) {
    if v == core::Tristate::True {
        m.insert(key.to_string(), serde_json::Value::Bool(true));
    } else if v == core::Tristate::False {
        m.insert(key.to_string(), serde_json::Value::Bool(false));
    }
}

fn bool_telemetry(v: bool) -> &'static str {
    if v { "true" } else { "false" }
}

fn count_file_stats(
    source_files: Vec<ast::SourceFile>,
) -> lsproto::ProjectInfoTelemetryMeasurements {
    let mut stats = lsproto::ProjectInfoTelemetryMeasurements::default();
    for source_file in source_files {
        let size = source_file.end() as f64;
        match source_file.script_kind() {
            core::ScriptKind::JS => {
                stats.js_file_count += 1.0;
                stats.js_file_size += size;
            }
            core::ScriptKind::JSX => {
                stats.jsx_file_count += 1.0;
                stats.jsx_file_size += size;
            }
            core::ScriptKind::TS => {
                if tspath::is_declaration_file_name(&source_file.file_name()) {
                    stats.dts_file_count += 1.0;
                    stats.dts_file_size += size;
                } else {
                    stats.ts_file_count += 1.0;
                    stats.ts_file_size += size;
                }
            }
            core::ScriptKind::TSX => {
                stats.tsx_file_count += 1.0;
                stats.tsx_file_size += size;
            }
            _ => {}
        }
    }
    stats
}

fn should_publish_program_diagnostics(p: &Project, snapshot_id: u64) -> bool {
    p.kind == crate::Kind::Configured
        && p.program.is_some()
        && p.program_last_update == snapshot_id
        && p.program_update_kind > crate::ProgramUpdateKind::Cloned
}
