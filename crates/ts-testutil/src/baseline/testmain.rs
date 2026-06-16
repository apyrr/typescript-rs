#![allow(dead_code)]

use std::backtrace::Backtrace;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::panic::Location;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static TRACKING_INITIALIZED: AtomicBool = AtomicBool::new(false);
static PROCESS_TRACKING_PATH: OnceLock<PathBuf> = OnceLock::new();

const FNV_OFFSET_BASIS_64: u64 = 0xcbf29ce484222325;
const FNV_PRIME_64: u64 = 0x00000100000001b3;

#[track_caller]
pub fn track() -> Box<dyn FnOnce()> {
    TRACKING_INITIALIZED.store(true, Ordering::SeqCst);
    let Some(tracking_path) = tracking_path_for_caller(Location::caller()) else {
        return Box::new(|| {});
    };

    Box::new(move || {
        write_recorded_baselines(&tracking_path);
    })
}

#[track_caller]
pub fn track_process() {
    TRACKING_INITIALIZED.store(true, Ordering::SeqCst);
    let Some(tracking_path) = tracking_path_for_caller(Location::caller()) else {
        return;
    };
    let _ = PROCESS_TRACKING_PATH.set(tracking_path);
}

pub(crate) fn record_baseline_tracking(relative_path: &str) {
    if tracking_dir().as_os_str().is_empty() {
        return;
    }
    if !TRACKING_INITIALIZED.load(Ordering::SeqCst) {
        eprintln!(
            "baseline: package uses baselines but TestMain did not call baseline.Track(). Please add a TestMain function with: defer baseline.Track()()"
        );
        return;
    }
    {
        recorded_baselines()
            .lock()
            .expect("recorded baseline mutex poisoned")
            .insert(relative_path.to_string());
    }
    if let Some(tracking_path) = PROCESS_TRACKING_PATH.get() {
        write_recorded_baselines(tracking_path);
    }
}

pub fn write_recorded_baselines(tracking_path: impl AsRef<Path>) {
    if recorded_baselines()
        .lock()
        .expect("recorded baseline mutex poisoned")
        .is_empty()
    {
        return;
    }
    if let Err(err) = do_write_recorded_baselines(tracking_path.as_ref()) {
        eprintln!(
            "baseline: failed to write tracking file {}: {err}",
            tracking_path.as_ref().display()
        );
        std::process::exit(1);
    }
}

pub fn do_write_recorded_baselines(tracking_path: &Path) -> std::io::Result<()> {
    let file = File::create(tracking_path)?;
    let mut writer = BufWriter::new(file);
    let baselines = recorded_baselines()
        .lock()
        .expect("recorded baseline mutex poisoned");
    for baseline in baselines.iter() {
        writeln!(writer, "{baseline}")?;
    }
    writer.flush()
}

fn recorded_baselines() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

fn tracking_dir() -> PathBuf {
    static TRACKING_DIR: OnceLock<PathBuf> = OnceLock::new();
    TRACKING_DIR
        .get_or_init(|| {
            env::var_os("TSGO_BASELINE_TRACKING_DIR")
                .map(PathBuf::from)
                .unwrap_or_default()
        })
        .clone()
}

fn tracking_path_for_caller(location: &'static Location<'static>) -> Option<PathBuf> {
    let tracking_dir = tracking_dir();
    if tracking_dir.as_os_str().is_empty() {
        return None;
    }

    let hash = tracking_hash_for_caller(location);
    Some(tracking_dir.join(format!("{hash:016x}.txt")))
}

fn tracking_hash_for_caller(location: &'static Location<'static>) -> u64 {
    let mut hash = FNV_OFFSET_BASIS_64;
    for file in caller_stack_files(location) {
        fnv1a_64_write(&mut hash, file.as_bytes());
    }
    hash
}

fn caller_stack_files(location: &'static Location<'static>) -> Vec<String> {
    let caller_file = location.file().to_string();
    let mut files = vec![caller_file.clone()];

    // PORT NOTE: stable Rust does not expose typed stack frames like Go's
    // runtime.CallersFrames; Backtrace Debug text is the available runtime stack
    // file source, with the tracked caller location kept deterministic.
    let parsed_files = parse_backtrace_files(&format!("{:?}", Backtrace::force_capture()));
    if let Some(caller_index) = parsed_files.iter().position(|file| file == &caller_file) {
        files.extend(parsed_files.into_iter().skip(caller_index + 1));
    } else {
        files.extend(parsed_files.into_iter().filter(|file| file != &caller_file));
    }

    files
}

fn parse_backtrace_files(backtrace: &str) -> Vec<String> {
    let mut files = Vec::new();
    for line in backtrace.lines() {
        if let Some((_, location)) = line.split_once(" at ") {
            if let Some(file) = parse_source_location(location) {
                files.push(file);
            }
            continue;
        }

        if let Some((_, rest)) = line.split_once("file: \"")
            && let Some((file, _)) = rest.split_once('"')
        {
            files.push(file.to_string());
        }
    }
    files
}

fn parse_source_location(location: &str) -> Option<String> {
    let (before_column, column) = location.rsplit_once(':')?;
    if column.parse::<u32>().is_ok() {
        if let Some((file, line)) = before_column.rsplit_once(':')
            && line.parse::<u32>().is_ok()
        {
            return Some(file.to_string());
        }
        return Some(before_column.to_string());
    }

    None
}

fn fnv1a_64_write(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME_64);
    }
}
