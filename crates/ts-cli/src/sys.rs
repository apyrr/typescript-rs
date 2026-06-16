#![allow(dead_code)]

use std::{
    env,
    io::{self, IsTerminal},
    process,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use ts_bundled as bundled;
use ts_execute::tsc;
use ts_tspath as tspath;
use ts_vfs::{self as vfs, osvfs};

pub struct OsSys {
    writer: io::Stdout,
    fs: Arc<dyn vfs::Fs + Send + Sync>,
    default_library_path: String,
    cwd: String,
    start: Instant,
}

impl Clone for OsSys {
    fn clone(&self) -> Self {
        Self {
            writer: io::stdout(),
            fs: Arc::clone(&self.fs),
            default_library_path: self.default_library_path.clone(),
            cwd: self.cwd.clone(),
            start: self.start,
        }
    }
}

impl OsSys {
    pub fn since_start(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    pub fn fs(&self) -> &dyn vfs::Fs {
        &*self.fs
    }

    pub fn default_library_path(&self) -> &str {
        &self.default_library_path
    }

    pub fn get_current_directory(&self) -> &str {
        &self.cwd
    }

    pub fn writer(&self) -> &io::Stdout {
        &self.writer
    }

    pub fn write_output_is_tty(&self) -> bool {
        io::stdout().is_terminal()
    }

    pub fn get_width_of_terminal(&self) -> i32 {
        get_stdout_terminal_width()
    }

    pub fn get_environment_variable(&self, name: &str) -> String {
        // PORT NOTE: lossy conversion matches Go's string-shaped environment
        // API for now; revisit if env byte fidelity becomes observable.
        env::var_os(name)
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

impl tsc::SystemInterface for OsSys {
    fn writer(&mut self) -> &mut dyn io::Write {
        &mut self.writer
    }

    fn fs(&self) -> &dyn tsc::FileSystem {
        &*self.fs
    }

    fn default_library_path(&self) -> String {
        self.default_library_path.clone()
    }

    fn get_current_directory(&self) -> String {
        self.cwd.clone()
    }

    fn write_output_is_tty(&self) -> bool {
        self.write_output_is_tty()
    }

    fn get_width_of_terminal(&self) -> i32 {
        self.get_width_of_terminal()
    }

    fn get_environment_variable(&self, name: &str) -> String {
        self.get_environment_variable(name)
    }

    fn now(&self) -> SystemTime {
        self.now()
    }

    fn since_start(&self) -> Duration {
        self.since_start()
    }
}

#[cfg(unix)]
fn get_stdout_terminal_width() -> i32 {
    terminal_size::terminal_size_of(io::stdout())
        .map(|(terminal_size::Width(width), _)| width.into())
        .unwrap_or_default()
}

#[cfg(windows)]
fn get_stdout_terminal_width() -> i32 {
    terminal_size::terminal_size_of(io::stdout())
        .map(|(terminal_size::Width(width), _)| width.into())
        .unwrap_or_default()
}

#[cfg(not(any(unix, windows)))]
fn get_stdout_terminal_width() -> i32 {
    0
}

pub fn new_system() -> OsSys {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(err) => {
            eprintln!("Error getting current directory: {err}");
            process::exit(tsc::ExitStatus::InvalidProjectOutputsSkipped as i32);
        }
    };

    // PORT NOTE: lossy conversion matches Go's string-shaped working-directory
    // API for now; revisit when tspath/path byte handling is ported.
    let cwd = cwd.to_string_lossy().into_owned();

    OsSys {
        cwd: tspath::normalize_path(&cwd),
        fs: Arc::new(bundled::wrap_fs(osvfs::os::fs())),
        default_library_path: bundled::lib_path(),
        writer: io::stdout(),
        start: Instant::now(),
    }
}
