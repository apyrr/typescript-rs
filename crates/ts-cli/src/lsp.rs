use std::{
    env, io,
    process::Command,
    sync::{
        Arc, Condvar, LazyLock, Mutex, Once,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{PROCESS_ALIVE_SUPPORTED, is_process_alive};
use ts_bundled as bundled;
use ts_core as core;
use ts_lsp as lsp;
#[cfg(feature = "pprof")]
use ts_pprof as pprof;
use ts_vfs::osvfs;

pub fn run_lsp(args: &[String]) -> i32 {
    let mut flags = LspFlags::default();
    if !flags.parse(args) {
        return 2;
    }

    if !flags.stdio {
        eprintln!("only stdio is supported");
        return 1;
    }

    #[cfg(feature = "pprof")]
    let profile_session = if !flags.pprof_dir.is_empty() {
        eprintln!("pprof profiles will be written to: {}", flags.pprof_dir);
        Some(pprof::begin_profiling(&flags.pprof_dir, io::stderr()))
    } else {
        None
    };
    #[cfg(not(feature = "pprof"))]
    if !flags.pprof_dir.is_empty() {
        eprintln!(
            "pprof profiling is disabled in this Rust build; ignoring {}",
            flags.pprof_dir
        );
    }

    let fs = bundled::wrap_fs(osvfs::os::fs());
    let default_library_path = bundled::lib_path();
    let typings_location = osvfs::os::get_global_typings_cache_location();

    let (watchdog_ctx, stop) = notify_context(context_background());

    // PORT NOTE: lossy conversion matches Go's string-shaped working-directory
    // API for now; revisit when tspath/path byte handling is ported.
    let cwd = core::must(env::current_dir())
        .to_string_lossy()
        .into_owned();
    let mut server = lsp::new_server(lsp::ServerOptions {
        r#in: Some(Box::new(lsp::to_reader(io::stdin()))),
        out: Some(Box::new(lsp::to_writer(io::stdout()))),
        err: Some(Box::new(io::stderr())),
        cwd,
        fs: Some(Arc::new(fs)),
        default_library_path,
        typings_location,
        parse_cache: None,
        npm_install: Some(npm_install),
        progress_delay: Duration::from_millis(250),
        set_parent_process_id: new_parent_process_watchdog(watchdog_ctx.clone(), stop.clone())
            .map(register_parent_process_watchdog),
    });

    let run_result = server.run(core::Context::background());
    if let Err(err) = run_result {
        eprintln!("{err}");
        stop();
        #[cfg(feature = "pprof")]
        if let Some(mut profile_session) = profile_session {
            profile_session.stop();
        }
        return 1;
    }

    stop();
    #[cfg(feature = "pprof")]
    if let Some(mut profile_session) = profile_session {
        profile_session.stop();
    }
    0
}

fn npm_install(cwd: String, args: Vec<String>) -> Result<Vec<u8>, io::Error> {
    let output = Command::new("npm").args(args).current_dir(cwd).output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(io::Error::other(exit_status_error(&output.status)))
    }
}

fn exit_status_error(status: &std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit status {code}");
    }
    status.to_string()
}

#[derive(Default)]
struct LspFlags {
    stdio: bool,
    pprof_dir: String,
    _pipe: String,
    _socket: String,
}

impl LspFlags {
    fn parse(&mut self, args: &[String]) -> bool {
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                break;
            }
            if !arg.starts_with('-') || arg == "-" {
                break;
            }

            let trimmed = if let Some(trimmed) = arg.strip_prefix("--") {
                trimmed
            } else if let Some(trimmed) = arg.strip_prefix('-') {
                trimmed
            } else {
                return false;
            };
            if trimmed.starts_with('-') {
                return false;
            }
            let (name, inline_value) = match trimmed.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (trimmed, None),
            };

            match name {
                "stdio" => {
                    let Some(value) = parse_bool_flag(inline_value.as_deref()) else {
                        return false;
                    };
                    self.stdio = value;
                }
                "pprofDir" => {
                    let Some(value) = inline_value.or_else(|| next_arg(args, &mut i)) else {
                        return false;
                    };
                    self.pprof_dir = value;
                }
                "pipe" => {
                    let Some(value) = inline_value.or_else(|| next_arg(args, &mut i)) else {
                        return false;
                    };
                    self._pipe = value;
                }
                "socket" => {
                    let Some(value) = inline_value.or_else(|| next_arg(args, &mut i)) else {
                        return false;
                    };
                    self._socket = value;
                }
                _ => return false,
            }
            i += 1;
        }
        true
    }
}

fn parse_bool_flag(value: Option<&str>) -> Option<bool> {
    match value {
        None => Some(true),
        Some("1" | "t" | "T" | "true" | "TRUE" | "True") => Some(true),
        Some("0" | "f" | "F" | "false" | "FALSE" | "False") => Some(false),
        _ => None,
    }
}

fn next_arg(args: &[String], i: &mut usize) -> Option<String> {
    *i += 1;
    args.get(*i).cloned()
}

type CancelFunc = Arc<dyn Fn() + Send + Sync + 'static>;

#[derive(Clone, Default)]
pub struct WatchdogContext {
    done: Arc<(Mutex<bool>, Condvar)>,
}

impl WatchdogContext {
    fn wait_timeout(&self, timeout: Duration) -> bool {
        let done = self.done.0.lock().unwrap_or_else(|err| err.into_inner());
        if *done {
            return true;
        }
        let (done, _) = self
            .done
            .1
            .wait_timeout(done, timeout)
            .unwrap_or_else(|err| err.into_inner());
        *done
    }
}

type ParentProcessWatchdog = Box<dyn Fn(i32) + Send + Sync>;

static PARENT_PROCESS_WATCHDOG: Mutex<Option<ParentProcessWatchdog>> = Mutex::new(None);
static SIGNAL_CANCELLED: LazyLock<Arc<AtomicBool>> =
    LazyLock::new(|| Arc::new(AtomicBool::new(false)));
static SIGNAL_INIT: Once = Once::new();

fn context_background() -> WatchdogContext {
    WatchdogContext::default()
}

fn notify_context(ctx: WatchdogContext) -> (WatchdogContext, CancelFunc) {
    install_signal_handlers();
    let done = ctx.done.clone();
    let stop = Arc::new(move || {
        let mut cancelled = done.0.lock().unwrap_or_else(|err| err.into_inner());
        *cancelled = true;
        done.1.notify_all();
    });
    let signal_ctx = ctx.clone();
    let signal_stop = stop.clone();
    thread::spawn(move || {
        loop {
            if signal_ctx.wait_timeout(Duration::from_millis(100)) {
                return;
            }
            if SIGNAL_CANCELLED.load(Ordering::SeqCst) {
                signal_stop();
                return;
            }
        }
    });
    (ctx, stop)
}

fn install_signal_handlers() {
    SIGNAL_INIT.call_once(|| {
        #[cfg(unix)]
        {
            let _ = signal_hook::flag::register(
                signal_hook::consts::SIGINT,
                Arc::clone(&SIGNAL_CANCELLED),
            );
            let _ = signal_hook::flag::register(
                signal_hook::consts::SIGTERM,
                Arc::clone(&SIGNAL_CANCELLED),
            );
        }
        #[cfg(windows)]
        {
            let _ = signal_hook::flag::register(
                signal_hook::consts::SIGINT,
                Arc::clone(&SIGNAL_CANCELLED),
            );
        }
    });
}

// new_parent_process_watchdog returns a SetParentProcessID callback if the
// platform supports process-alive checking, or None otherwise.
pub fn new_parent_process_watchdog(
    ctx: WatchdogContext,
    stop: CancelFunc,
) -> Option<impl Fn(i32) + Send + Sync + 'static> {
    if !PROCESS_ALIVE_SUPPORTED {
        return None;
    }
    Some(move |parent_pid| {
        start_parent_process_watchdog(ctx.clone(), stop.clone(), parent_pid);
    })
}

// start_parent_process_watchdog starts a background thread that monitors the
// parent process and cancels the context if the parent dies. This prevents
// orphaned language server processes when the editor crashes or is killed.
pub fn start_parent_process_watchdog(ctx: WatchdogContext, stop: CancelFunc, parent_pid: i32) {
    if parent_pid <= 0 {
        return;
    }

    thread::spawn(move || {
        loop {
            if ctx.wait_timeout(Duration::from_secs(5)) {
                return;
            }
            if !is_process_alive(parent_pid) {
                eprintln!("Parent process {parent_pid} has exited, shutting down.");
                stop();
                return;
            }
        }
    });
}

fn register_parent_process_watchdog<F>(watchdog: F) -> fn(i32)
where
    F: Fn(i32) + Send + Sync + 'static,
{
    *PARENT_PROCESS_WATCHDOG
        .lock()
        .unwrap_or_else(|err| err.into_inner()) = Some(Box::new(watchdog));
    fn call(parent_pid: i32) {
        if let Some(watchdog) = &*PARENT_PROCESS_WATCHDOG
            .lock()
            .unwrap_or_else(|err| err.into_inner())
        {
            watchdog(parent_pid);
        }
    }
    call
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_string()).collect()
    }

    #[test]
    fn parse_lsp_flags_accepts_go_flag_forms() {
        let mut flags = LspFlags::default();

        assert!(flags.parse(&args(&[
            "--stdio",
            "-pprofDir",
            "/tmp/pprof",
            "--pipe=tsgo-lsp.pipe",
            "-socket",
            "127.0.0.1:0",
        ])));

        assert!(flags.stdio);
        assert_eq!(flags.pprof_dir, "/tmp/pprof");
        assert_eq!(flags._pipe, "tsgo-lsp.pipe");
        assert_eq!(flags._socket, "127.0.0.1:0");
    }

    #[test]
    fn parse_lsp_flags_accepts_explicit_bool_values() {
        let mut flags = LspFlags::default();

        assert!(flags.parse(&args(&["--stdio=false"])));

        assert!(!flags.stdio);
    }

    #[test]
    fn parse_lsp_flags_rejects_unknown_and_missing_values() {
        let mut flags = LspFlags::default();
        assert!(!flags.parse(&args(&["--bogus"])));

        let mut flags = LspFlags::default();
        assert!(!flags.parse(&args(&["--pprofDir"])));

        let mut flags = LspFlags::default();
        assert!(!flags.parse(&args(&["--stdio=maybe"])));
    }

    #[test]
    fn parse_lsp_flags_stops_at_double_dash_or_first_operand() {
        let mut flags = LspFlags::default();
        assert!(flags.parse(&args(&["--stdio", "--", "--pprofDir", "ignored"])));
        assert!(flags.stdio);
        assert_eq!(flags.pprof_dir, "");

        let mut flags = LspFlags::default();
        assert!(flags.parse(&args(&["input.ts", "--stdio"])));
        assert!(!flags.stdio);
    }

    #[test]
    fn run_lsp_returns_two_for_flag_parse_errors() {
        assert_eq!(run_lsp(&args(&["--unknown"])), 2);
        assert_eq!(run_lsp(&args(&["--pprofDir"])), 2);
    }

    #[test]
    fn run_lsp_requires_stdio_like_go_command() {
        assert_eq!(run_lsp(&args(&[])), 1);
        assert_eq!(run_lsp(&args(&["--stdio=false"])), 1);
    }
}
