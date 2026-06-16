use std::{
    io::{self, Write},
    time::{Duration, SystemTime},
};

use serde_json::Value;

use crate::incremental::Program;
use ts_ast as ast;
use ts_collections::SyncMap;
use ts_compiler as compiler;
use ts_diagnostics as diagnostics;
use ts_locale as locale;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs as vfs;

pub type EmitResult = compiler::EmitResult;
pub type Diagnostic = ast::Diagnostic;
pub type DiagnosticsMessage = diagnostics::Message;
pub type Locale = locale::Locale;
pub type Path = tspath::Path;
pub type MTimesCache = SyncMap<Path, SystemTime>;
pub use vfs::Fs as FileSystem;

pub type System = Box<dyn SystemInterface>;

pub trait SystemClone {
    fn clone_box(&self) -> Box<dyn SystemInterface>;
}

impl<T> SystemClone for T
where
    T: SystemInterface + Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn SystemInterface> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn SystemInterface> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait SystemInterface: SystemClone + Send + Sync {
    fn writer(&mut self) -> &mut dyn Write;
    fn fs(&self) -> &dyn FileSystem;
    fn default_library_path(&self) -> String;
    fn get_current_directory(&self) -> String;
    fn write_output_is_tty(&self) -> bool;
    fn get_width_of_terminal(&self) -> i32;
    fn get_environment_variable(&self, name: &str) -> String;

    fn now(&self) -> SystemTime;
    fn since_start(&self) -> Duration;
}

impl Write for Box<dyn SystemInterface> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer().flush()
    }
}

impl vfs::Fs for Box<dyn SystemInterface> {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs().use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.fs().file_exists(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.fs().read_file(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs().write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs().append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.fs().remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.fs().chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.fs().directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        self.fs().get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        self.fs().stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        self.fs().walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        self.fs().realpath(path)
    }
}

impl tsoptions::ParseConfigHost for Box<dyn SystemInterface> {
    fn fs(&self) -> &dyn vfs::Fs {
        self.as_ref().fs()
    }

    fn get_current_directory(&self) -> String {
        self.as_ref().get_current_directory()
    }
}

impl tsoptions::ParseConfigHost for dyn SystemInterface {
    fn fs(&self) -> &dyn vfs::Fs {
        SystemInterface::fs(self)
    }

    fn get_current_directory(&self) -> String {
        SystemInterface::get_current_directory(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum ExitStatus {
    Success = 0,
    DiagnosticsPresentOutputsGenerated = 1,
    DiagnosticsPresentOutputsSkipped = 2,
    InvalidProjectOutputsSkipped = 3,
    ProjectReferenceCycleOutputsSkipped = 4,
    NotImplemented = 5,
}

pub const EXIT_STATUS_SUCCESS: ExitStatus = ExitStatus::Success;
pub const EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_GENERATED: ExitStatus =
    ExitStatus::DiagnosticsPresentOutputsGenerated;
pub const EXIT_STATUS_DIAGNOSTICS_PRESENT_OUTPUTS_SKIPPED: ExitStatus =
    ExitStatus::DiagnosticsPresentOutputsSkipped;
pub const EXIT_STATUS_INVALID_PROJECT_OUTPUTS_SKIPPED: ExitStatus =
    ExitStatus::InvalidProjectOutputsSkipped;
pub const EXIT_STATUS_PROJECT_REFERENCE_CYCLE_OUTPUTS_SKIPPED: ExitStatus =
    ExitStatus::ProjectReferenceCycleOutputsSkipped;
pub const EXIT_STATUS_NOT_IMPLEMENTED: ExitStatus = ExitStatus::NotImplemented;

impl Default for ExitStatus {
    fn default() -> Self {
        Self::Success
    }
}

pub trait Watcher {
    fn do_cycle(&mut self);
}

pub struct CommandLineResult {
    pub status: ExitStatus,
    pub watcher: Option<Box<dyn Watcher>>,
}

impl Default for CommandLineResult {
    fn default() -> Self {
        Self {
            status: ExitStatus::Success,
            watcher: None,
        }
    }
}

pub type CommandLineTesting = Box<dyn CommandLineTestingInterface>;

pub trait CommandLineTestingClone {
    fn clone_box(&self) -> Box<dyn CommandLineTestingInterface>;
}

impl<T> CommandLineTestingClone for T
where
    T: CommandLineTestingInterface + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn CommandLineTestingInterface> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn CommandLineTestingInterface> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait CommandLineTestingInterface: CommandLineTestingClone + Send + Sync {
    // Ensure that all emitted files are timestamped in order to ensure they are deterministic for test baseline
    fn on_emitted_files(&self, result: &EmitResult, m_times_cache: Option<&MTimesCache>);
    fn on_list_files_start(&self, w: &mut dyn Write);
    fn on_list_files_end(&self, w: &mut dyn Write);
    fn on_statistics_start(&self, w: &mut dyn Write);
    fn on_statistics_end(&self, w: &mut dyn Write);
    fn on_build_status_report_start(&self, w: &mut dyn Write);
    fn on_build_status_report_end(&self, w: &mut dyn Write);
    fn on_watch_status_report_start(&self);
    fn on_watch_status_report_end(&self);
    fn get_trace(
        &self,
        w: Box<dyn Write + Send>,
        locale: Locale,
        use_package_json_cache: bool,
    ) -> Box<dyn Fn(&DiagnosticsMessage, Vec<Value>) + Send + Sync>;
    fn on_program(&self, program: &Program);
}

#[derive(Clone, Default)]
pub struct CompileTimes {
    pub config_time: Duration,
    pub parse_time: Duration,
    pub(crate) bind_time: Duration,
    pub(crate) check_time: Duration,
    pub(crate) total_time: Duration,
    pub(crate) emit_time: Duration,
    pub build_info_read_time: Duration,
    pub changes_compute_time: Duration,
}

#[derive(Default)]
pub struct CompileAndEmitResult {
    pub diagnostics: Vec<Diagnostic>,
    pub emit_result: Option<EmitResult>,
    pub status: ExitStatus,
    pub(crate) times: CompileTimes,
}
