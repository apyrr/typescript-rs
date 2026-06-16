mod compile;
mod diagnostics;
mod emit;
mod extendedconfigcache;
mod help;
mod init;
mod statistics;

pub use compile::{
    CommandLineResult, CommandLineTesting, CommandLineTestingInterface, CompileAndEmitResult,
    CompileTimes, Diagnostic, DiagnosticsMessage,
    EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_GENERATED,
    EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED, EXIT_STATUS_INVALID_PROJECT_OUTPUTS_SKIPPED,
    EXIT_STATUS_NOT_IMPLEMENTED, EXIT_STATUS_PROJECT_REFERENCE_CYCLE_OUTPUTS_SKIPPED,
    EXIT_STATUS_SUCCESS, EmitResult, ExitStatus, FileSystem, Locale, MTimesCache, Path, System,
    SystemInterface, Watcher,
};
use diagnostics::{Colors, create_colors};
pub use diagnostics::{
    DiagnosticReporter, DiagnosticsReporter, create_builder_status_reporter,
    create_diagnostic_reporter, create_report_error_summary, create_watch_status_reporter,
    quiet_diagnostic_reporter, quiet_diagnostics_reporter,
};
pub use emit::{
    EmitInput, emit_and_report_statistics, emit_files_and_report_errors,
    get_trace_with_writer_from_sys,
};
pub use extendedconfigcache::{ExtendedConfigCache, ExtendedConfigCacheEntry, ParseConfigHost};
use help::get_header;
pub use help::{print_build_help, print_help, print_version};
pub use init::write_config_file;
pub use statistics::Statistics;
use statistics::{MemoryStats, statistics_from_program};
