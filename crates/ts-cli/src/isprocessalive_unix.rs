#![cfg(unix)]

pub const PROCESS_ALIVE_SUPPORTED: bool = true;

// is_process_alive checks if a process with the given PID is still running.
// On Unix, probing with signal 0 succeeds when the process exists. EPERM means
// it exists but this process lacks permission to signal it. ESRCH or any other
// error indicates the process is gone.
pub fn is_process_alive(pid: i32) -> bool {
    let Some(pid) = rustix::process::Pid::from_raw(pid) else {
        return false;
    };
    rustix::process::test_kill_process(pid)
        .or_else(|err| (err == rustix::io::Errno::PERM).then_some(()).ok_or(err))
        .is_ok()
}
