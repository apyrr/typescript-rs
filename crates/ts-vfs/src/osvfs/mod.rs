#[cfg(any(target_os = "linux", target_os = "macos"))]
mod eintr_unix;
#[cfg(test)]
mod helpers_test;
pub mod os;
#[cfg(test)]
mod os_test;
#[cfg(target_os = "macos")]
mod realpath_darwin;
#[cfg(target_os = "linux")]
mod realpath_linux;
#[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
mod realpath_other;
#[cfg(test)]
mod realpath_test;
#[cfg(windows)]
mod realpath_windows;
#[cfg(not(windows))]
mod reparsepoint_other;
#[cfg(windows)]
mod reparsepoint_windows;
#[cfg(test)]
mod reparsepoint_windows_test;
