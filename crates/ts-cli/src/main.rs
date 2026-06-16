#![forbid(unsafe_code)]
use std::{env, process};

use ts_core as core;
use ts_execute as execute;

mod api;
#[cfg(windows)]
mod enablevtprocessing_windows;
#[cfg(not(any(unix, windows)))]
mod isprocessalive_other;
#[cfg(unix)]
mod isprocessalive_unix;
#[cfg(windows)]
mod isprocessalive_windows;
mod lsp;
mod sys;

use api::run_api;
use lsp::run_lsp;
use sys::new_system;

#[cfg(not(any(unix, windows)))]
pub use isprocessalive_other::{PROCESS_ALIVE_SUPPORTED, is_process_alive};
#[cfg(unix)]
pub use isprocessalive_unix::{PROCESS_ALIVE_SUPPORTED, is_process_alive};
#[cfg(windows)]
pub use isprocessalive_windows::{PROCESS_ALIVE_SUPPORTED, is_process_alive};

#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    #[cfg(windows)]
    enablevtprocessing_windows::enable_vt_processing();

    process::exit(run_main());
}

pub fn run_main() -> i32 {
    core::apply_debug_stack_limit();
    let args = env::args().skip(1).collect::<Vec<_>>();
    if let Some(command) = args.first() {
        match command.as_str() {
            "--lsp" => return run_lsp(&args[1..]),
            "--api" => return run_api(&args[1..]),
            _ => {}
        }
    }
    let result = execute::command_line(Box::new(new_system()), args, None);
    result.status as i32
}
