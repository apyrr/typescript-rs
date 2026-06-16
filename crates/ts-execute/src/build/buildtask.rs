use std::cell::RefCell;
use std::fmt;
use std::io::{self, Write};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::SystemTime;

use ts_ast as ast;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::incremental;
use crate::tsc;

use super::compiler_host::CompilerHost;
use super::host::Host as BuildHost;
use super::orchestrator::{Orchestrator, OrchestratorResult};
use super::uptodatestatus::*;

type UpdateKind = u32;

pub(crate) const UPDATE_KIND_NONE: UpdateKind = 0;
pub(crate) const UPDATE_KIND_CONFIG: UpdateKind = 1;
const UPDATE_KIND_UPDATE: UpdateKind = 2;

type BuildKind = u32;

const BUILD_KIND_NONE: BuildKind = 0;
const BUILD_KIND_PSEUDO: BuildKind = 1;
const BUILD_KIND_PROGRAM: BuildKind = 2;

pub(crate) type BuildTaskHandle = Arc<Mutex<BuildTask>>;

pub(crate) fn lock_task(task: &BuildTaskHandle) -> std::sync::MutexGuard<'_, BuildTask> {
    task.lock().unwrap_or_else(|err| err.into_inner())
}

#[derive(Clone)]
pub(crate) struct UpstreamTask {
    pub(crate) task: BuildTaskHandle,
    pub(crate) ref_index: usize,
}

#[derive(Clone)]
pub(crate) struct BuildInfoEntry {
    build_info: Option<incremental::BuildInfo>,
    path: tspath::Path,
    m_time: SystemTime,
    dts_time: Option<SystemTime>,
}

struct CachedBuildInfoReader {
    build_info: Option<incremental::BuildInfo>,
}

impl incremental::BuildInfoReader for CachedBuildInfoReader {
    fn read_build_info(
        &self,
        _config: &tsoptions::ParsedCommandLine,
    ) -> Option<incremental::BuildInfo> {
        self.build_info.clone()
    }
}

struct BuildInfoEmit {
    file_name: String,
    build_info: incremental::BuildInfo,
    m_time: SystemTime,
}

#[derive(Default)]
struct BuildTaskWriteState {
    build_info_emit: Option<BuildInfoEmit>,
}

#[derive(Clone, Default)]
struct SharedBuilder(Arc<Mutex<String>>);

impl Write for SharedBuilder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        self.0
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push_str(&text);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Display for SharedBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.lock().unwrap_or_else(|err| err.into_inner()))
    }
}

struct TaskResult {
    pub(crate) builder: SharedBuilder,
    pub(crate) report_status: tsc::DiagnosticReporter,
    pub(crate) diagnostic_reporter: tsc::DiagnosticReporter,
    pub(crate) exit_status: tsc::ExitStatus,
    pub(crate) statistics: Option<tsc::Statistics>,
    pub(crate) program: Option<incremental::Program>,
    pub(crate) build_kind: BuildKind,
    pub(crate) files_to_delete: Vec<String>,
}

pub struct BuildTask {
    pub(crate) config: String,
    pub(crate) resolved: Option<tsoptions::ParsedCommandLine>,
    pub(crate) up_stream: Vec<UpstreamTask>,
    pub(crate) down_stream: Vec<BuildTaskHandle>, // Only set and used in watch mode
    pub(crate) status: Option<UpToDateStatus>,
    pub(crate) done: Arc<(Mutex<bool>, Condvar)>,

    // task reporting
    pub(crate) result: Option<TaskResult>,
    pub(crate) prev_reporter: Option<BuildTaskHandle>,
    pub(crate) report_done: Arc<(Mutex<bool>, Condvar)>,

    // Watching things
    pub(crate) config_time: SystemTime,
    pub(crate) extended_config_times: Vec<SystemTime>,
    pub(crate) input_files: Vec<SystemTime>,

    pub(crate) build_info_entry: Option<BuildInfoEntry>,
    pub(crate) build_info_entry_mu: Mutex<()>,

    pub(crate) errors: Vec<ast::Diagnostic>,
    pub(crate) pending: AtomicBool,
    pub(crate) is_initial_cycle: bool,
    pub(crate) down_stream_update_mu: Mutex<()>,
    pub(crate) dirty: bool,
}

impl BuildTask {
    pub fn new(config: String, is_initial_cycle: bool) -> Self {
        Self {
            config,
            resolved: None,
            up_stream: Vec::new(),
            down_stream: Vec::new(),
            status: None,
            done: Arc::new((Mutex::new(false), Condvar::new())),
            result: None,
            prev_reporter: None,
            report_done: Arc::new((Mutex::new(false), Condvar::new())),
            config_time: SystemTime::UNIX_EPOCH,
            extended_config_times: Vec::new(),
            input_files: Vec::new(),
            build_info_entry: None,
            build_info_entry_mu: Mutex::new(()),
            errors: Vec::new(),
            pending: AtomicBool::new(false),
            is_initial_cycle,
            down_stream_update_mu: Mutex::new(()),
            dirty: false,
        }
    }

    pub fn set_build_info_entry(&mut self, build_info_entry: Option<BuildInfoEntry>) {
        self.build_info_entry = build_info_entry;
    }

    pub fn set_resolved(&mut self, resolved: Option<tsoptions::ParsedCommandLine>) {
        self.resolved = resolved;
    }

    pub fn resolved(&self) -> Option<&tsoptions::ParsedCommandLine> {
        self.resolved.as_ref()
    }

    pub fn clear_up_stream(&mut self) {
        self.up_stream.clear();
    }

    pub fn push_up_stream(&mut self, task: BuildTaskHandle, ref_index: usize) {
        self.up_stream.push(UpstreamTask { task, ref_index });
    }

    pub fn push_down_stream(&mut self, task: BuildTaskHandle) {
        self.down_stream.push(task);
    }

    pub fn reset_report_done(&mut self) {
        self.report_done = Arc::new((Mutex::new(false), Condvar::new()));
    }

    pub fn reset_done(&mut self) {
        self.done = Arc::new((Mutex::new(false), Condvar::new()));
    }

    pub fn set_prev_reporter(&mut self, task: BuildTaskHandle) {
        self.prev_reporter = Some(task);
    }

    pub fn reset_result(&mut self) {
        self.result = Some(TaskResult {
            builder: SharedBuilder::default(),
            report_status: std::sync::Arc::new(tsc::quiet_diagnostic_reporter),
            diagnostic_reporter: std::sync::Arc::new(tsc::quiet_diagnostic_reporter),
            exit_status: tsc::EXIT_STATUS_SUCCESS,
            statistics: None,
            program: None,
            build_kind: BUILD_KIND_NONE,
            files_to_delete: Vec::new(),
        });
    }

    pub fn set_report_status(&mut self, report_status: tsc::DiagnosticReporter) {
        if self.result.is_none() {
            self.reset_result();
        }
        self.result.as_mut().unwrap().report_status = report_status;
    }

    pub fn set_diagnostic_reporter(&mut self, diagnostic_reporter: tsc::DiagnosticReporter) {
        if self.result.is_none() {
            self.reset_result();
        }
        self.result.as_mut().unwrap().diagnostic_reporter = diagnostic_reporter;
    }

    pub fn result_writer(&self) -> Box<dyn Write + Send> {
        Box::new(self.result.as_ref().unwrap().builder.clone())
    }

    fn wait_on_upstream(&self) {
        for upstream in &self.up_stream {
            let done_pair = { lock_task(&upstream.task).done.clone() };
            let (lock, cvar) = &*done_pair;
            let mut done = lock.lock().unwrap();
            while !*done {
                done = cvar.wait(done).unwrap();
            }
        }
    }

    fn unblock_downstream(&mut self) {
        self.pending.store(false, Ordering::SeqCst);
        self.is_initial_cycle = false;
        let (lock, cvar) = &*self.done;
        *lock.lock().unwrap() = true;
        cvar.notify_all();
    }

    fn report_diagnostic(&mut self, err: ast::Diagnostic) {
        self.errors.push(err.clone());
        if let Some(result) = &mut self.result {
            (result.diagnostic_reporter)(err);
        }
    }

    pub fn report(
        &mut self,
        orchestrator: &mut Orchestrator,
        _config_path: tspath::Path,
        build_result: &mut OrchestratorResult,
    ) {
        if let Some(prev_reporter) = &self.prev_reporter {
            let report_done = { lock_task(prev_reporter).report_done.clone() };
            let (lock, cvar) = &*report_done;
            let mut done = lock.lock().unwrap();
            while !*done {
                done = cvar.wait(done).unwrap();
            }
        }
        if !self.errors.is_empty() {
            build_result.errors.extend(self.errors.clone());
        }
        if let Some(result) = &self.result {
            let _ = write!(orchestrator.opts.sys.writer(), "{}", result.builder);
            if result.exit_status > build_result.result.status {
                build_result.result.status = result.exit_status;
            }
            if let Some(statistics) = &result.statistics {
                build_result.statistics.aggregate(statistics);
            }
            // If we built the program, or updated timestamps, or had errors, we need to
            // delete files that are no longer needed
            match result.build_kind {
                BUILD_KIND_PROGRAM => {
                    if let Some(testing) = &orchestrator.opts.testing {
                        testing.on_program(result.program.as_ref().unwrap());
                    }
                    build_result.statistics.projects_built += 1;
                }
                BUILD_KIND_PSEUDO => {
                    build_result.statistics.timestamp_updates += 1;
                }
                _ => {}
            }
            build_result
                .files_to_delete
                .extend(result.files_to_delete.clone());
        }
        self.result = None;
        let (lock, cvar) = &*self.report_done;
        *lock.lock().unwrap() = true;
        cvar.notify_all();
    }

    pub fn build_project(&mut self, orchestrator: &mut Orchestrator, path: tspath::Path) {
        // Wait on upstream tasks to complete
        self.wait_on_upstream();
        if self.pending.load(Ordering::SeqCst) {
            self.status = Some(self.get_up_to_date_status(orchestrator, path.clone()));
            self.report_up_to_date_status(orchestrator);
            if !self.handle_status_that_doesnt_require_build(orchestrator) {
                self.compile_and_emit(orchestrator, path.clone());
                self.update_downstream(orchestrator, path);
            } else {
                if let Some(resolved) = &self.resolved {
                    let mut diagnostics = resolved.get_config_file_parsing_ast_diagnostics();
                    let mut encoded_diagnostics =
                        if let Some(config_file) = resolved.config_file.as_ref() {
                            config_file
                                .diagnostics
                                .iter()
                                .filter(|message| {
                                    !diagnostics.iter().any(|diagnostic| {
                                        diagnostic.to_string() == message.as_str()
                                    })
                                })
                                .cloned()
                                .collect()
                        } else {
                            Vec::new()
                        };
                    encoded_diagnostics.extend(resolved.errors.clone());
                    diagnostics.extend(
                        encoded_diagnostics
                            .into_iter()
                            .map(compiler::config_file_parsing_diagnostic),
                    );
                    for diagnostic in diagnostics {
                        self.report_diagnostic(diagnostic);
                    }
                }
                if !self.errors.is_empty() {
                    self.result.as_mut().unwrap().exit_status =
                        tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED;
                }
            }
        } else if !self.errors.is_empty() {
            self.report_up_to_date_status(orchestrator);
            for err in self.errors.clone() {
                // Should not add the diagnostics so just reporting
                (self.result.as_ref().unwrap().diagnostic_reporter)(err);
            }
        }
        self.unblock_downstream();
    }

    fn update_downstream(&mut self, orchestrator: &Orchestrator, path: tspath::Path) {
        if self.is_initial_cycle {
            return;
        }
        if orchestrator
            .opts
            .command
            .build_options
            .stop_build_on_errors
            .is_true()
            && self.status.as_ref().is_some_and(UpToDateStatus::is_error)
        {
            return;
        }

        for down_stream in &self.down_stream {
            let mut down_stream_mut = lock_task(down_stream);
            if let Some(status) = down_stream_mut.status.clone() {
                match status.kind {
                    UP_TO_DATE_STATUS_TYPE_UP_TO_DATE => {
                        if !self
                            .result
                            .as_ref()
                            .unwrap()
                            .program
                            .as_ref()
                            .unwrap()
                            .has_changed_dts_file()
                        {
                            down_stream_mut.status = Some(UpToDateStatus {
                                kind: UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_UPSTREAM_TYPES,
                                data: status.data.clone(),
                            });
                        } else {
                            down_stream_mut.status = Some(UpToDateStatus {
                                kind: UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER,
                                data: StatusData::InputOutputName(InputOutputName {
                                    input: self.config.clone(),
                                    output: status.oldest_output_file_name(),
                                }),
                            });
                        }
                    }
                    UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_UPSTREAM_TYPES
                    | UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_INPUT_FILE_TEXT => {
                        if self
                            .result
                            .as_ref()
                            .unwrap()
                            .program
                            .as_ref()
                            .unwrap()
                            .has_changed_dts_file()
                        {
                            down_stream_mut.status = Some(UpToDateStatus {
                                kind: UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER,
                                data: StatusData::InputOutputName(InputOutputName {
                                    input: self.config.clone(),
                                    output: status.oldest_output_file_name(),
                                }),
                            });
                        }
                    }
                    UP_TO_DATE_STATUS_TYPE_UPSTREAM_ERRORS => {
                        let upstream_errors = status.upstream_errors();
                        let ref_config = core::resolve_config_file_name_of_project_reference(
                            &upstream_errors.r#ref,
                        );
                        if orchestrator.to_path(&ref_config) == path {
                            down_stream_mut.reset_status();
                        }
                    }
                    _ => {}
                }
            }
            down_stream_mut.pending.store(true, Ordering::SeqCst);
        }
    }

    fn compile_and_emit(&mut self, orchestrator: &mut Orchestrator, path: tspath::Path) {
        self.errors.clear();
        if orchestrator.opts.command.build_options.verbose.is_true() {
            self.result
                .as_mut()
                .unwrap()
                .report_status(ast::new_compiler_diagnostic(
                    &diagnostics::Building_project_0,
                    &[Box::new(orchestrator.relative_file_name(&self.config))
                        as diagnostics::Argument],
                ));
        }

        // Real build
        let mut compile_times = tsc::CompileTimes::default();
        compile_times.config_time = orchestrator.host.config_time(&path);
        let build_info_read_start = orchestrator.opts.sys.now();
        let (cached_build_info, _) = self.load_or_store_build_info(
            orchestrator.host.as_ref(),
            &self.resolved.as_ref().unwrap().get_build_info_file_name(),
        );
        let cached_build_info_reader = CachedBuildInfoReader {
            build_info: cached_build_info,
        };
        let old_program = if !orchestrator.opts.command.build_options.force.is_true() {
            incremental::read_build_info_program(
                self.resolved.as_ref().unwrap(),
                &cached_build_info_reader,
                orchestrator.host.as_ref(),
            )
        } else {
            None
        };
        compile_times.build_info_read_time = orchestrator
            .opts
            .sys
            .now()
            .duration_since(build_info_read_start)
            .unwrap_or_default();
        let parse_start = orchestrator.opts.sys.now();
        let program = compiler::new_program(crate::command_line::program_options(
            self.resolved.as_ref().unwrap(),
            Arc::new(CompilerHost {
                host: orchestrator.host.clone(),
                trace: tsc::get_trace_with_writer_from_sys(
                    Box::new(self.result.as_ref().unwrap().builder.clone()),
                    orchestrator.opts.command.locale(),
                    orchestrator.opts.testing.clone(),
                    false,
                ),
            }),
            None,
        ));
        compile_times.parse_time = orchestrator
            .opts
            .sys
            .now()
            .duration_since(parse_start)
            .unwrap_or_default();
        let changes_compute_start = orchestrator.opts.sys.now();
        let incremental_program = incremental::new_program(
            program,
            old_program.as_ref(),
            incremental::create_host(orchestrator.host.clone()),
            orchestrator.opts.testing.is_some(),
        );
        self.result.as_mut().unwrap().program = Some(incremental_program);
        compile_times.changes_compute_time = orchestrator
            .opts
            .sys
            .now()
            .duration_since(changes_compute_start)
            .unwrap_or_default();

        let diagnostic_reporter = self.result.as_ref().unwrap().diagnostic_reporter.clone();
        let reported_diagnostics = Arc::new(Mutex::new(Vec::new()));
        let report_diagnostic: tsc::DiagnosticReporter = {
            let diagnostic_reporter = diagnostic_reporter.clone();
            let reported_diagnostics = reported_diagnostics.clone();
            Arc::new(move |diagnostic| {
                reported_diagnostics
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push(diagnostic.clone());
                diagnostic_reporter(diagnostic);
            })
        };
        let writer = Box::new(self.result.as_ref().unwrap().builder.clone());
        let write_state = Rc::new(RefCell::new(BuildTaskWriteState::default()));
        let write_file: compiler::WriteFile = {
            let host = orchestrator.host.clone();
            let sys = orchestrator.opts.sys.clone();
            let store_output_time_stamp = self.store_output_time_stamp(orchestrator);
            let write_state = Rc::clone(&write_state);
            Rc::new(RefCell::new(
                move |file_name: &str,
                      text: &str,
                      data: Option<&mut compiler::WriteFileData>|
                      -> Result<(), String> {
                    let err = host
                        .fs()
                        .write_file(file_name, text)
                        .map_err(|err| err.to_string());
                    if err.is_ok() {
                        if let Some(data) = data {
                            if let Some(build_info) =
                                data.build_info.as_ref().and_then(|build_info| {
                                    build_info.downcast_ref::<incremental::BuildInfo>()
                                })
                            {
                                write_state.borrow_mut().build_info_emit = Some(BuildInfoEmit {
                                    file_name: file_name.to_owned(),
                                    build_info: build_info.clone(),
                                    m_time: sys.now(),
                                });
                            } else if store_output_time_stamp {
                                host.store_m_time(file_name, sys.now());
                            }
                        } else if store_output_time_stamp {
                            host.store_m_time(file_name, sys.now());
                        }
                    }
                    err
                },
            ))
        };
        let (result, statistics) = tsc::emit_and_report_statistics(tsc::EmitInput {
            sys: orchestrator.opts.sys.clone(),
            program_like: self.result.as_mut().unwrap().program.as_mut().unwrap(),
            config: self.resolved.clone().unwrap(),
            report_diagnostic,
            report_error_summary: std::sync::Arc::new(tsc::quiet_diagnostics_reporter),
            writer,
            write_file: Some(write_file),
            compile_times,
            testing: orchestrator.opts.testing.clone(),
            testing_m_times_cache: Some(orchestrator.host.m_times()),
            tracing: None,
        });
        if let Some(build_info_emit) = write_state.borrow_mut().build_info_emit.take() {
            let has_changed_dts_file = self
                .result
                .as_ref()
                .unwrap()
                .program
                .as_ref()
                .unwrap()
                .has_changed_dts_file();
            self.on_build_info_emit(
                orchestrator,
                &build_info_emit.file_name,
                build_info_emit.build_info,
                has_changed_dts_file,
                build_info_emit.m_time,
            );
        }
        self.errors.extend(
            reported_diagnostics
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone(),
        );
        self.result.as_mut().unwrap().exit_status = result.status;
        self.result.as_mut().unwrap().statistics = statistics;
        let emitted_files = result
            .emit_result
            .as_ref()
            .map(|emit_result| emit_result.emitted_files.clone())
            .unwrap_or_default();
        if (!self
            .result
            .as_ref()
            .unwrap()
            .program
            .as_ref()
            .unwrap()
            .get_program()
            .options()
            .no_emit_on_error
            .is_true()
            || result.diagnostics.is_empty())
            && (!emitted_files.is_empty()
                || self.status.as_ref().unwrap().kind
                    != UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_BUILD_INFO_WITH_ERRORS)
        {
            // Update time stamps for rest of the outputs
            self.update_time_stamps(
                orchestrator,
                emitted_files.clone(),
                &diagnostics::Updating_unchanged_output_timestamps_of_project_0,
            );
        }
        self.result.as_mut().unwrap().build_kind = BUILD_KIND_PROGRAM;
        if result.status == tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED
            || result.status == tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_GENERATED
        {
            self.status = Some(UpToDateStatus {
                kind: UP_TO_DATE_STATUS_TYPE_BUILD_ERRORS,
                data: StatusData::None,
            });
        } else {
            let oldest_output_file_name = if !emitted_files.is_empty() {
                emitted_files[0].clone()
            } else {
                core::first_or_nil_seq(self.resolved.as_ref().unwrap().get_output_file_names())
            };
            self.status = Some(UpToDateStatus {
                kind: UP_TO_DATE_STATUS_TYPE_UP_TO_DATE,
                data: StatusData::String(oldest_output_file_name),
            });
        }
    }

    fn handle_status_that_doesnt_require_build(&mut self, orchestrator: &mut Orchestrator) -> bool {
        match self.status.as_ref().unwrap().kind {
            UP_TO_DATE_STATUS_TYPE_UP_TO_DATE => {
                if orchestrator.opts.command.build_options.dry.is_true() {
                    self.result
                        .as_mut()
                        .unwrap()
                        .report_status(ast::new_compiler_diagnostic(
                            &diagnostics::Project_0_is_up_to_date,
                            vec![self.config.clone().into()],
                        ));
                }
                return true;
            }
            UP_TO_DATE_STATUS_TYPE_UPSTREAM_ERRORS => {
                let upstream_status = self.status.as_ref().unwrap().upstream_errors();
                if orchestrator.opts.command.build_options.verbose.is_true() {
                    self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                        core::if_else(
                            upstream_status.ref_has_upstream_errors,
                            &diagnostics::Skipping_build_of_project_0_because_its_dependency_1_was_not_built,
                            &diagnostics::Skipping_build_of_project_0_because_its_dependency_1_has_errors,
                        ),
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(&upstream_status.r#ref).into(),
                        ],
                    ));
                }
                return true;
            }
            UP_TO_DATE_STATUS_TYPE_SOLUTION => return true,
            UP_TO_DATE_STATUS_TYPE_CONFIG_FILE_NOT_FOUND => {
                self.report_diagnostic(ast::new_compiler_diagnostic(
                    &diagnostics::File_0_not_found,
                    vec![self.config.clone().into()],
                ));
                return true;
            }
            _ => {}
        }

        // update timestamps
        if self.status.as_ref().unwrap().is_pseudo_build() {
            if orchestrator.opts.command.build_options.dry.is_true() {
                self.result
                    .as_mut()
                    .unwrap()
                    .report_status(ast::new_compiler_diagnostic(
                    &diagnostics::A_non_dry_build_would_update_timestamps_for_output_of_project_0,
                    vec![self.config.clone().into()],
                ));
                self.status = Some(UpToDateStatus {
                    kind: UP_TO_DATE_STATUS_TYPE_UP_TO_DATE,
                    data: StatusData::None,
                });
                return true;
            }

            self.update_time_stamps(
                orchestrator,
                Vec::new(),
                &diagnostics::Updating_output_timestamps_of_project_0,
            );
            let data = self.status.as_ref().unwrap().data.clone();
            self.status = Some(UpToDateStatus {
                kind: UP_TO_DATE_STATUS_TYPE_UP_TO_DATE,
                data,
            });
            self.result.as_mut().unwrap().build_kind = BUILD_KIND_PSEUDO;
            return true;
        }

        if orchestrator.opts.command.build_options.dry.is_true() {
            self.result
                .as_mut()
                .unwrap()
                .report_status(ast::new_compiler_diagnostic(
                    &diagnostics::A_non_dry_build_would_build_project_0,
                    vec![self.config.clone().into()],
                ));
            self.status = Some(UpToDateStatus {
                kind: UP_TO_DATE_STATUS_TYPE_UP_TO_DATE,
                data: StatusData::None,
            });
            return true;
        }
        false
    }

    fn get_up_to_date_status(
        &mut self,
        orchestrator: &mut Orchestrator,
        config_path: tspath::Path,
    ) -> UpToDateStatus {
        if let Some(status) = &self.status {
            return status.clone();
        }
        // Config file not found
        if self.resolved.is_none() {
            return UpToDateStatus::new(UP_TO_DATE_STATUS_TYPE_CONFIG_FILE_NOT_FOUND);
        }

        // Solution - nothing to build
        if self.resolved.as_ref().unwrap().file_names().is_empty()
            && self
                .resolved
                .as_ref()
                .unwrap()
                .project_references()
                .is_empty()
                == false
        {
            return UpToDateStatus::new(UP_TO_DATE_STATUS_TYPE_SOLUTION);
        }

        for upstream in &self.up_stream {
            let upstream_status = lock_task(&upstream.task).status.clone().unwrap();
            if orchestrator
                .opts
                .command
                .build_options
                .stop_build_on_errors
                .is_true()
                && upstream_status.is_error()
            {
                // Upstream project has errors, so we cannot build this project
                return UpToDateStatus {
                    kind: UP_TO_DATE_STATUS_TYPE_UPSTREAM_ERRORS,
                    data: StatusData::UpstreamErrors(UpstreamErrors {
                        r#ref: self.resolved.as_ref().unwrap().project_references()
                            [upstream.ref_index]
                            .path
                            .clone(),
                        ref_has_upstream_errors: upstream_status.kind
                            == UP_TO_DATE_STATUS_TYPE_UPSTREAM_ERRORS,
                    }),
                };
            }
        }

        if orchestrator.opts.command.build_options.force.is_true() {
            return UpToDateStatus::new(UP_TO_DATE_STATUS_TYPE_FORCE_BUILD);
        }

        // Check the build info
        let build_info_path = self.resolved.as_ref().unwrap().get_build_info_file_name();
        let _ = config_path;
        let (build_info, build_info_time) =
            self.load_or_store_build_info(orchestrator.host.as_ref(), &build_info_path);
        if build_info.is_none() {
            return UpToDateStatus::with_string(
                UP_TO_DATE_STATUS_TYPE_OUTPUT_MISSING,
                build_info_path,
            );
        }

        let build_info = build_info.unwrap();
        // build info version
        if !build_info.is_valid_version() {
            return UpToDateStatus::with_string(
                UP_TO_DATE_STATUS_TYPE_TS_VERSION_OUTPUT_OF_DATE,
                build_info.version.clone(),
            );
        }

        if build_info.errors
            || (!self
                .resolved
                .as_ref()
                .unwrap()
                .compiler_options()
                .no_check
                .is_true()
                && (build_info.semantic_errors || build_info.check_pending))
        {
            return UpToDateStatus::with_string(
                UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_BUILD_INFO_WITH_ERRORS,
                build_info_path,
            );
        }

        if self
            .resolved
            .as_ref()
            .unwrap()
            .compiler_options()
            .is_incremental()
        {
            if !build_info.is_incremental() {
                return UpToDateStatus::with_string(
                    UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_OPTIONS,
                    build_info_path.clone(),
                );
            }

            if (self
                .resolved
                .as_ref()
                .unwrap()
                .compiler_options()
                .get_emit_declarations()
                && !build_info.emit_diagnostics_per_file.is_empty())
                || (!self
                    .resolved
                    .as_ref()
                    .unwrap()
                    .compiler_options()
                    .no_check
                    .is_true()
                    && (!build_info.change_file_set.is_empty()
                        || !build_info.semantic_diagnostics_per_file.is_empty()))
            {
                return UpToDateStatus::with_string(
                    UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_BUILD_INFO_WITH_ERRORS,
                    build_info_path.clone(),
                );
            }

            if !self
                .resolved
                .as_ref()
                .unwrap()
                .compiler_options()
                .no_emit
                .is_true()
                && (!build_info.change_file_set.is_empty()
                    || !build_info.affected_files_pending_emit.is_empty())
            {
                return UpToDateStatus::with_string(
                    UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_BUILD_INFO_WITH_PENDING_EMIT,
                    build_info_path.clone(),
                );
            }

            if build_info.is_emit_pending(
                self.resolved.as_ref().unwrap(),
                &tspath::get_directory_path(&tspath::get_normalized_absolute_path(
                    &build_info_path,
                    &orchestrator.compare_paths_options.current_directory,
                )),
            ) {
                return UpToDateStatus::with_string(
                    UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_OPTIONS,
                    build_info_path.clone(),
                );
            }
        }

        let mut input_text_unchanged = false;
        let mut oldest_output_file_and_time = FileAndTime {
            file: build_info_path.clone(),
            time: build_info_time,
        };
        let mut newest_input_file_and_time = FileAndTime::default();
        let mut seen_roots = collections::Set::<tspath::Path>::default();
        let build_info_directory =
            tspath::get_directory_path(&tspath::get_normalized_absolute_path(
                &build_info_path,
                &orchestrator.compare_paths_options.current_directory,
            ));
        let mut build_info_root_info_reader = None;

        for input_file in self.resolved.as_ref().unwrap().file_names() {
            let input_time = orchestrator.host.get_m_time(input_file);
            if input_time == SystemTime::UNIX_EPOCH {
                return UpToDateStatus::with_string(
                    UP_TO_DATE_STATUS_TYPE_INPUT_FILE_MISSING,
                    input_file.clone(),
                );
            }
            let input_path = orchestrator.to_path(input_file);
            if input_time > oldest_output_file_and_time.time {
                let mut version = String::new();
                let mut current_version = String::new();
                if build_info.is_incremental() {
                    if build_info_root_info_reader.is_none() {
                        build_info_root_info_reader =
                            Some(build_info.get_build_info_root_info_reader(
                                &build_info_directory,
                                orchestrator.compare_paths_options.clone(),
                            ));
                    }
                    let (build_info_file_info, resolved_input_path) = build_info_root_info_reader
                        .as_ref()
                        .unwrap()
                        .get_build_info_file_info(input_path.clone());
                    if let Some(file_info) =
                        build_info_file_info.and_then(|info| info.get_file_info())
                    {
                        if !file_info.version.is_empty() {
                            version = file_info.version.clone();
                            let (text, ok) = orchestrator.host.fs().read_file(&resolved_input_path);
                            if ok {
                                current_version = incremental::compute_hash(
                                    &text,
                                    orchestrator.opts.testing.is_some(),
                                );
                                if version == current_version {
                                    input_text_unchanged = true;
                                }
                            }
                        }
                    }
                }

                if version.is_empty() || version != current_version {
                    return UpToDateStatus {
                        kind: UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER,
                        data: StatusData::InputOutputName(InputOutputName {
                            input: input_file.clone(),
                            output: build_info_path.clone(),
                        }),
                    };
                }
            }
            if input_time > newest_input_file_and_time.time {
                newest_input_file_and_time = FileAndTime {
                    file: input_file.clone(),
                    time: input_time,
                };
            }
            seen_roots.add(input_path);
        }

        if build_info_root_info_reader.is_none() {
            build_info_root_info_reader = Some(build_info.get_build_info_root_info_reader(
                &build_info_directory,
                orchestrator.compare_paths_options.clone(),
            ));
        }
        for root in build_info_root_info_reader.as_ref().unwrap().roots() {
            if !seen_roots.has(&root) {
                return UpToDateStatus {
                    kind: UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_ROOTS,
                    data: StatusData::InputOutputName(InputOutputName {
                        input: root.clone(),
                        output: build_info_path.clone(),
                    }),
                };
            }
        }

        if !self
            .resolved
            .as_ref()
            .unwrap()
            .compiler_options()
            .is_incremental()
        {
            for output_file in self.resolved.as_ref().unwrap().get_output_file_names() {
                let output_time = orchestrator.host.get_m_time(&output_file);
                if output_time == SystemTime::UNIX_EPOCH {
                    return UpToDateStatus::with_string(
                        UP_TO_DATE_STATUS_TYPE_OUTPUT_MISSING,
                        output_file.clone(),
                    );
                }

                if output_time < newest_input_file_and_time.time {
                    return UpToDateStatus {
                        kind: UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER,
                        data: StatusData::InputOutputName(InputOutputName {
                            input: newest_input_file_and_time.file.clone(),
                            output: output_file.clone(),
                        }),
                    };
                }

                if output_time < oldest_output_file_and_time.time {
                    oldest_output_file_and_time = FileAndTime {
                        file: output_file.clone(),
                        time: output_time,
                    };
                }
            }
        }

        let mut ref_dts_unchanged = false;
        for upstream_index in 0..self.up_stream.len() {
            let ref_index = self.up_stream[upstream_index].ref_index;
            let upstream_status = lock_task(&self.up_stream[upstream_index].task)
                .status
                .clone()
                .expect("upstream status should be computed");
            if upstream_status.kind == UP_TO_DATE_STATUS_TYPE_SOLUTION {
                continue;
            }

            let ref_input_output_file_and_time = upstream_status.input_output_file_and_time();
            if let Some(ref_input_output_file_and_time) = ref_input_output_file_and_time {
                if ref_input_output_file_and_time.input.time != SystemTime::UNIX_EPOCH
                    && ref_input_output_file_and_time.input.time < oldest_output_file_and_time.time
                {
                    continue;
                }
            }

            if self.has_conflicting_build_info(
                orchestrator,
                &lock_task(&self.up_stream[upstream_index].task),
            ) {
                return UpToDateStatus {
                    kind: UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER,
                    data: StatusData::InputOutputName(InputOutputName {
                        input: self.resolved.as_ref().unwrap().project_references()[ref_index]
                            .path
                            .clone(),
                        output: oldest_output_file_and_time.file.clone(),
                    }),
                };
            }

            // PORT NOTE: reshaped for borrowck; upstream task is shared graph identity.
            let newest_dts_change_time = lock_task(&self.up_stream[upstream_index].task)
                .get_latest_changed_dts_m_time(orchestrator);
            if newest_dts_change_time != SystemTime::UNIX_EPOCH
                && newest_dts_change_time < oldest_output_file_and_time.time
            {
                ref_dts_unchanged = true;
                continue;
            }

            return UpToDateStatus {
                kind: UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER,
                data: StatusData::InputOutputName(InputOutputName {
                    input: self.resolved.as_ref().unwrap().project_references()[ref_index]
                        .path
                        .clone(),
                    output: oldest_output_file_and_time.file.clone(),
                }),
            };
        }

        let check_input_file_time = |input_file: &str| -> Option<UpToDateStatus> {
            let input_time = orchestrator.host.get_m_time(input_file);
            if input_time > oldest_output_file_and_time.time {
                return Some(UpToDateStatus {
                    kind: UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER,
                    data: StatusData::InputOutputName(InputOutputName {
                        input: input_file.to_string(),
                        output: oldest_output_file_and_time.file.clone(),
                    }),
                });
            }
            None
        };

        if let Some(config_status) = check_input_file_time(&self.config) {
            return config_status;
        }

        for extended_config in self.resolved.as_ref().unwrap().extended_source_files() {
            if let Some(extended_config_status) = check_input_file_time(extended_config) {
                return extended_config_status;
            }
        }

        UpToDateStatus {
            kind: core::if_else(
                ref_dts_unchanged,
                UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_UPSTREAM_TYPES,
                core::if_else(
                    input_text_unchanged,
                    UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_INPUT_FILE_TEXT,
                    UP_TO_DATE_STATUS_TYPE_UP_TO_DATE,
                ),
            ),
            data: StatusData::InputOutputFileAndTime(InputOutputFileAndTime {
                input: newest_input_file_and_time,
                output: oldest_output_file_and_time,
                build_info: build_info_path,
            }),
        }
    }

    fn report_up_to_date_status(&mut self, orchestrator: &Orchestrator) {
        if !orchestrator.opts.command.build_options.verbose.is_true() {
            return;
        }
        let status = self.status.as_ref().unwrap().clone();
        match status.kind {
            UP_TO_DATE_STATUS_TYPE_CONFIG_FILE_NOT_FOUND => {
                self.result
                    .as_mut()
                    .unwrap()
                    .report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_config_file_does_not_exist,
                        vec![orchestrator.relative_file_name(&self.config).into()],
                    ));
            }
            UP_TO_DATE_STATUS_TYPE_UPSTREAM_ERRORS => {
                let upstream_status = status.upstream_errors();
                self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                    core::if_else(
                        upstream_status.ref_has_upstream_errors,
                        &diagnostics::Project_0_can_t_be_built_because_its_dependency_1_was_not_built,
                        &diagnostics::Project_0_can_t_be_built_because_its_dependency_1_has_errors,
                    ),
                    vec![
                        orchestrator.relative_file_name(&self.config).into(),
                        orchestrator.relative_file_name(&upstream_status.r#ref).into(),
                    ],
                ));
            }
            UP_TO_DATE_STATUS_TYPE_BUILD_ERRORS => {
                self.result
                    .as_mut()
                    .unwrap()
                    .report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_it_has_errors,
                        vec![orchestrator.relative_file_name(&self.config).into()],
                    ));
            }
            UP_TO_DATE_STATUS_TYPE_UP_TO_DATE => {
                // This is to ensure skipping verbose log for projects that were built,
                // and then some other package changed but this package doesnt need update
                if let Some(input_output_file_and_time) = status.input_output_file_and_time() {
                    self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_up_to_date_because_newest_input_1_is_older_than_output_2,
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(&input_output_file_and_time.input.file).into(),
                            orchestrator.relative_file_name(&input_output_file_and_time.output.file).into(),
                        ],
                    ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_UPSTREAM_TYPES => {
                self.result
                    .as_mut()
                    .unwrap()
                    .report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_up_to_date_with_d_ts_files_from_its_dependencies,
                        vec![orchestrator.relative_file_name(&self.config).into()],
                    ));
            }
            UP_TO_DATE_STATUS_TYPE_UP_TO_DATE_WITH_INPUT_FILE_TEXT => {
                self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                    &diagnostics::Project_0_is_up_to_date_but_needs_to_update_timestamps_of_output_files_that_are_older_than_input_files,
                    vec![orchestrator.relative_file_name(&self.config).into()],
                ));
            }
            UP_TO_DATE_STATUS_TYPE_INPUT_FILE_MISSING => {
                if let StatusData::String(file_name) = &status.data {
                    self.result
                        .as_mut()
                        .unwrap()
                        .report_status(ast::new_compiler_diagnostic(
                            &diagnostics::Project_0_is_out_of_date_because_input_1_does_not_exist,
                            vec![
                                orchestrator.relative_file_name(&self.config).into(),
                                orchestrator.relative_file_name(file_name).into(),
                            ],
                        ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_OUTPUT_MISSING => {
                if let StatusData::String(file_name) = &status.data {
                    self.result
                        .as_mut()
                        .unwrap()
                        .report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_output_file_1_does_not_exist,
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(file_name).into(),
                        ],
                    ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_INPUT_FILE_NEWER => {
                if let Some(input_output) = status.input_output_name() {
                    self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_output_1_is_older_than_input_2,
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(&input_output.output).into(),
                            orchestrator.relative_file_name(&input_output.input).into(),
                        ],
                    ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_BUILD_INFO_WITH_PENDING_EMIT => {
                if let StatusData::String(file_name) = &status.data {
                    self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_buildinfo_file_1_indicates_that_some_of_the_changes_were_not_emitted,
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(file_name).into(),
                        ],
                    ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_BUILD_INFO_WITH_ERRORS => {
                if let StatusData::String(file_name) = &status.data {
                    self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_buildinfo_file_1_indicates_that_program_needs_to_report_errors,
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(file_name).into(),
                        ],
                    ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_OPTIONS => {
                if let StatusData::String(file_name) = &status.data {
                    self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_buildinfo_file_1_indicates_there_is_change_in_compilerOptions,
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(file_name).into(),
                        ],
                    ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_OUT_OF_DATE_ROOTS => {
                if let Some(input_output) = status.input_output_name() {
                    self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_buildinfo_file_1_indicates_that_file_2_was_root_file_of_compilation_but_not_any_more,
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(&input_output.output).into(),
                            orchestrator.relative_file_name(&input_output.input).into(),
                        ],
                    ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_TS_VERSION_OUTPUT_OF_DATE => {
                if let StatusData::String(version) = &status.data {
                    self.result.as_mut().unwrap().report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_out_of_date_because_output_for_it_was_generated_with_version_1_that_differs_with_current_version_2,
                        vec![
                            orchestrator.relative_file_name(&self.config).into(),
                            orchestrator.relative_file_name(version).into(),
                            core::version().into(),
                        ],
                    ));
                }
            }
            UP_TO_DATE_STATUS_TYPE_FORCE_BUILD => {
                self.result
                    .as_mut()
                    .unwrap()
                    .report_status(ast::new_compiler_diagnostic(
                        &diagnostics::Project_0_is_being_forcibly_rebuilt,
                        vec![orchestrator.relative_file_name(&self.config).into()],
                    ));
            }
            UP_TO_DATE_STATUS_TYPE_SOLUTION => {
                // Does not need to report status
            }
            _ => panic!("Unknown up to date status kind: {}", status.kind),
        }
    }

    fn can_update_js_dts_output_timestamps(&self) -> bool {
        !self
            .resolved
            .as_ref()
            .unwrap()
            .compiler_options()
            .no_emit
            .is_true()
            && !self
                .resolved
                .as_ref()
                .unwrap()
                .compiler_options()
                .is_incremental()
    }

    fn update_time_stamps(
        &mut self,
        orchestrator: &mut Orchestrator,
        emitted_files: Vec<String>,
        verbose_message: &'static diagnostics::Message,
    ) {
        let emitted = collections::Set::new_from_items(emitted_files);
        let mut verbose_message_reported = false;
        let build_info_name = self.resolved.as_ref().unwrap().get_build_info_file_name();
        let now = orchestrator.opts.sys.now();
        let mut update_time_stamp = |this: &mut BuildTask, file: String| {
            if emitted.has(&file) {
                return;
            }
            if !verbose_message_reported
                && orchestrator.opts.command.build_options.verbose.is_true()
            {
                this.result
                    .as_mut()
                    .unwrap()
                    .report_status(ast::new_compiler_diagnostic(
                        verbose_message,
                        vec![orchestrator.relative_file_name(&this.config).into()],
                    ));
                verbose_message_reported = true;
            }
            let err = orchestrator.host.set_m_time(&file, now);
            if err.is_ok() {
                if file == build_info_name {
                    let _lock = this.build_info_entry_mu.lock().unwrap();
                    if let Some(entry) = &mut this.build_info_entry {
                        entry.m_time = now;
                    }
                } else if this.store_output_time_stamp(orchestrator) {
                    orchestrator.host.store_m_time(&file, now);
                }
            }
        };

        if self.can_update_js_dts_output_timestamps() {
            for output_file in self.resolved.as_ref().unwrap().get_output_file_names() {
                update_time_stamp(self, output_file);
            }
        }
        update_time_stamp(
            self,
            self.resolved.as_ref().unwrap().get_build_info_file_name(),
        );
    }

    pub fn clean_project(&mut self, orchestrator: &mut Orchestrator, _path: tspath::Path) {
        if self.resolved.is_none() {
            self.report_diagnostic(ast::new_compiler_diagnostic(
                &diagnostics::File_0_not_found,
                vec![self.config.clone().into()],
            ));
            self.result.as_mut().unwrap().exit_status =
                tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED;
            return;
        }

        let inputs = collections::Set::new_from_items(
            self.resolved
                .as_ref()
                .unwrap()
                .file_names()
                .into_iter()
                .map(|name| orchestrator.to_path(&name)),
        );
        for output_file in self.resolved.as_ref().unwrap().get_output_file_names() {
            self.clean_project_output(orchestrator, &output_file, &inputs);
        }
        self.clean_project_output(
            orchestrator,
            &self.resolved.as_ref().unwrap().get_build_info_file_name(),
            &inputs,
        );
    }

    fn clean_project_output(
        &mut self,
        orchestrator: &mut Orchestrator,
        output_file: &str,
        inputs: &collections::Set<tspath::Path>,
    ) {
        let output_path = orchestrator.to_path(output_file);
        // If output name is same as input file name, do not delete and ignore the error
        if inputs.has(&output_path) {
            return;
        }
        if orchestrator.host.fs().file_exists(output_file) {
            if !orchestrator.opts.command.build_options.dry.is_true() {
                let err = orchestrator.host.fs().remove(output_file);
                if err.is_err() {
                    self.report_diagnostic(ast::new_compiler_diagnostic(
                        &diagnostics::Failed_to_delete_file_0,
                        vec![output_file.to_string().into()],
                    ));
                }
            } else {
                self.result
                    .as_mut()
                    .unwrap()
                    .files_to_delete
                    .push(output_file.to_string());
            }
        }
    }

    pub fn update_watch(
        &mut self,
        orchestrator: &mut Orchestrator,
        old_cache: &collections::SyncMap<tspath::Path, SystemTime>,
    ) {
        self.config_time =
            orchestrator
                .host
                .load_or_store_m_time(&self.config, Some(old_cache), false);
        if let Some(resolved) = &self.resolved {
            self.extended_config_times = resolved
                .extended_source_files()
                .into_iter()
                .map(|p| {
                    orchestrator
                        .host
                        .load_or_store_m_time(p, Some(old_cache), false)
                })
                .collect();
            self.input_files = resolved
                .file_names()
                .into_iter()
                .map(|p| {
                    orchestrator
                        .host
                        .load_or_store_m_time(p, Some(old_cache), false)
                })
                .collect();
            if self.can_update_js_dts_output_timestamps() {
                for output_file in resolved.get_output_file_names() {
                    orchestrator
                        .host
                        .store_m_time_from_old_cache(&output_file, old_cache);
                }
            }
        }
    }

    fn reset_status(&mut self) {
        self.status = None;
        self.pending.store(true, Ordering::SeqCst);
        self.errors.clear();
    }

    fn reset_config(&mut self, orchestrator: &mut Orchestrator, path: tspath::Path) {
        self.dirty = true;
        orchestrator.host.resolved_references.delete(&path);
    }

    pub fn has_update(
        &mut self,
        orchestrator: &mut Orchestrator,
        path: tspath::Path,
    ) -> UpdateKind {
        let mut needs_config_update = false;
        let mut needs_update = false;
        if orchestrator.host.get_m_time(&self.config) != self.config_time {
            self.reset_config(orchestrator, path.clone());
            needs_config_update = true;
        }
        if self.resolved.is_some() {
            let resolved = self.resolved.clone().unwrap();
            for (index, file) in resolved.extended_source_files().iter().enumerate() {
                let current_m_time = orchestrator.host.get_m_time(file);
                if current_m_time != self.extended_config_times[index] {
                    self.reset_config(orchestrator, path.clone());
                    needs_config_update = true;
                }
            }
            for (index, file) in resolved.file_names().iter().enumerate() {
                if orchestrator.host.get_m_time(file) != self.input_files[index] {
                    self.reset_status();
                    needs_update = true;
                }
            }
            if !needs_config_update {
                let config_start = orchestrator.opts.sys.now();
                let new_config =
                    resolved.reload_file_names_of_parsed_command_line(orchestrator.host.fs());
                let config_time = orchestrator
                    .opts
                    .sys
                    .now()
                    .duration_since(config_start)
                    .unwrap_or_default();
                // Make new channels if needed later
                self.report_done = Arc::new((Mutex::new(false), Condvar::new()));
                self.done = Arc::new((Mutex::new(false), Condvar::new()));
                if resolved.file_names() != new_config.file_names() {
                    orchestrator
                        .host
                        .resolved_references
                        .store(path.clone(), Some(new_config.clone()));
                    orchestrator.host.store_config_time(path, config_time);
                    self.resolved = Some(new_config);
                    self.reset_status();
                    needs_update = true;
                }
            }
        }
        core::if_else(
            needs_config_update,
            UPDATE_KIND_CONFIG,
            core::if_else(needs_update, UPDATE_KIND_UPDATE, UPDATE_KIND_NONE),
        )
    }

    pub(crate) fn load_or_store_build_info<H>(
        &mut self,
        host: &BuildHost<H>,
        build_info_file_name: &str,
    ) -> (Option<incremental::BuildInfo>, SystemTime)
    where
        H: compiler::CompilerHost,
    {
        let path = host.to_path(build_info_file_name);
        let _lock = self.build_info_entry_mu.lock().unwrap();
        if let Some(entry) = &self.build_info_entry {
            if entry.path == path {
                return (entry.build_info.clone(), entry.m_time);
            }
        }
        let build_info = if build_info_file_name.is_empty() {
            None
        } else {
            let (data, ok) = host.fs().read_file(build_info_file_name);
            if ok {
                serde_json::from_str(&data).ok()
            } else {
                None
            }
        };
        let mut m_time = SystemTime::UNIX_EPOCH;
        if build_info.is_some() {
            m_time = host.get_m_time(build_info_file_name);
        }
        self.build_info_entry = Some(BuildInfoEntry {
            build_info: build_info.clone(),
            path,
            m_time,
            dts_time: None,
        });
        (build_info, m_time)
    }

    fn on_build_info_emit(
        &mut self,
        orchestrator: &Orchestrator,
        build_info_file_name: &str,
        build_info: incremental::BuildInfo,
        has_changed_dts_file: bool,
        m_time: SystemTime,
    ) {
        let _lock = self.build_info_entry_mu.lock().unwrap();
        let dts_time = if has_changed_dts_file {
            Some(m_time)
        } else {
            self.build_info_entry
                .as_ref()
                .and_then(|entry| entry.dts_time)
        };
        self.build_info_entry = Some(BuildInfoEntry {
            build_info: Some(build_info),
            path: orchestrator.to_path(build_info_file_name),
            m_time,
            dts_time,
        });
    }

    fn has_conflicting_build_info(
        &self,
        _orchestrator: &Orchestrator,
        upstream: &BuildTask,
    ) -> bool {
        if let (Some(this), Some(upstream)) = (&self.build_info_entry, &upstream.build_info_entry) {
            return this.path == upstream.path;
        }
        false
    }

    fn get_latest_changed_dts_m_time(&mut self, orchestrator: &Orchestrator) -> SystemTime {
        let _lock = self.build_info_entry_mu.lock().unwrap();
        if let Some(dts_time) = self
            .build_info_entry
            .as_ref()
            .and_then(|entry| entry.dts_time)
        {
            return dts_time;
        }
        let dts_time = orchestrator
            .host
            .get_m_time(&tspath::get_normalized_absolute_path(
                &self
                    .build_info_entry
                    .as_ref()
                    .unwrap()
                    .build_info
                    .as_ref()
                    .unwrap()
                    .latest_changed_dts_file,
                &tspath::get_directory_path(
                    &self.build_info_entry.as_ref().unwrap().path.to_string(),
                ),
            ));
        self.build_info_entry.as_mut().unwrap().dts_time = Some(dts_time);
        dts_time
    }

    fn store_output_time_stamp(&self, orchestrator: &Orchestrator) -> bool {
        orchestrator.opts.command.compiler_options.watch.is_true()
            && !self
                .resolved
                .as_ref()
                .unwrap()
                .compiler_options()
                .is_incremental()
    }
}

impl TaskResult {
    fn report_status(&self, diagnostic: ast::Diagnostic) {
        (self.report_status)(diagnostic);
    }

    fn diagnostic_reporter(&self, diagnostic: ast::Diagnostic) {
        (self.diagnostic_reporter)(diagnostic);
    }
}
