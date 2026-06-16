#![cfg(not(any(unix, windows)))]

pub const PROCESS_ALIVE_SUPPORTED: bool = false;

pub fn is_process_alive(_pid: i32) -> bool {
    panic!("isProcessAlive is not supported on this platform")
}
