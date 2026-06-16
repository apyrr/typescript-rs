use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;

use serde_json::Value;
use ts_ast as ast;
use ts_collections::OrderedMap;
use ts_compiler as compiler;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_format as format;
use ts_json as json;
use ts_locale as locale;
use ts_ls as lsutil;
use ts_parser as parser;
#[cfg(feature = "pprof")]
use ts_pprof as pprof;
use ts_tracing as tracing;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::build;
use crate::incremental;
use crate::tsc;
use crate::watcher;

pub(crate) fn command_line_error_diagnostic(error: String) -> ast::Diagnostic {
    if let Some(argument) = error.strip_prefix("Unknown_compiler_option_0: ") {
        return ast::new_compiler_diagnostic(
            &diagnostics::Unknown_compiler_option_0,
            &[Box::new(argument.to_owned()) as diagnostics::Argument],
        );
    }
    if let Some(argument) = error.strip_prefix("Cannot_read_file_0: ") {
        return ast::new_compiler_diagnostic(
            &diagnostics::Cannot_read_file_0,
            &[Box::new(argument.to_owned()) as diagnostics::Argument],
        );
    }
    if let Some(argument) = error.strip_prefix("Compiler_option_0_may_only_be_used_with_build: ") {
        return ast::new_compiler_diagnostic(
            &diagnostics::Compiler_option_0_may_only_be_used_with_build,
            &[Box::new(argument.to_owned()) as diagnostics::Argument],
        );
    }
    if let Some(argument) = error.strip_prefix("Compiler_option_0_may_not_be_used_with_build: ") {
        return ast::new_compiler_diagnostic(
            &diagnostics::Compiler_option_0_may_not_be_used_with_build,
            &[Box::new(argument.to_owned()) as diagnostics::Argument],
        );
    }
    if error.starts_with("Option_build_must_be_the_first_command_line_argument: ") {
        return ast::new_compiler_diagnostic(
            &diagnostics::Option_build_must_be_the_first_command_line_argument,
            &[],
        );
    }
    if let Some(arguments) = error.strip_prefix("Options_0_and_1_cannot_be_combined: ") {
        let mut arguments = arguments.splitn(2, "\u{1f}");
        let first = arguments
            .next()
            .expect("encoded first option diagnostic name")
            .to_owned();
        let second = arguments
            .next()
            .expect("encoded second option diagnostic name")
            .to_owned();
        return ast::new_compiler_diagnostic(
            &diagnostics::Options_0_and_1_cannot_be_combined,
            &[
                Box::new(first) as diagnostics::Argument,
                Box::new(second) as diagnostics::Argument,
            ],
        );
    }
    if let Some(arguments) = error.strip_prefix("Argument_for_0_option_must_be_Colon_1: ") {
        let mut arguments = arguments.splitn(2, "\u{1f}");
        let option = arguments
            .next()
            .expect("encoded option diagnostic name")
            .to_owned();
        let values = arguments
            .next()
            .expect("encoded option diagnostic values")
            .to_owned();
        return ast::new_compiler_diagnostic(
            &diagnostics::Argument_for_0_option_must_be_Colon_1,
            &[
                Box::new(option) as diagnostics::Argument,
                Box::new(values) as diagnostics::Argument,
            ],
        );
    }
    if error == "Locale_must_be_an_IETF_BCP_47_language_tag_Examples_Colon_0_1" {
        return ast::new_compiler_diagnostic(
            &diagnostics::Locale_must_be_an_IETF_BCP_47_language_tag_Examples_Colon_0_1,
            &[
                Box::new("en".to_owned()) as diagnostics::Argument,
                Box::new("ja-jp".to_owned()) as diagnostics::Argument,
            ],
        );
    }
    panic!("unhandled command line diagnostic: {error}");
}

pub(crate) fn program_options(
    config: &tsoptions::ParsedCommandLine,
    host: Arc<dyn compiler::CompilerHost>,
    tracing: Option<tracing::Tracing>,
) -> compiler::ProgramOptions {
    compiler::ProgramOptions {
        host,
        config: Box::new(config.clone()),
        use_source_of_project_reference: false,
        single_threaded: core::TS_UNKNOWN,
        create_checker_pool: None,
        typings_location: String::new(),
        project_name: String::new(),
        type_script_version: String::new(),
        tracing,
    }
}

pub(crate) fn apply_command_line_watch_options(
    config: &mut tsoptions::ParsedCommandLine,
    watch_options: &BTreeMap<String, String>,
) {
    for (name, value) in watch_options {
        config.watch_options.insert(name.clone(), value.clone());
    }
}

pub fn start_tracing_if_needed(
    mut sys: tsc::System,
    config: &tsoptions::ParsedCommandLine,
    testing: Option<tsc::CommandLineTesting>,
) -> Option<tracing::Tracing> {
    let trace_dir = config.compiler_options().generate_trace.clone();
    if trace_dir.is_empty() {
        return None;
    }
    let mut config_file_path = String::new();
    if let Some(config_file) = &config.config_file {
        config_file_path = config_file.source_file.file_name();
    }
    match tracing::start_tracing(
        sys.clone(),
        &trace_dir,
        &config_file_path,
        testing.is_some(),
    ) {
        Ok(tr) => Some(tr),
        Err(err) => {
            let _ = writeln!(sys.writer(), "Warning: Failed to start tracing: {err}");
            None
        }
    }
}

pub fn stop_tracing(mut sys: tsc::System, tr: Option<tracing::Tracing>) {
    let Some(mut tr) = tr else {
        return;
    };
    if let Err(err) = tr.stop_tracing() {
        let _ = writeln!(sys.writer(), "Warning: Failed to stop tracing: {err}");
    }
}

#[cfg(not(feature = "pprof"))]
struct DisabledProfileSession<W: Write> {
    writer: W,
}

#[cfg(not(feature = "pprof"))]
fn begin_profiling_disabled<W: Write>(
    profile_dir: &str,
    mut writer: W,
) -> DisabledProfileSession<W> {
    let _ = writeln!(
        writer,
        "Warning: pprof profiling is disabled in this Rust build; ignoring {profile_dir}"
    );
    DisabledProfileSession { writer }
}

#[cfg(not(feature = "pprof"))]
impl<W: Write> DisabledProfileSession<W> {
    fn stop(&mut self) {
        let _ = self.writer.flush();
    }
}

#[cfg(feature = "pprof")]
type ProfileSession<W> = pprof::ProfileSession<W>;
#[cfg(not(feature = "pprof"))]
type ProfileSession<W> = DisabledProfileSession<W>;

#[cfg(feature = "pprof")]
fn begin_profile_session<W: Write>(profile_dir: &str, writer: W) -> ProfileSession<W> {
    pprof::begin_profiling(profile_dir, writer)
}

#[cfg(not(feature = "pprof"))]
fn begin_profile_session<W: Write>(profile_dir: &str, writer: W) -> ProfileSession<W> {
    begin_profiling_disabled(profile_dir, writer)
}

fn stop_profile_session<W: Write>(profile_session: &mut Option<ProfileSession<W>>) {
    if let Some(profile_session) = profile_session.as_mut() {
        profile_session.stop();
    }
}

fn command_line_raw_for_init(
    command_line: &tsoptions::ParsedCommandLine,
) -> OrderedMap<String, Value> {
    if let Some(raw) = command_line.raw.as_deref()
        && let Ok(raw) = serde_json::from_str::<OrderedMap<String, Value>>(raw)
    {
        return raw;
    }

    let mut result = OrderedMap::default();
    for (name, value) in &command_line.options {
        let parsed = tsoptions::options_declaration_for(name)
            .and_then(|option| option.kind)
            .map(|kind| match kind {
                tsoptions::CommandLineOptionKind::Boolean => Value::Bool(value == "true"),
                tsoptions::CommandLineOptionKind::Number => value
                    .parse::<i64>()
                    .ok()
                    .map(Into::into)
                    .unwrap_or_else(|| Value::String(value.clone())),
                tsoptions::CommandLineOptionKind::List
                | tsoptions::CommandLineOptionKind::ListOrElement => Value::Array(
                    value
                        .split(',')
                        .map(|item| Value::String(item.to_owned()))
                        .collect(),
                ),
                _ => Value::String(value.clone()),
            })
            .unwrap_or_else(|| Value::String(value.clone()));
        result.set(name.clone(), parsed);
    }
    result
}

pub fn command_line(
    sys: tsc::System,
    command_line_args: Vec<String>,
    testing: Option<tsc::CommandLineTesting>,
) -> tsc::CommandLineResult {
    if !command_line_args.is_empty() {
        match command_line_args[0].to_lowercase().as_str() {
            "-b" | "--b" | "-build" | "--build" => {
                return tsc_build_compilation(
                    sys.clone(),
                    tsoptions::parse_build_command_line(&command_line_args, sys),
                    testing,
                );
                // case "-f":
                // 	return fmtMain(sys, commandLineArgs[1], commandLineArgs[1])
            }
            _ => {}
        }
    }

    tsc_compilation(
        sys.clone(),
        tsoptions::parse_command_line(&command_line_args, sys.clone()),
        testing,
    )
}

pub fn fmt_main(mut sys: tsc::System, mut input: String, mut output: String) -> tsc::ExitStatus {
    let ctx = format::with_format_code_settings(
        format::Context::default(),
        lsutil::get_default_format_code_settings(),
        "\n".to_string(),
    );
    input = tspath::to_path(
        &input,
        &sys.get_current_directory(),
        sys.fs().use_case_sensitive_file_names(),
    );
    output = tspath::to_path(
        &output,
        &sys.get_current_directory(),
        sys.fs().use_case_sensitive_file_names(),
    );
    let (file_content, ok) = sys.fs().read_file(&input);
    if !ok {
        let _ = writeln!(sys.writer(), "File not found: {input}");
        return tsc::EXIT_STATUS_NOT_IMPLEMENTED;
    }
    let text = file_content;
    let pathified = tspath::to_path(&input, &sys.get_current_directory(), true);
    let source_file = parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: pathified.clone(),
            path: pathified.clone(),
            ..Default::default()
        },
        text.clone(),
        core::get_script_kind_from_file_name(&pathified),
    );
    let edits = format::format_document(&ctx, &source_file);
    let new_text = core::apply_bulk_edits(&text, &edits);

    if let Err(err) = sys.fs().write_file(&output, &new_text) {
        let _ = writeln!(sys.writer(), "{}", err);
        return tsc::EXIT_STATUS_NOT_IMPLEMENTED;
    }
    tsc::EXIT_STATUS_SUCCESS
}

pub fn tsc_build_compilation(
    sys: tsc::System,
    build_command: tsoptions::ParsedBuildCommandLine,
    testing: Option<tsc::CommandLineTesting>,
) -> tsc::CommandLineResult {
    let locale = build_command.locale();
    let report_diagnostic = tsc::create_diagnostic_reporter(
        sys.clone(),
        Box::new(sys.clone()),
        locale.clone(),
        build_command.compiler_options.clone(),
    );

    if !build_command.errors.is_empty() {
        for err in build_command.errors {
            report_diagnostic(command_line_error_diagnostic(err));
        }
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED,
            ..Default::default()
        };
    }

    let mut profile_session = None;
    if !build_command.compiler_options.pprof_dir.is_empty() {
        // !!! stderr?
        profile_session = Some(begin_profile_session(
            &build_command.compiler_options.pprof_dir,
            Box::new(sys.clone()),
        ));
    }

    if build_command.compiler_options.help.is_true() {
        tsc::print_version(sys.clone(), locale.clone());
        tsc::print_build_help(sys.clone(), locale.clone(), tsoptions::build_opts());
        stop_profile_session(&mut profile_session);
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_SUCCESS,
            ..Default::default()
        };
    }

    let orchestrator = build::new_orchestrator(build::Options {
        sys,
        command: build_command,
        testing,
    });
    let result = orchestrator.start();
    stop_profile_session(&mut profile_session);
    result
}

pub fn tsc_compilation(
    sys: tsc::System,
    command_line: tsoptions::ParsedCommandLine,
    testing: Option<tsc::CommandLineTesting>,
) -> tsc::CommandLineResult {
    let mut config_file_name = String::new();
    let locale = command_line.locale();
    let mut report_diagnostic = tsc::create_diagnostic_reporter(
        sys.clone(),
        Box::new(sys.clone()),
        locale.clone(),
        command_line.compiler_options(),
    );

    if !command_line.errors.is_empty() {
        for e in command_line.errors.clone() {
            report_diagnostic(command_line_error_diagnostic(e));
        }
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED,
            ..Default::default()
        };
    }

    let mut profile_session = None;
    if !command_line.compiler_options().pprof_dir.is_empty() {
        // !!! stderr?
        profile_session = Some(begin_profile_session(
            &command_line.compiler_options().pprof_dir,
            Box::new(sys.clone()),
        ));
    }

    if command_line.compiler_options().init.is_true() {
        tsc::write_config_file(
            sys.clone(),
            locale.clone(),
            report_diagnostic,
            command_line_raw_for_init(&command_line),
        );
        stop_profile_session(&mut profile_session);
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_SUCCESS,
            ..Default::default()
        };
    }

    if command_line.compiler_options().version.is_true() {
        tsc::print_version(sys.clone(), locale.clone());
        stop_profile_session(&mut profile_session);
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_SUCCESS,
            ..Default::default()
        };
    }

    if command_line.compiler_options().help.is_true()
        || command_line.compiler_options().all.is_true()
    {
        tsc::print_help(sys.clone(), locale.clone(), &command_line);
        stop_profile_session(&mut profile_session);
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_SUCCESS,
            ..Default::default()
        };
    }

    if command_line.compiler_options().watch.is_true()
        && command_line.compiler_options().list_files_only.is_true()
    {
        report_diagnostic(ast::new_compiler_diagnostic(
            &diagnostics::Options_0_and_1_cannot_be_combined,
            &[
                Box::new("watch".to_owned()) as diagnostics::Argument,
                Box::new("listFilesOnly".to_owned()) as diagnostics::Argument,
            ],
        ));
        stop_profile_session(&mut profile_session);
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED,
            ..Default::default()
        };
    }

    if !command_line.compiler_options().project.is_empty() {
        if !command_line.file_names().is_empty() {
            report_diagnostic(ast::new_compiler_diagnostic(
                &diagnostics::Option_project_cannot_be_mixed_with_source_files_on_a_command_line,
                &[],
            ));
            stop_profile_session(&mut profile_session);
            return tsc::CommandLineResult {
                status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED,
                ..Default::default()
            };
        }

        let file_or_directory = tspath::normalize_path(&command_line.compiler_options().project);
        if sys.fs().directory_exists(&file_or_directory) {
            config_file_name = tspath::combine_paths(&file_or_directory, &["tsconfig.json"]);
            if !sys.fs().file_exists(&config_file_name) {
                report_diagnostic(ast::new_compiler_diagnostic(
                    &diagnostics::Cannot_find_a_tsconfig_json_file_at_the_current_directory_Colon_0,
                    &[Box::new(config_file_name.clone()) as diagnostics::Argument],
                ));
                stop_profile_session(&mut profile_session);
                return tsc::CommandLineResult {
                    status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED,
                    ..Default::default()
                };
            }
        } else {
            config_file_name = file_or_directory.clone();
            if !sys.fs().file_exists(&config_file_name) {
                report_diagnostic(ast::new_compiler_diagnostic(
                    &diagnostics::The_specified_path_does_not_exist_Colon_0,
                    &[Box::new(file_or_directory) as diagnostics::Argument],
                ));
                stop_profile_session(&mut profile_session);
                return tsc::CommandLineResult {
                    status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED,
                    ..Default::default()
                };
            }
        }
    } else if !command_line.compiler_options().ignore_config.is_true()
        || command_line.file_names().is_empty()
    {
        let search_path = tspath::normalize_path(&sys.get_current_directory());
        config_file_name = find_config_file(
            search_path,
            |file| sys.fs().file_exists(file),
            "tsconfig.json",
        );
        if !command_line.file_names().is_empty() {
            if !config_file_name.is_empty() {
                // Error to not specify config file
                report_diagnostic(ast::new_compiler_diagnostic(
                    &diagnostics::X_tsconfig_json_is_present_but_will_not_be_loaded_if_files_are_specified_on_commandline_Use_ignoreConfig_to_skip_this_error,
                    &[],
                ));
                stop_profile_session(&mut profile_session);
                return tsc::CommandLineResult {
                    status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED,
                    ..Default::default()
                };
            }
        } else if config_file_name.is_empty() {
            if command_line.compiler_options().show_config.is_true() {
                report_diagnostic(ast::new_compiler_diagnostic(
                    &diagnostics::Cannot_find_a_tsconfig_json_file_at_the_current_directory_Colon_0,
                    &[
                        Box::new(tspath::normalize_path(&sys.get_current_directory()))
                            as diagnostics::Argument,
                    ],
                ));
            } else {
                tsc::print_version(sys.clone(), locale.clone());
                tsc::print_help(sys.clone(), locale.clone(), &command_line);
            }
            stop_profile_session(&mut profile_session);
            return tsc::CommandLineResult {
                status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED,
                ..Default::default()
            };
        }
    }

    // !!! convert to options with absolute paths is usually done here, but for ease of implementation, it's done in `tsoptions.ParseCommandLine()`
    let compiler_options_from_command_line = command_line.compiler_options();
    let mut config_for_compilation = command_line.clone();
    let extended_config_cache = tsc::ExtendedConfigCache::default();
    let mut compile_times = tsc::CompileTimes::default();
    if !config_file_name.is_empty() {
        let config_start = sys.now();
        let option_map = tsoptions::command_line_options_to_map(tsoptions::options_declarations());
        let mut command_line_raw = serde_json::Map::new();
        for (name, value) in &command_line.options {
            let option = option_map
                .get(name)
                .expect("command line option declaration");
            let raw_value = match value.as_str() {
                "null" => Value::Null,
                "true" if option.kind == Some(tsoptions::CommandLineOptionKind::Boolean) => {
                    Value::Bool(true)
                }
                "false" if option.kind == Some(tsoptions::CommandLineOptionKind::Boolean) => {
                    Value::Bool(false)
                }
                _ if option.kind == Some(tsoptions::CommandLineOptionKind::Number) => {
                    let number = value.parse::<i64>().expect("numeric command line option");
                    Value::Number(number.into())
                }
                _ => Value::String(value.clone()),
            };
            command_line_raw.insert(name.clone(), raw_value);
        }
        let options_raw = Value::Object(serde_json::Map::from_iter([(
            "compilerOptions".to_owned(),
            Value::Object(command_line_raw),
        )]));
        let (config_parse_result, errors) = tsoptions::get_parsed_command_line_of_config_file(
            &config_file_name,
            Some(&compiler_options_from_command_line),
            Some(&options_raw),
            &sys as &dyn tsoptions::ParseConfigHost,
            Some(&extended_config_cache),
        );
        compile_times.config_time = sys.now().duration_since(config_start).unwrap_or_default();
        if !errors.is_empty() {
            // these are unrecoverable errors--exit to report them as diagnostics
            for e in errors {
                report_diagnostic(command_line_error_diagnostic(e));
            }
            stop_profile_session(&mut profile_session);
            return tsc::CommandLineResult {
                status: tsc::EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_GENERATED,
                ..Default::default()
            };
        }
        config_for_compilation = config_parse_result.unwrap_or_default();
        apply_command_line_watch_options(&mut config_for_compilation, &command_line.watch_options);
        // Updater to reflect pretty
        report_diagnostic = tsc::create_diagnostic_reporter(
            sys.clone(),
            Box::new(sys.clone()),
            locale.clone(),
            command_line.compiler_options(),
        );
    }

    let report_error_summary = tsc::create_report_error_summary(
        sys.clone(),
        locale.clone(),
        config_for_compilation.compiler_options(),
    );
    if compiler_options_from_command_line.show_config.is_true() {
        show_config(sys.clone(), &config_for_compilation, &config_file_name);
        stop_profile_session(&mut profile_session);
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_SUCCESS,
            ..Default::default()
        };
    }
    if config_for_compilation.compiler_options().watch.is_true() {
        let mut watcher = watcher::create_watcher(
            sys.clone(),
            config_for_compilation,
            compiler_options_from_command_line,
            report_diagnostic,
            report_error_summary,
            command_line.watch_options.clone(),
            testing,
        );
        watcher.start();
        stop_profile_session(&mut profile_session);
        return tsc::CommandLineResult {
            status: tsc::EXIT_STATUS_SUCCESS,
            watcher: Some(Box::new(watcher)),
        };
    } else if config_for_compilation.compiler_options().is_incremental() {
        let result = perform_incremental_compilation(
            sys,
            &config_for_compilation,
            report_diagnostic,
            report_error_summary,
            extended_config_cache,
            &mut compile_times,
            testing,
        );
        stop_profile_session(&mut profile_session);
        return result;
    }
    let result = perform_compilation(
        sys,
        &config_for_compilation,
        report_diagnostic,
        report_error_summary,
        extended_config_cache,
        &mut compile_times,
        testing,
    );
    stop_profile_session(&mut profile_session);
    result
}

pub fn find_config_file(
    search_path: String,
    file_exists: impl Fn(&str) -> bool,
    config_name: &str,
) -> String {
    let Some(result) = tspath::for_each_ancestor_directory(search_path, |ancestor| {
        let full_config_name = tspath::combine_paths(ancestor, &[config_name]);
        if file_exists(&full_config_name) {
            return Some(full_config_name);
        }
        None
    }) else {
        return String::new();
    };
    result
}

pub fn get_trace_from_sys(
    sys: tsc::System,
    locale: locale::Locale,
    _testing: Option<tsc::CommandLineTesting>,
) -> Option<Box<compiler::Trace>> {
    let writer = std::sync::Mutex::new(sys);
    Some(Box::new(move |msg, args| {
        let args = args.iter().cloned().collect();
        let mut writer = writer.lock().unwrap_or_else(|err| err.into_inner());
        let _ = writeln!(writer.writer(), "{}", msg.localize(locale.clone(), args));
    }))
}

pub fn perform_incremental_compilation(
    sys: tsc::System,
    config: &tsoptions::ParsedCommandLine,
    report_diagnostic: tsc::DiagnosticReporter,
    report_error_summary: tsc::DiagnosticsReporter,
    extended_config_cache: tsc::ExtendedConfigCache,
    compile_times: &mut tsc::CompileTimes,
    testing: Option<tsc::CommandLineTesting>,
) -> tsc::CommandLineResult {
    let host: Arc<dyn compiler::CompilerHost> = compiler::new_cached_fs_compiler_host(
        sys.get_current_directory(),
        Box::new(sys.clone()),
        sys.default_library_path(),
        Some(Box::new(extended_config_cache)),
        get_trace_from_sys(sys.clone(), config.locale(), testing.clone()),
    )
    .into();
    let build_info_read_start = sys.now();
    let old_program = incremental::read_build_info_program(
        config,
        &incremental::new_build_info_reader(Arc::clone(&host)),
        host.as_ref(),
    );
    compile_times.build_info_read_time = sys
        .now()
        .duration_since(build_info_read_start)
        .unwrap_or_default();

    let tr = start_tracing_if_needed(sys.clone(), config, testing.clone());

    let parse_start = sys.now();
    let program = compiler::new_program(program_options(config, Arc::clone(&host), tr.clone()));
    compile_times.parse_time = sys.now().duration_since(parse_start).unwrap_or_default();
    let changes_compute_start = sys.now();
    let mut incremental_program = incremental::new_program(
        program,
        old_program.as_ref(),
        incremental::create_host(Arc::clone(&host)),
        testing.is_some(),
    );
    compile_times.changes_compute_time = sys
        .now()
        .duration_since(changes_compute_start)
        .unwrap_or_default();
    let (result, _) = tsc::emit_and_report_statistics(tsc::EmitInput {
        sys: sys.clone(),
        program_like: &mut incremental_program,
        config: config.clone(),
        report_diagnostic,
        report_error_summary,
        writer: Box::new(sys.clone()),
        write_file: None,
        compile_times: compile_times.clone(),
        testing: testing.clone(),
        testing_m_times_cache: None,
        tracing: tr.clone(),
    });

    stop_tracing(sys.clone(), tr);

    if let Some(testing) = testing {
        testing.on_program(&incremental_program);
    }
    tsc::CommandLineResult {
        status: result.status,
        ..Default::default()
    }
}

pub fn perform_compilation(
    sys: tsc::System,
    config: &tsoptions::ParsedCommandLine,
    report_diagnostic: tsc::DiagnosticReporter,
    report_error_summary: tsc::DiagnosticsReporter,
    extended_config_cache: tsc::ExtendedConfigCache,
    compile_times: &mut tsc::CompileTimes,
    testing: Option<tsc::CommandLineTesting>,
) -> tsc::CommandLineResult {
    let host: Arc<dyn compiler::CompilerHost> = compiler::new_cached_fs_compiler_host(
        sys.get_current_directory(),
        Box::new(sys.clone()),
        sys.default_library_path(),
        Some(Box::new(extended_config_cache)),
        get_trace_from_sys(sys.clone(), config.locale(), testing.clone()),
    )
    .into();

    let tr = start_tracing_if_needed(sys.clone(), config, testing.clone());

    let parse_start = sys.now();
    let mut program = compiler::new_program(program_options(config, host, tr.clone()));
    compile_times.parse_time = sys.now().duration_since(parse_start).unwrap_or_default();
    let (result, _) = tsc::emit_and_report_statistics(tsc::EmitInput {
        sys: sys.clone(),
        program_like: &mut program,
        config: config.clone(),
        report_diagnostic,
        report_error_summary,
        writer: Box::new(sys.clone()),
        write_file: None,
        compile_times: compile_times.clone(),
        testing: testing.clone(),
        testing_m_times_cache: None,
        tracing: tr.clone(),
    });

    stop_tracing(sys.clone(), tr);

    tsc::CommandLineResult {
        status: result.status,
        ..Default::default()
    }
}

pub fn show_config(
    mut sys: tsc::System,
    config: &tsoptions::ParsedCommandLine,
    config_file_name: &str,
) {
    let ts_config = tsoptions::convert_to_ts_config(config, config_file_name);
    let mut writer = sys.writer();
    let _ = json::marshal_indent_write(&mut writer, &ts_config, "", "    ");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_error_diagnostic_converts_build_option_combination_errors() {
        let diagnostic = command_line_error_diagnostic(
            "Options_0_and_1_cannot_be_combined: clean\u{1f}force".to_owned(),
        );

        assert_eq!(
            diagnostic.code(),
            diagnostics::Options_0_and_1_cannot_be_combined.code()
        );
        assert_eq!(
            diagnostic.localize(locale::und()),
            "Options 'clean' and 'force' cannot be combined."
        );
    }

    #[test]
    fn command_line_watch_options_override_config_parse_result() {
        let mut config = tsoptions::ParsedCommandLine::default();
        config
            .watch_options
            .insert("watchFile".to_owned(), "UseFsEvents".to_owned());
        let command_line_watch_options =
            BTreeMap::from([("watchInterval".to_owned(), "1000".to_owned())]);

        apply_command_line_watch_options(&mut config, &command_line_watch_options);

        assert_eq!(
            config
                .watch_options
                .get("watchInterval")
                .map(String::as_str),
            Some("1000")
        );
        assert_eq!(
            config.watch_options.get("watchFile").map(String::as_str),
            Some("UseFsEvents")
        );
    }
}
