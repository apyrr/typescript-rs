use std::io::Write;
use std::sync::{Arc, Mutex};

use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_tracing as tracing;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use super::{
    CommandLineTesting, CompileAndEmitResult, CompileTimes, DiagnosticReporter,
    DiagnosticsReporter, ExitStatus, MTimesCache, MemoryStats, Statistics, System,
    statistics_from_program,
};

pub fn get_trace_with_writer_from_sys(
    w: Box<dyn Write + Send>,
    locale: locale::Locale,
    testing: Option<CommandLineTesting>,
    use_package_json_cache: bool,
) -> Box<dyn Fn(&diagnostics::Message, Vec<serde_json::Value>) + Send + Sync> {
    if testing.is_none() {
        let writer = Mutex::new(w);
        Box::new(move |msg, args| {
            let args = args
                .into_iter()
                .map(|arg| Box::new(arg) as diagnostics::Any)
                .collect();
            let mut writer = writer.lock().unwrap_or_else(|err| err.into_inner());
            let _ = writeln!(writer, "{}", msg.localize(locale.clone(), args));
        })
    } else {
        testing
            .expect("checked above")
            .get_trace(w, locale, use_package_json_cache)
    }
}

pub struct EmitInput<'a> {
    pub sys: System,
    pub program_like: &'a mut dyn compiler::ProgramLike,
    pub config: tsoptions::ParsedCommandLine,
    pub report_diagnostic: DiagnosticReporter,
    pub report_error_summary: DiagnosticsReporter,
    pub writer: Box<dyn Write>,
    pub write_file: Option<compiler::WriteFile>,
    pub compile_times: CompileTimes,
    pub testing: Option<CommandLineTesting>,
    pub testing_m_times_cache: Option<Arc<MTimesCache>>,
    pub tracing: Option<tracing::Tracing>,
}

pub fn emit_and_report_statistics(
    mut input: EmitInput<'_>,
) -> (CompileAndEmitResult, Option<Statistics>) {
    let mut statistics = None;
    let mut result = emit_files_and_report_errors_worker(&mut input);
    if result.status != ExitStatus::Success {
        // compile exited early
        return (result, None);
    }
    input.compile_times = result.times.clone();
    input.compile_times.total_time = input.sys.since_start();
    result.times.total_time = input.compile_times.total_time;

    if input.config.compiler_options().diagnostics.is_true()
        || input
            .config
            .compiler_options()
            .extended_diagnostics
            .is_true()
    {
        // GC must be called twice to allow things to settle.
        let mem_stats = MemoryStats::read_after_settling();

        statistics = Some(statistics_from_program(&input, &mem_stats));
        let testing = input.testing.take();
        statistics.as_ref().unwrap().report(input.writer, testing);
    }

    if result
        .emit_result
        .as_ref()
        .is_some_and(|emit_result| emit_result.emit_skipped)
        && !result.diagnostics.is_empty()
    {
        result.status = ExitStatus::DiagnosticsPresentOutputsSkipped;
    } else if !result.diagnostics.is_empty() {
        result.status = ExitStatus::DiagnosticsPresentOutputsGenerated;
    }
    (result, statistics)
}

pub fn emit_files_and_report_errors(mut input: EmitInput<'_>) -> CompileAndEmitResult {
    emit_files_and_report_errors_worker(&mut input)
}

fn emit_files_and_report_errors_worker(input: &mut EmitInput<'_>) -> CompileAndEmitResult {
    let mut result = CompileAndEmitResult {
        times: input.compile_times.clone(),
        ..Default::default()
    };
    let ctx = core::Context::background();

    let program_like = &mut *input.program_like;
    let mut all_diagnostics = compiler::get_diagnostics_of_any_program(
        ctx.clone(),
        program_like,
        None,
        false,
        |program_like, ctx, file| {
            // Options diagnostics include global diagnostics (even though we collect them separately),
            // and global diagnostics create checkers, which then bind all of the files. Do this binding
            // early so we can track the time.
            program_like.get_bind_diagnostics(ctx, file)
        },
        |program_like, ctx, file| program_like.get_semantic_diagnostics(ctx, file),
    );

    let mut emit_result = compiler::EmitResult {
        emit_skipped: true,
        diagnostics: Vec::new(),
        ..Default::default()
    };
    if !input.program_like.options().list_files_only.is_true() {
        let emit_start = input.sys.now();
        emit_result = input
            .program_like
            .emit(
                ctx,
                compiler::EmitOptions {
                    write_file: input.write_file.clone(),
                    ..Default::default()
                },
            )
            .unwrap_or_default();
        result.times.emit_time = input
            .sys
            .now()
            .duration_since(emit_start)
            .unwrap_or_default();
    }
    all_diagnostics.extend(emit_result.diagnostics.clone());
    if let Some(testing) = &input.testing {
        testing.on_emitted_files(&emit_result, input.testing_m_times_cache.as_deref());
    }

    all_diagnostics = compiler::sort_and_deduplicate_diagnostics(all_diagnostics);
    for diagnostic in &all_diagnostics {
        (input.report_diagnostic)(diagnostic.clone());
    }

    list_files(input, &emit_result);

    (input.report_error_summary)(all_diagnostics.clone());
    result.diagnostics = all_diagnostics;
    result.emit_result = Some(emit_result);
    result.status = ExitStatus::Success;
    result
}

pub fn list_files(input: &mut EmitInput<'_>, emit_result: &compiler::EmitResult) {
    if let Some(testing) = &input.testing {
        testing.on_list_files_start(&mut input.writer);
    }
    let options = input.program_like.options();
    if options.list_emitted_files.is_true() {
        for file in &emit_result.emitted_files {
            let _ = writeln!(
                input.writer,
                "TSFILE:  {}",
                tspath::get_normalized_absolute_path(file, input.config.get_current_directory())
            );
        }
    }
    if options.explain_files.is_true() {
        input
            .program_like
            .program()
            .explain_files(&mut input.writer, input.config.locale());
    } else if options.list_files.is_true() || options.list_files_only.is_true() {
        for file in input.program_like.get_source_files() {
            let _ = writeln!(input.writer, "{}", file.file_name());
        }
    }
    if let Some(testing) = &input.testing {
        testing.on_list_files_end(&mut input.writer);
    }
}
