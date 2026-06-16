use std::{
    env, io,
    sync::{
        Arc, LazyLock, Once,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use ts_api as api;
use ts_bundled as bundled;
use ts_core as core;

static API_SIGNAL_CANCELLED: LazyLock<Arc<AtomicBool>> =
    LazyLock::new(|| Arc::new(AtomicBool::new(false)));
static API_SIGNAL_INIT: Once = Once::new();

struct ApiFlags {
    cwd: String,
    pipe_path: String,
    callbacks: String,
    r#async: bool,
}

fn parse_api_flags(args: &[String]) -> Result<ApiFlags, ()> {
    let mut flags = ApiFlags {
        // PORT NOTE: lossy conversion matches Go's string-shaped working-directory
        // API for now; revisit when tspath/path byte handling is ported.
        cwd: core::must(env::current_dir())
            .to_string_lossy()
            .into_owned(),
        pipe_path: String::new(),
        callbacks: String::new(),
        r#async: false,
    };

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
        } else {
            arg.strip_prefix('-').unwrap()
        };
        let (name, value) = if let Some((name, value)) = trimmed.split_once('=') {
            (name, Some(value.to_owned()))
        } else {
            (trimmed, None)
        };

        match name {
            "cwd" => {
                flags.cwd = take_flag_value(args, &mut i, value)?;
            }
            "pipe" => {
                flags.pipe_path = take_flag_value(args, &mut i, value)?;
            }
            "callbacks" => {
                flags.callbacks = take_flag_value(args, &mut i, value)?;
            }
            "async" => {
                flags.r#async = match value {
                    Some(value) => parse_bool_flag(&value)?,
                    None => true,
                };
            }
            "h" | "help" => return Err(()),
            _ => return Err(()),
        }
        i += 1;
    }

    Ok(flags)
}

fn take_flag_value(args: &[String], i: &mut usize, value: Option<String>) -> Result<String, ()> {
    if let Some(value) = value {
        return Ok(value);
    }
    *i += 1;
    args.get(*i).cloned().ok_or(())
}

fn parse_bool_flag(value: &str) -> Result<bool, ()> {
    match value {
        "1" | "t" | "T" | "true" | "TRUE" | "True" => Ok(true),
        "0" | "f" | "F" | "false" | "FALSE" | "False" => Ok(false),
        _ => Err(()),
    }
}

pub fn run_api(args: &[String]) -> i32 {
    let flags = match parse_api_flags(args) {
        Ok(flags) => flags,
        Err(()) => {
            eprintln!("flag provided but not defined or missing value");
            return 2;
        }
    };

    let default_library_path = bundled::lib_path();

    let mut callbacks_list = Vec::new();
    if !flags.callbacks.is_empty() {
        callbacks_list = flags.callbacks.split(',').map(str::to_owned).collect();
    }

    let mut options = api::StdioServerOptions {
        err: Some(Box::new(io::stderr())),
        cwd: flags.cwd,
        default_library_path,
        callbacks: callbacks_list,
        r#async: flags.r#async,
        ..Default::default()
    };
    if !flags.pipe_path.is_empty() {
        options.pipe_path = flags.pipe_path;
    } else {
        options.r#in = Some(Box::new(io::stdin()));
        options.out = Some(Box::new(io::stdout()));
    }

    let mut server = api::new_stdio_server(options);

    let (ctx, stop) = notify_context(core::Context::background());
    if let Err(err) = server.run(ctx) {
        eprintln!("{err}");
        stop.cancel();
        return 1;
    }
    stop.cancel();
    0
}

fn notify_context(ctx: core::Context) -> (core::Context, core::CancelFunc) {
    install_signal_handlers();
    let (ctx, cancel) = core::with_cancel(ctx);
    let signal_ctx = ctx.clone();
    let signal_cancel = cancel.clone();
    thread::spawn(move || {
        loop {
            if signal_ctx.err().is_some() {
                return;
            }
            if API_SIGNAL_CANCELLED.load(Ordering::SeqCst) {
                signal_cancel.cancel();
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
    (ctx, cancel)
}

fn install_signal_handlers() {
    API_SIGNAL_INIT.call_once(|| {
        #[cfg(unix)]
        {
            let _ = signal_hook::flag::register(
                signal_hook::consts::SIGINT,
                Arc::clone(&API_SIGNAL_CANCELLED),
            );
            let _ = signal_hook::flag::register(
                signal_hook::consts::SIGTERM,
                Arc::clone(&API_SIGNAL_CANCELLED),
            );
        }
        #[cfg(windows)]
        {
            let _ = signal_hook::flag::register(
                signal_hook::consts::SIGINT,
                Arc::clone(&API_SIGNAL_CANCELLED),
            );
        }
    });
}
