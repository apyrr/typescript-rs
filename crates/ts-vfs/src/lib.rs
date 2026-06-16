#![forbid(unsafe_code)]

pub mod cachedvfs;
pub mod internal;
pub mod iovfs;
#[expect(
    dead_code,
    reason = "ported OS VFS helpers are ahead of current callers"
)]
pub mod osvfs;
pub mod trackingvfs;
pub mod vfs;
#[cfg(test)]
mod vfs_test;
pub mod vfsmatch;
pub mod vfsmock;
pub mod vfstest;
pub mod vfswatch;
pub mod wrapvfs;

pub use vfs::{DirEntry, Entries, FileInfo, FileType, Fs, WalkDirFunc};

pub type FS = std::sync::Arc<dyn Fs + Send + Sync>;
