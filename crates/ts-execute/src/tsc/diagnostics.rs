use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use chrono::{DateTime, Local};
use ts_ast as ast;
use ts_core as core;
use ts_diagnosticwriter as diagnosticwriter;
use ts_locale as locale;
use ts_tspath as tspath;

use super::{CommandLineTesting, System};

pub fn get_format_opts_of_sys(
    sys: System,
    locale: locale::Locale,
) -> diagnosticwriter::FormattingOptions {
    diagnosticwriter::FormattingOptions {
        new_line: "\n".to_owned(),
        compare_paths_options: tspath::ComparePathsOptions {
            current_directory: sys.get_current_directory(),
            use_case_sensitive_file_names: sys.fs().use_case_sensitive_file_names(),
        },
        locale,
    }
}

pub type DiagnosticReporter = Arc<dyn Fn(ast::Diagnostic) + Send + Sync>;

pub fn quiet_diagnostic_reporter(_diagnostic: ast::Diagnostic) {}

pub fn create_diagnostic_reporter(
    sys: System,
    w: Box<dyn Write + Send>,
    locale: locale::Locale,
    options: core::CompilerOptions,
) -> DiagnosticReporter {
    if options.quiet.is_true() {
        return Arc::new(quiet_diagnostic_reporter);
    }
    let format_opts = get_format_opts_of_sys(sys.clone(), locale);
    let writer = Mutex::new(w);
    if should_be_pretty(sys, Some(options.clone())) {
        return Arc::new(move |diagnostic| {
            let mut writer = writer.lock().unwrap_or_else(|err| err.into_inner());
            let writer_diagnostic = diagnosticwriter::wrap_ast_diagnostic(diagnostic);
            diagnosticwriter::format_diagnostic_with_color_and_context(
                writer.as_mut(),
                &writer_diagnostic,
                &format_opts,
            );
            let _ = write!(writer, "{}", format_opts.new_line);
        });
    }
    Arc::new(move |diagnostic| {
        let mut writer = writer.lock().unwrap_or_else(|err| err.into_inner());
        let writer_diagnostic = diagnosticwriter::wrap_ast_diagnostic(diagnostic);
        diagnosticwriter::write_format_diagnostic(
            writer.as_mut(),
            &writer_diagnostic,
            &format_opts,
        );
    })
}

fn format_time(time: SystemTime) -> String {
    let time: DateTime<Local> = time.into();
    time.format("%I:%M:%S %p").to_string()
}

pub fn default_is_pretty(sys: System) -> bool {
    if !sys.get_environment_variable("NO_COLOR").is_empty() {
        return false;
    }
    if !sys.get_environment_variable("FORCE_COLOR").is_empty() {
        return true;
    }
    sys.write_output_is_tty()
}

pub fn should_be_pretty(sys: System, options: Option<core::CompilerOptions>) -> bool {
    if options
        .as_ref()
        .is_none_or(|options| options.pretty.is_unknown())
    {
        return default_is_pretty(sys);
    }
    options.unwrap().pretty.is_true()
}

pub struct Colors {
    pub show_colors: bool,

    pub is_windows: bool,
    pub is_windows_terminal: bool,
    pub is_vs_code: bool,
    pub supports_richer_colors: bool,
}

pub fn create_colors(sys: System) -> Colors {
    if !default_is_pretty(sys.clone()) {
        return Colors {
            show_colors: false,
            is_windows: false,
            is_windows_terminal: false,
            is_vs_code: false,
            supports_richer_colors: false,
        };
    }

    let os = sys.get_environment_variable("OS");
    let is_windows = os.to_lowercase().contains("windows");
    let is_windows_terminal = !sys.get_environment_variable("WT_SESSION").is_empty();
    let is_vs_code = sys.get_environment_variable("TERM_PROGRAM") == "vscode";
    let supports_richer_colors = sys.get_environment_variable("COLORTERM") == "truecolor"
        || sys.get_environment_variable("TERM") == "xterm-256color";

    Colors {
        show_colors: true,
        is_windows,
        is_windows_terminal,
        is_vs_code,
        supports_richer_colors,
    }
}

impl Colors {
    pub fn bold(&self, str_: String) -> String {
        if !self.show_colors {
            return str_;
        }
        format!("\x1b[1m{str_}\x1b[22m")
    }

    pub fn blue(&self, str_: String) -> String {
        if !self.show_colors {
            return str_;
        }

        // Effectively Powershell and Command prompt users use cyan instead
        // of blue because the default theme doesn't show blue with enough contrast.
        if self.is_windows && !self.is_windows_terminal && !self.is_vs_code {
            return self.bright_white(str_);
        }
        format!("\x1b[94m{str_}\x1b[39m")
    }

    pub fn blue_background(&self, str_: String) -> String {
        if !self.show_colors {
            return str_;
        }
        if self.supports_richer_colors {
            format!("\x1B[48;5;68m{str_}\x1B[39;49m")
        } else {
            format!("\x1b[44m{str_}\x1B[39;49m")
        }
    }

    pub fn bright_white(&self, str_: String) -> String {
        if !self.show_colors {
            return str_;
        }
        format!("\x1b[97m{str_}\x1b[39m")
    }
}

pub type DiagnosticsReporter = Arc<dyn Fn(Vec<ast::Diagnostic>) + Send + Sync>;

pub fn quiet_diagnostics_reporter(_diagnostics: Vec<ast::Diagnostic>) {}

pub fn create_report_error_summary(
    sys: System,
    locale: locale::Locale,
    options: core::CompilerOptions,
) -> DiagnosticsReporter {
    if should_be_pretty(sys.clone(), Some(options)) {
        let format_opts = get_format_opts_of_sys(sys.clone(), locale);
        return Arc::new(move |diagnostics| {
            let mut sys = sys.clone();
            diagnosticwriter::write_error_summary_text(
                sys.writer(),
                diagnosticwriter::from_ast_diagnostics(diagnostics),
                &format_opts,
            );
        });
    }
    Arc::new(quiet_diagnostics_reporter)
}

pub fn create_builder_status_reporter(
    sys: System,
    w: Box<dyn Write + Send>,
    locale: locale::Locale,
    options: core::CompilerOptions,
    testing: Option<CommandLineTesting>,
) -> DiagnosticReporter {
    if options.quiet.is_true() {
        return Arc::new(quiet_diagnostic_reporter);
    }

    let format_opts = get_format_opts_of_sys(sys.clone(), locale);
    let write_status = if should_be_pretty(sys.clone(), Some(options.clone())) {
        diagnosticwriter::format_diagnostics_status_with_color_and_time
    } else {
        diagnosticwriter::format_diagnostics_status_and_time
    };
    let writer = Mutex::new(w);
    Arc::new(move |diagnostic| {
        let mut writer = writer.lock().unwrap_or_else(|err| err.into_inner());
        let writer_diagnostic = diagnosticwriter::wrap_ast_diagnostic(diagnostic);
        if let Some(testing) = &testing {
            testing.on_build_status_report_start(writer.as_mut());
        }
        write_status(
            writer.as_mut(),
            &format_time(sys.now()),
            &writer_diagnostic,
            &format_opts,
        );
        let _ = write!(writer, "{}{}", format_opts.new_line, format_opts.new_line);
        if let Some(testing) = &testing {
            testing.on_build_status_report_end(writer.as_mut());
        }
    })
}

pub fn create_watch_status_reporter(
    sys: System,
    locale: locale::Locale,
    options: core::CompilerOptions,
    testing: Option<CommandLineTesting>,
) -> DiagnosticReporter {
    let format_opts = get_format_opts_of_sys(sys.clone(), locale);
    let write_status = if should_be_pretty(sys.clone(), Some(options.clone())) {
        diagnosticwriter::format_diagnostics_status_with_color_and_time
    } else {
        diagnosticwriter::format_diagnostics_status_and_time
    };
    let time_sys = sys.clone();
    let sys = std::sync::Mutex::new(sys);
    Arc::new(move |diagnostic| {
        let writer_diagnostic = diagnosticwriter::wrap_ast_diagnostic(diagnostic);
        let mut sys = sys.lock().unwrap_or_else(|err| err.into_inner());
        let mut writer = sys.writer();
        if let Some(testing) = &testing {
            testing.on_watch_status_report_start();
        }
        diagnosticwriter::try_clear_screen(&mut writer, &writer_diagnostic, &options);
        write_status(
            &mut writer,
            &format_time(time_sys.now()),
            &writer_diagnostic,
            &format_opts,
        );
        let _ = write!(writer, "{}{}", format_opts.new_line, format_opts.new_line);
        if let Some(testing) = &testing {
            testing.on_watch_status_report_end();
        }
    })
}
