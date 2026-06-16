#![forbid(unsafe_code)]

#[expect(
    dead_code,
    reason = "ported command-line entry points are ahead of current callers"
)]
mod command_line;
mod watcher;

#[expect(
    dead_code,
    private_interfaces,
    reason = "ported build-mode support is ahead of current callers"
)]
pub mod build;
#[expect(
    dead_code,
    non_camel_case_types,
    unreachable_patterns,
    unused_assignments,
    reason = "ported incremental support is ahead of current callers"
)]
pub mod incremental;
#[expect(
    dead_code,
    unused_assignments,
    reason = "ported tsc helpers are ahead of current callers"
)]
pub mod tsc;
#[expect(
    dead_code,
    reason = "ported tsc test harness is ahead of current callers"
)]
pub mod tsctests;

pub use command_line::command_line;
pub use watcher::Watcher;
