#![cfg(windows)]

use sysinfo::{Pid, ProcessesToUpdate, System};

pub const PROCESS_ALIVE_SUPPORTED: bool = true;

// is_process_alive checks if a process with the given PID is still running.
pub fn is_process_alive(pid: i32) -> bool {
    let Ok(pid) = u32::try_from(pid) else {
        return false;
    };
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).is_some()
}
