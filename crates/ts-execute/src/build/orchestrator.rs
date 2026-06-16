use std::io::{Result as IoResult, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use ts_ast as ast;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::tsc;

use super::buildtask::{BuildTask, BuildTaskHandle, lock_task};
use super::host::{Host, SyncMap};

struct SystemWriter(tsc::System);

impl Write for SystemWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.writer().write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.0.writer().flush()
    }
}

pub struct Options {
    pub sys: tsc::System,
    pub command: tsoptions::ParsedBuildCommandLine,
    pub testing: Option<tsc::CommandLineTesting>,
}

pub struct OrchestratorResult {
    pub result: tsc::CommandLineResult,
    pub errors: Vec<ast::Diagnostic>,
    pub statistics: tsc::Statistics,
    pub files_to_delete: Vec<String>,
}

impl OrchestratorResult {
    pub fn report(&mut self, o: &mut Orchestrator) {
        if o.opts.command.compiler_options.watch.is_true() {
            let args: Vec<diagnostics::Argument> = vec![Box::new(self.errors.len())];
            let message: &diagnostics::Message = if self.errors.len() == 1 {
                &diagnostics::Found_1_error_Watching_for_file_changes
            } else {
                &diagnostics::Found_0_errors_Watching_for_file_changes
            };
            (o.watch_status_reporter)(ast::new_compiler_diagnostic(message, &args));
        } else {
            (o.error_summary_reporter)(self.errors.clone());
        }
        if !self.files_to_delete.is_empty() {
            let args: Vec<diagnostics::Argument> = vec![Box::new(
                self.files_to_delete
                    .iter()
                    .map(|f| format!("\r\n * {f}"))
                    .collect::<Vec<_>>()
                    .join(""),
            )];
            (o.create_builder_status_reporter(None))(ast::new_compiler_diagnostic(
                &diagnostics::A_non_dry_build_would_delete_the_following_files_Colon_0,
                &args,
            ))
        }
        if !o.opts.command.compiler_options.diagnostics.is_true()
            && !o
                .opts
                .command
                .compiler_options
                .extended_diagnostics
                .is_true()
        {
            return;
        }
        self.statistics.set_total_time(o.opts.sys.since_start());
        self.statistics.report(
            Box::new(SystemWriter(o.opts.sys.clone())),
            o.opts.testing.clone(),
        );
    }
}

pub struct Orchestrator {
    pub opts: Options,
    pub compare_paths_options: tspath::ComparePathsOptions,
    pub host: Arc<Host<Box<dyn ts_compiler::CompilerHost>>>,

    // order generation result
    pub tasks: SyncMap<tspath::Path, BuildTaskHandle>,
    pub order: Vec<String>,
    pub errors: Vec<ast::Diagnostic>,

    pub error_summary_reporter: tsc::DiagnosticsReporter,
    pub watch_status_reporter: tsc::DiagnosticReporter,
}

impl Orchestrator {
    pub fn relative_file_name(&self, file_name: &str) -> String {
        tspath::convert_to_relative_path(file_name, &self.compare_paths_options)
    }

    pub fn to_path(&self, file_name: &str) -> tspath::Path {
        tspath::to_path(
            file_name,
            &self.compare_paths_options.current_directory,
            self.compare_paths_options.use_case_sensitive_file_names,
        )
    }

    pub fn order(&self) -> Vec<String> {
        self.order.clone()
    }

    pub fn upstream(&self, config_name: &str) -> Vec<String> {
        let path = self.to_path(config_name);
        let task = self.get_task(path);
        lock_task(&task)
            .up_stream
            .clone()
            .iter()
            .map(|t| lock_task(&t.task).config.clone())
            .collect()
    }

    pub fn downstream(&self, config_name: &str) -> Vec<String> {
        let path = self.to_path(config_name);
        let task = self.get_task(path);
        lock_task(&task)
            .down_stream
            .clone()
            .iter()
            .map(|t| lock_task(t).config.clone())
            .collect()
    }

    pub fn get_task(&self, path: tspath::Path) -> BuildTaskHandle {
        let (task, ok) = self.tasks.load(&path);
        if !ok {
            panic!("No build task found for {path}");
        }
        task.unwrap()
    }

    pub fn create_build_tasks(
        &mut self,
        old_tasks: Option<&SyncMap<tspath::Path, BuildTaskHandle>>,
        configs: Vec<String>,
        wg: &dyn core::WorkGroup,
    ) {
        let _ = wg;
        for config in configs {
            // PORT NOTE: reshaped for borrowck/thread-safety; the Rust port's
            // task graph contains non-Send compiler state, so config task
            // creation currently runs inline while preserving Go control flow.
            {
                let path = self.to_path(&config);
                let mut task = None;
                let mut build_info = None;
                if let Some(old_tasks) = old_tasks {
                    let (existing, ok) = old_tasks.load(&path);
                    if let (Some(existing), true) = (existing, ok) {
                        if !lock_task(&existing).dirty {
                            // Reuse existing task if config is same
                            task = Some(existing);
                        } else {
                            build_info = lock_task(&existing).build_info_entry.clone();
                        }
                    }
                }
                if task.is_none() {
                    let new_task = Arc::new(std::sync::Mutex::new(BuildTask::new(
                        config.clone(),
                        old_tasks.is_none(),
                    )));
                    {
                        let mut new_task = lock_task(&new_task);
                        new_task.pending.store(true, Ordering::SeqCst);
                        new_task.set_build_info_entry(build_info);
                    }
                    task = Some(new_task);
                }
                let task = task.unwrap();
                let (_, loaded) = self.tasks.load_or_store(path.clone(), Some(task.clone()));
                if loaded {
                    continue;
                }
                let mut task_guard = lock_task(&task);
                task_guard.set_resolved(
                    self.host
                        .get_resolved_project_reference(config.clone(), path.clone()),
                );
                task_guard.clear_up_stream();
                let referenced_configs = task_guard.resolved().map(|resolved| {
                    resolved
                        .project_references()
                        .iter()
                        .map(core::resolve_project_reference_path)
                        .collect::<Vec<_>>()
                });
                drop(task_guard);
                if let Some(referenced_configs) = referenced_configs {
                    self.create_build_tasks(old_tasks, referenced_configs, wg);
                }
            }
        }
    }

    pub fn setup_build_task(
        &mut self,
        config_name: String,
        down_stream: Option<BuildTaskHandle>,
        in_circular_context: bool,
        completed: &mut core::Set<tspath::Path>,
        analyzing: &mut core::Set<tspath::Path>,
        mut circularity_stack: Vec<String>,
    ) -> Option<BuildTaskHandle> {
        let path = self.to_path(&config_name);
        let task = self.get_task(path.clone());
        if !completed.has(&path) {
            if analyzing.has(&path) {
                if !in_circular_context {
                    let args: Vec<diagnostics::Argument> =
                        vec![Box::new(circularity_stack.join("\n"))];
                    self.errors.push(ast::new_compiler_diagnostic(
                        &diagnostics::Project_references_may_not_form_a_circular_graph_Cycle_detected_Colon_0,
                        &args,
                    ));
                }
                return None;
            }
            analyzing.add(path.clone());
            circularity_stack.push(config_name.clone());
            let resolved = lock_task(&task).resolved().cloned();
            if let Some(resolved) = resolved {
                let sub_references: Vec<String> = resolved
                    .project_references()
                    .iter()
                    .map(core::resolve_project_reference_path)
                    .collect();
                for (index, sub_reference) in sub_references.iter().enumerate() {
                    let upstream = self.setup_build_task(
                        sub_reference.clone(),
                        Some(task.clone()),
                        in_circular_context || resolved.project_references()[index].circular,
                        completed,
                        analyzing,
                        circularity_stack.clone(),
                    );
                    if let Some(upstream) = upstream {
                        lock_task(&task).push_up_stream(upstream, index);
                    }
                }
            }
            circularity_stack.pop();
            completed.add(path);
            lock_task(&task).reset_report_done();
            let prev = core::last_or_nil(&self.order);
            if !prev.is_empty() {
                lock_task(&task).set_prev_reporter(self.get_task(self.to_path(&prev)));
            }
            lock_task(&task).reset_done();
            self.order.push(config_name.clone());
        }
        if self.opts.command.compiler_options.watch.is_true() {
            if let Some(down_stream) = down_stream {
                lock_task(&task).push_down_stream(down_stream);
            }
        }
        Some(task)
    }

    pub fn generate_graph_reusing_old_tasks(&mut self) {
        let tasks = std::mem::take(&mut self.tasks);
        self.tasks = SyncMap::default();
        self.host.set_tasks(self.tasks.clone());
        self.order = Vec::new();
        self.errors = Vec::new();
        self.generate_graph(Some(tasks));
    }

    pub fn generate_graph(&mut self, old_tasks: Option<SyncMap<tspath::Path, BuildTaskHandle>>) {
        let projects = self.opts.command.resolved_project_paths();
        // Parse all config files in parallel
        let wg = core::new_work_group(self.opts.command.compiler_options.single_threaded.is_true());
        self.create_build_tasks(old_tasks.as_ref(), projects.clone(), wg.as_ref());
        wg.run_and_wait();

        // Generate the graph
        let mut completed = core::Set::default();
        let mut analyzing = core::Set::default();
        let circularity_stack = Vec::new();
        for project in projects {
            self.setup_build_task(
                project,
                None,
                false,
                &mut completed,
                &mut analyzing,
                circularity_stack.clone(),
            );
        }
    }

    pub fn start(mut self) -> tsc::CommandLineResult {
        if self.opts.command.compiler_options.watch.is_true() {
            (self.watch_status_reporter)(ast::new_compiler_diagnostic(
                &diagnostics::Starting_compilation_in_watch_mode,
                &[],
            ));
        }
        self.generate_graph(None);
        let mut result = self.build_or_clean();
        if self.opts.command.compiler_options.watch.is_true() {
            self.watch();
            result.watcher = Some(Box::new(self));
        }
        result
    }

    pub fn watch(&mut self) {
        self.update_watch();
        self.reset_caches();

        // Start watching for file changes
        if self.opts.testing.is_none() {
            let watch_interval = self.opts.command.watch_options.watch_interval();
            loop {
                // Testing mode: run a single cycle and exit
                thread::sleep(watch_interval);
                self.do_cycle();
            }
        }
    }

    pub fn update_watch(&mut self) {
        let old_cache = self.host.m_times();
        self.replace_m_times(Arc::new(SyncMap::default()));
        self.range_task(|o, path, task| {
            // PORT NOTE: task update does not need the path argument; Go passes
            // only the old mtimes cache here.
            let _ = path;
            lock_task(&task).update_watch(o, &old_cache);
        });
    }

    pub fn reset_caches(&mut self) {
        // Clean out all the caches
        self.host.clear_fs_cache();
        self.host.reset_extended_config_cache();
        self.host.source_files.reset();
        self.replace_config_times(SyncMap::default());
    }

    pub fn do_cycle(&mut self) {
        let needs_config_update = AtomicBool::new(false);
        let needs_update = AtomicBool::new(false);
        let m_times = self.host.m_times().clone_map();
        self.range_task(|o, path, task| {
            let update_kind = lock_task(&task).has_update(o, path);
            if update_kind != super::buildtask::UPDATE_KIND_NONE {
                needs_update.store(true, Ordering::SeqCst);
                if update_kind == super::buildtask::UPDATE_KIND_CONFIG {
                    needs_config_update.store(true, Ordering::SeqCst);
                }
            }
        });

        if !needs_update.load(Ordering::SeqCst) {
            self.replace_m_times(m_times);
            self.reset_caches();
            return;
        }

        (self.watch_status_reporter)(ast::new_compiler_diagnostic(
            &diagnostics::File_change_detected_Starting_incremental_compilation,
            &[],
        ));
        if needs_config_update.load(Ordering::SeqCst) {
            // Generate new tasks
            self.generate_graph_reusing_old_tasks();
        }

        self.build_or_clean();
        self.update_watch();
        self.reset_caches();
    }

    pub fn build_or_clean(&mut self) -> tsc::CommandLineResult {
        if !self.opts.command.build_options.clean.is_true()
            && self.opts.command.build_options.verbose.is_true()
        {
            let args: Vec<diagnostics::Argument> = vec![Box::new(
                self.order()
                    .iter()
                    .map(|p| format!("\r\n    * {}", self.relative_file_name(p)))
                    .collect::<Vec<_>>()
                    .join(""),
            )];
            (self.create_builder_status_reporter(None))(ast::new_compiler_diagnostic(
                &diagnostics::Projects_in_this_build_Colon_0,
                &args,
            ));
        }
        let mut build_result = OrchestratorResult::default();
        if self.errors.is_empty() {
            build_result.statistics.projects = self.order().len();
            self.range_task(|o, path, task| {
                o.build_or_clean_project(task, path, &mut build_result);
            });
        } else {
            // Circularity errors prevent any project from being built
            build_result.result.status = tsc::EXIT_STATUS_PROJECT_REFERENCE_CYCLE_OUTPUTS_SKIPPED;
            let report_diagnostic = self.create_diagnostic_reporter(None);
            for err in &self.errors {
                report_diagnostic(err.clone());
            }
            build_result.errors = self.errors.clone();
        }
        build_result.report(self);
        build_result.result
    }

    pub fn range_task<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Orchestrator, tspath::Path, BuildTaskHandle),
    {
        let mut num_routines = 4;
        if self.opts.command.compiler_options.single_threaded.is_true() {
            num_routines = 1;
        } else if let Some(builders) = self.opts.command.build_options.builders {
            num_routines = builders as usize;
        }

        let _ = num_routines;
        for config in self.order.clone() {
            let path = self.to_path(&config);
            let task = self.get_task(path.clone());
            f(self, path, task);
        }
    }

    pub fn build_or_clean_project(
        &mut self,
        task: BuildTaskHandle,
        path: tspath::Path,
        build_result: &mut OrchestratorResult,
    ) {
        let mut task_mut = lock_task(&task);
        task_mut.reset_result();
        let report_status = tsc::create_builder_status_reporter(
            self.opts.sys.clone(),
            task_mut.result_writer(),
            self.opts.command.locale(),
            self.opts.command.compiler_options.clone(),
            self.opts.testing.clone(),
        );
        let diagnostic_reporter = tsc::create_diagnostic_reporter(
            self.opts.sys.clone(),
            task_mut.result_writer(),
            self.opts.command.locale(),
            self.opts.command.compiler_options.clone(),
        );
        task_mut.set_report_status(report_status);
        task_mut.set_diagnostic_reporter(diagnostic_reporter);
        if !self.opts.command.build_options.clean.is_true() {
            task_mut.build_project(self, path.clone());
        } else {
            task_mut.clean_project(self, path.clone());
        }
        task_mut.report(self, path, build_result);
    }

    pub fn get_writer(&mut self, task: Option<BuildTaskHandle>) -> Box<dyn Write + Send> {
        if let Some(task) = task {
            return lock_task(&task).result_writer();
        }
        Box::new(SystemWriter(self.opts.sys.clone()))
    }

    pub fn create_builder_status_reporter(
        &mut self,
        task: Option<BuildTaskHandle>,
    ) -> tsc::DiagnosticReporter {
        tsc::create_builder_status_reporter(
            self.opts.sys.clone(),
            self.get_writer(task),
            self.opts.command.locale(),
            self.opts.command.compiler_options.clone(),
            self.opts.testing.clone(),
        )
    }

    pub fn create_diagnostic_reporter(
        &mut self,
        task: Option<BuildTaskHandle>,
    ) -> tsc::DiagnosticReporter {
        tsc::create_diagnostic_reporter(
            self.opts.sys.clone(),
            self.get_writer(task),
            self.opts.command.locale(),
            self.opts.command.compiler_options.clone(),
        )
    }

    fn replace_m_times(&mut self, m_times: Arc<SyncMap<tspath::Path, SystemTime>>) {
        self.host.replace_m_times(m_times);
    }

    fn replace_config_times(&mut self, config_times: SyncMap<tspath::Path, Duration>) {
        self.host.replace_config_times(config_times);
    }
}

impl Default for OrchestratorResult {
    fn default() -> Self {
        Self {
            result: tsc::CommandLineResult::default(),
            errors: Vec::new(),
            statistics: tsc::Statistics::default(),
            files_to_delete: Vec::new(),
        }
    }
}

impl tsc::Watcher for Orchestrator {
    fn do_cycle(&mut self) {
        Orchestrator::do_cycle(self);
    }
}

pub fn new_orchestrator(opts: Options) -> Orchestrator {
    let current_directory = opts.sys.get_current_directory();
    let default_library_path = opts.sys.default_library_path();
    let sys = opts.sys.clone();
    let compare_paths_options = tspath::ComparePathsOptions {
        current_directory: current_directory.clone(),
        use_case_sensitive_file_names: opts.sys.fs().use_case_sensitive_file_names(),
    };
    let tasks = SyncMap::default();
    let cached_fs = Arc::new(vfs::cachedvfs::CachedFs::from(Arc::new(sys)));
    let host = Arc::new(Host::new(
        opts.sys.clone(),
        opts.command.clone(),
        compare_paths_options.clone(),
        tasks.clone(),
        ts_compiler::new_compiler_host(
            current_directory,
            Box::new(Arc::clone(&cached_fs)),
            default_library_path,
            None,
            None,
        ),
        cached_fs,
        Arc::new(SyncMap::default()),
    ));
    let mut orchestrator = Orchestrator {
        opts,
        compare_paths_options,
        host,
        tasks,
        order: Vec::new(),
        errors: Vec::new(),
        error_summary_reporter: Arc::new(|_| {}),
        watch_status_reporter: Arc::new(|_| {}),
    };
    if orchestrator.opts.command.compiler_options.watch.is_true() {
        orchestrator.watch_status_reporter = tsc::create_watch_status_reporter(
            orchestrator.opts.sys.clone(),
            orchestrator.opts.command.locale(),
            orchestrator.opts.command.compiler_options.clone(),
            orchestrator.opts.testing.clone(),
        );
    } else {
        orchestrator.error_summary_reporter = tsc::create_report_error_summary(
            orchestrator.opts.sys.clone(),
            orchestrator.opts.command.locale(),
            orchestrator.opts.command.compiler_options.clone(),
        );
    }
    orchestrator
}
