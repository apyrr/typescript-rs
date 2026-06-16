use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use ts_ast as ast;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs::{cachedvfs, trackingvfs, vfswatch};

use crate::incremental;
use crate::tsc;

pub struct CachedParsedSourceFile {
    pub file: ast::ParsedSourceFile,
    pub mod_time: Option<SystemTime>,
}

impl Clone for CachedParsedSourceFile {
    fn clone(&self) -> Self {
        Self {
            file: self.file.share_readonly(),
            mod_time: self.mod_time,
        }
    }
}

pub struct WatchCompilerHost {
    pub compiler_host: Box<dyn compiler::CompilerHost>,
    pub cache: Arc<collections::SyncMap<tspath::Path, CachedParsedSourceFile>>,
}

impl WatchCompilerHost {
    pub fn get_parsed_source_file(
        &self,
        opts: ast::SourceFileParseOptions,
    ) -> Option<ast::ParsedSourceFile> {
        let info = self.compiler_host.fs().stat(&opts.file_name).ok();

        let (cached, ok) = self.cache.load(&opts.path);
        if let Some(cached) = cached.filter(|_| ok) {
            if info
                .as_ref()
                .is_some_and(|info| info.mod_time() == cached.mod_time)
            {
                return Some(cached.file.share_readonly());
            }
        }

        let file = self.compiler_host.get_parsed_source_file(opts.clone());
        if let Some(file) = &file {
            if let Some(info) = info {
                self.cache.store(
                    opts.path,
                    Some(CachedParsedSourceFile {
                        file: file.share_readonly(),
                        mod_time: info.mod_time(),
                    }),
                );
            }
        } else {
            self.cache.delete(&opts.path);
        }
        file
    }

    pub fn get_source_file(&self, opts: ast::SourceFileParseOptions) -> Option<ast::SourceFile> {
        self.get_parsed_source_file(opts)
            .map(ast::ParsedSourceFile::into_source_file)
    }
}

impl compiler::CompilerHost for WatchCompilerHost {
    fn fs(&self) -> &dyn ts_vfs::Fs {
        self.compiler_host.fs()
    }

    fn default_library_path(&self) -> String {
        self.compiler_host.default_library_path()
    }

    fn get_current_directory(&self) -> String {
        self.compiler_host.get_current_directory()
    }

    fn trace(&self, msg: &'static diagnostics::Message, args: &compiler::DiagnosticArgs) {
        self.compiler_host.trace(msg, args);
    }

    fn get_parsed_source_file(
        &self,
        opts: ast::SourceFileParseOptions,
    ) -> Option<ast::ParsedSourceFile> {
        WatchCompilerHost::get_parsed_source_file(self, opts)
    }

    fn get_source_file(&self, opts: ast::SourceFileParseOptions) -> Option<ast::SourceFile> {
        WatchCompilerHost::get_source_file(self, opts)
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: tspath::Path,
    ) -> Option<tsoptions::ParsedCommandLine> {
        self.compiler_host
            .get_resolved_project_reference(file_name, path)
    }
}

pub struct Watcher {
    pub mu: Arc<Mutex<()>>,
    pub sys: tsc::System,
    pub config_file_name: String,
    pub config: tsoptions::ParsedCommandLine,
    pub compiler_options_from_command_line: core::CompilerOptions,
    pub watch_options_from_command_line: BTreeMap<String, String>,
    pub report_diagnostic: tsc::DiagnosticReporter,
    pub report_error_summary: tsc::DiagnosticsReporter,
    pub report_watch_status: tsc::DiagnosticReporter,
    pub testing: Option<tsc::CommandLineTesting>,

    pub program: Option<incremental::Program>,
    pub extended_config_cache: Option<Arc<tsc::ExtendedConfigCache>>,
    pub config_modified: bool,
    pub config_has_errors: bool,
    pub config_file_paths: Vec<String>,

    source_file_cache: Arc<collections::SyncMap<tspath::Path, CachedParsedSourceFile>>,
    pub file_watcher: vfswatch::FileWatcher,
}

pub fn create_watcher(
    sys: tsc::System,
    config_parse_result: tsoptions::ParsedCommandLine,
    compiler_options_from_command_line: core::CompilerOptions,
    report_diagnostic: tsc::DiagnosticReporter,
    report_error_summary: tsc::DiagnosticsReporter,
    watch_options_from_command_line: BTreeMap<String, String>,
    testing: Option<tsc::CommandLineTesting>,
) -> Watcher {
    let report_watch_status = tsc::create_watch_status_reporter(
        sys.clone(),
        config_parse_result.locale(),
        config_parse_result.compiler_options(),
        testing.clone(),
    );
    let mut w = Watcher {
        mu: Arc::new(Mutex::new(())),
        sys: sys.clone(),
        config: config_parse_result,
        compiler_options_from_command_line,
        watch_options_from_command_line,
        report_diagnostic,
        report_error_summary,
        report_watch_status,
        testing,
        source_file_cache: Arc::new(collections::SyncMap::default()),
        config_file_name: String::new(),
        program: None,
        extended_config_cache: None,
        config_modified: false,
        config_has_errors: false,
        config_file_paths: Vec::new(),
        file_watcher: vfswatch::FileWatcher::new(
            Arc::new(sys.clone()),
            std::time::Duration::ZERO,
            false,
            || {},
        ),
    };
    if let Some(config_file) = &w.config.config_file {
        w.config_file_name = config_file.source_file.file_name();
    }
    w.file_watcher = vfswatch::FileWatcher::new(
        Arc::new(w.sys.clone()),
        w.config.watch_options().watch_interval(),
        w.testing.is_some(),
        || {},
    );
    w
}

impl Watcher {
    pub fn start(&mut self) {
        // PORT NOTE: reshaped for borrowck; clone the mutex handle so the guard
        // does not borrow `self` while the build mutates watcher fields.
        let mu = Arc::clone(&self.mu);
        let _lock = mu.lock().unwrap_or_else(|err| err.into_inner());
        self.extended_config_cache = Some(Arc::new(tsc::ExtendedConfigCache::default()));
        let host: Arc<dyn compiler::CompilerHost> = compiler::new_compiler_host(
            self.sys.get_current_directory(),
            Box::new(self.sys.clone()),
            self.sys.default_library_path(),
            Some(Box::new(Arc::clone(
                self.extended_config_cache.as_ref().unwrap(),
            ))),
            crate::command_line::get_trace_from_sys(
                self.sys.clone(),
                self.config.locale(),
                self.testing.clone(),
            ),
        )
        .into();
        self.program = incremental::read_build_info_program(
            &self.config,
            &incremental::new_build_info_reader(Arc::clone(&host)),
            host.as_ref(),
        );

        if !self.config_file_name.is_empty() {
            self.config_file_paths = vec![self.config_file_name.clone()];
            self.config_file_paths
                .extend(self.config.extended_source_files().iter().cloned());
        }

        if !self
            .sys
            .get_environment_variable("TS_WATCH_DEBUG")
            .is_empty()
        {
            self.file_watcher
                .set_debug_log(Some(Box::new(self.sys.clone())));
        }

        (self.report_watch_status)(ast::new_compiler_diagnostic(
            &diagnostics::Starting_compilation_in_watch_mode,
            &[],
        ));
        self.do_build();

        if self.testing.is_none() {
            loop {
                if self.file_watcher.poll_once(|| self.sys.now()) {
                    self.do_cycle();
                }
            }
        }
    }

    pub fn do_cycle(&mut self) {
        // PORT NOTE: reshaped for borrowck; clone the mutex handle so the guard
        // does not borrow `self` while rechecking/building mutates fields.
        let mu = Arc::clone(&self.mu);
        let _lock = mu.lock().unwrap_or_else(|err| err.into_inner());
        if self.recheck_ts_config() {
            return;
        }
        if !self.file_watcher.watch_state_uninitialized()
            && !self.config_modified
            && !self.file_watcher.has_changes_from_watch_state()
        {
            if let Some(testing) = &self.testing {
                testing.on_program(self.program.as_ref().unwrap());
            }
            return;
        }

        (self.report_watch_status)(ast::new_compiler_diagnostic(
            &diagnostics::File_change_detected_Starting_incremental_compilation,
            &[],
        ));
        self.do_build();
    }

    pub fn do_build(&mut self) {
        if self.config_modified {
            self.source_file_cache = Arc::new(collections::SyncMap::default());
        }

        let cached = Arc::new(cachedvfs::CachedFs::from(Arc::new(self.sys.clone())));
        let tfs = Arc::new(trackingvfs::TrackingFs::new(Arc::clone(&cached)));
        let inner_host = compiler::new_compiler_host(
            self.sys.get_current_directory(),
            Box::new(Arc::clone(&tfs)),
            self.sys.default_library_path(),
            Some(Box::new(Arc::clone(
                self.extended_config_cache.as_ref().unwrap(),
            ))),
            crate::command_line::get_trace_from_sys(
                self.sys.clone(),
                self.config.locale(),
                self.testing.clone(),
            ),
        );
        let host: Arc<dyn compiler::CompilerHost> = Arc::new(WatchCompilerHost {
            compiler_host: inner_host,
            cache: self.source_file_cache.clone(),
        });

        let mut wildcard_dirs = BTreeMap::new();
        if self.config.config_file.is_some() {
            wildcard_dirs = self.config.wildcard_directories().clone();
            for dir in wildcard_dirs.keys() {
                tfs.seen_files
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .insert(dir.clone());
            }
            if !wildcard_dirs.is_empty() {
                self.config = self
                    .config
                    .reload_file_names_of_parsed_command_line(self.sys.fs());
            }
        }
        for path in &self.config_file_paths {
            tfs.seen_files
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .insert(path.clone());
        }

        self.program = Some(incremental::new_program(
            compiler::new_program(crate::command_line::program_options(
                &self.config,
                Arc::clone(&host),
                None,
            )),
            self.program.as_ref(),
            incremental::create_host(Arc::clone(&host)),
            self.testing.is_some(),
        ));

        let result = self.compile_and_emit();
        cached.disable_and_clear_cache();
        self.file_watcher.update_watch_state(
            &tfs.seen_files
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            &wildcard_dirs,
        );
        self.file_watcher
            .set_poll_interval(self.config.watch_options().watch_interval());
        self.config_modified = false;

        let program_files = self.program.as_ref().unwrap().get_program().files_by_path();
        let mut stale_paths = Vec::new();
        self.source_file_cache.range(|path, _| {
            if !program_files.contains_key(path) {
                stale_paths.push(path.clone());
            }
            true
        });
        for path in stale_paths {
            self.source_file_cache.delete(&path);
        }

        let error_count = result.diagnostics.len();
        if error_count == 1 {
            (self.report_watch_status)(ast::new_compiler_diagnostic(
                &diagnostics::Found_1_error_Watching_for_file_changes,
                &[],
            ));
        } else {
            (self.report_watch_status)(ast::new_compiler_diagnostic(
                &diagnostics::Found_0_errors_Watching_for_file_changes,
                &[Box::new(error_count) as diagnostics::Argument],
            ));
        }

        if let Some(testing) = &self.testing {
            testing.on_program(self.program.as_ref().unwrap());
        }
    }

    pub fn compile_and_emit(&mut self) -> tsc::CompileAndEmitResult {
        tsc::emit_files_and_report_errors(tsc::EmitInput {
            sys: self.sys.clone(),
            program_like: self.program.as_mut().unwrap(),
            config: self.config.clone(),
            report_diagnostic: Arc::clone(&self.report_diagnostic),
            report_error_summary: Arc::clone(&self.report_error_summary),
            writer: Box::new(self.sys.clone()),
            write_file: None,
            compile_times: tsc::CompileTimes::default(),
            testing: self.testing.clone(),
            testing_m_times_cache: None,
            tracing: None,
        })
    }

    pub fn recheck_ts_config(&mut self) -> bool {
        if self.config_file_name.is_empty() {
            return false;
        }

        if !self.config_has_errors && !self.config_file_paths.is_empty() {
            let mut changed = false;
            for path in &self.config_file_paths {
                let Some(old) = self.file_watcher.watch_state_entry(path) else {
                    changed = true;
                    break;
                };
                let s = self.sys.fs().stat(path).ok();
                if !old.exists {
                    if s.is_some() {
                        changed = true;
                        break;
                    }
                } else if s.as_ref().and_then(|info| info.mod_time()) != old.mod_time {
                    changed = true;
                    break;
                }
            }
            if !changed {
                return false;
            }
        }

        let extended_config_cache = tsc::ExtendedConfigCache::default();
        let (config_parse_result, errors) = tsoptions::get_parsed_command_line_of_config_file(
            &self.config_file_name,
            Some(&self.compiler_options_from_command_line),
            None,
            &self.sys,
            Some(&extended_config_cache),
        );
        if !errors.is_empty() {
            for e in &errors {
                (self.report_diagnostic)(crate::command_line::command_line_error_diagnostic(
                    e.clone(),
                ));
            }
            self.config_has_errors = true;
            let error_count = errors.len();
            if error_count == 1 {
                (self.report_watch_status)(ast::new_compiler_diagnostic(
                    &diagnostics::Found_1_error_Watching_for_file_changes,
                    &[],
                ));
            } else {
                (self.report_watch_status)(ast::new_compiler_diagnostic(
                    &diagnostics::Found_0_errors_Watching_for_file_changes,
                    &[Box::new(error_count) as diagnostics::Argument],
                ));
            }
            return true;
        }
        if self.config_has_errors {
            self.config_modified = true;
        }
        let mut config_parse_result = config_parse_result.unwrap_or_default();
        crate::command_line::apply_command_line_watch_options(
            &mut config_parse_result,
            &self.watch_options_from_command_line,
        );
        self.config_has_errors = false;
        self.config_file_paths = vec![self.config_file_name.clone()];
        self.config_file_paths
            .extend(config_parse_result.extended_source_files().iter().cloned());
        if self.config.raw != config_parse_result.raw
            || self.config.options != config_parse_result.options
            || self.config.watch_options != config_parse_result.watch_options
            || self.config.file_names != config_parse_result.file_names
        {
            self.config_modified = true;
        }
        self.config = config_parse_result;
        self.extended_config_cache = Some(Arc::new(extended_config_cache));
        false
    }
}

impl tsc::Watcher for Watcher {
    fn do_cycle(&mut self) {
        Watcher::do_cycle(self);
    }
}
