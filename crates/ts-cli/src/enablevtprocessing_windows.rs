#![cfg(windows)]

pub fn enable_vt_processing() {
    let _ = enable_ansi_support::enable_ansi_support();
}
