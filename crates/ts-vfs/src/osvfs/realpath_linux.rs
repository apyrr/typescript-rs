use std::{fs, io, os::fd::AsRawFd, sync::OnceLock};

use rustix::{
    fs::{Mode, OFlags, open, readlink},
    io::retry_on_intr,
};

// On Linux, we use the O_PATH + /proc/self/fd trick to resolve the canonical
// path in O(1) syscalls (open + readlink + close) instead of Go's
// filepath.EvalSymlinks which does an lstat per path component — O(depth).
//
// This is the approach libuv/Node.js could use, though libuv currently just
// calls C realpath(3) which itself does a readlink per component. On the Go
// side, the per-component approach is even more expensive because each
// os.Lstat call involves goroutine scheduling overhead (entersyscall /
// exitsyscall).
//
// How it works:
//   - open(path, O_PATH|O_CLOEXEC) gives us a lightweight fd that follows all
//     symlinks to the final target. O_PATH requires only search permission on
//     directories (same as lstat), and works for both files and directories.
//   - readlink("/proc/self/fd/<fd>") returns the fully resolved canonical path
//     that the kernel computed during the open.
//
// Falls back to filepath.EvalSymlinks if /proc is not available (e.g. containers
// or chroots without procfs mounted).

const PROC_SELF_FD: &str = "/proc/self/fd/";
fn has_proc_self_fd() -> bool {
    static HAS_PROC_SELF_FD: OnceLock<bool> = OnceLock::new();
    *HAS_PROC_SELF_FD.get_or_init(|| fs::metadata(PROC_SELF_FD).is_ok())
}

pub fn realpath(path: &str) -> Result<String, io::Error> {
    if !has_proc_self_fd() {
        let path = fs::canonicalize(path)?;
        return Ok(path.to_string_lossy().into_owned());
    }

    let file = retry_on_intr(|| open(path, OFlags::CLOEXEC | OFlags::PATH, Mode::empty()))
        .map_err(io::Error::from)?;
    let proc_path = format!("{PROC_SELF_FD}{}", file.as_raw_fd());
    let path =
        retry_on_intr(|| readlink(proc_path.as_str(), Vec::new())).map_err(io::Error::from)?;
    Ok(path.to_string_lossy().into_owned())
}
