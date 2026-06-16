use std::{path::PathBuf, sync::OnceLock};

//go:generate go run generate.go

// Define the below here to consolidate documentation.

// Embedded is true if the bundled files are implemented through an embedded FS.
#[cfg(not(feature = "noembed"))]
pub const EMBEDDED: bool = crate::embed::EMBEDDED;

// Embedded is true if the bundled files are implemented through an embedded FS.
#[cfg(feature = "noembed")]
pub const EMBEDDED: bool = crate::noembed::EMBEDDED;

pub fn embedded() -> bool {
    EMBEDDED
}

// WrapFS returns an FS which redirects embedded paths to the embedded file system.
// If the embedded file system is not available, it returns the original FS.
#[cfg(not(feature = "noembed"))]
pub fn wrap_fs<FS: ts_vfs::Fs>(fs: FS) -> crate::embed::WrappedFS<FS> {
    crate::embed::wrap_fs(fs)
}

// WrapFS returns an FS which redirects embedded paths to the embedded file system.
// If the embedded file system is not available, it returns the original FS.
#[cfg(feature = "noembed")]
pub fn wrap_fs<FS>(fs: FS) -> FS {
    crate::noembed::wrap_fs(fs)
}

// LibPath returns the path to the directory containing the bundled lib.d.ts files.
// If embedding is not enabled, this is a path on disk, and must be accessed through
// a real OS filesystem.
#[cfg(not(feature = "noembed"))]
pub fn lib_path() -> String {
    crate::embed::lib_path()
}

// LibPath returns the path to the directory containing the bundled lib.d.ts files.
// If embedding is not enabled, this is a path on disk, and must be accessed through
// a real OS filesystem.
#[cfg(feature = "noembed")]
pub fn lib_path() -> String {
    crate::noembed::lib_path()
}

#[cfg(not(feature = "noembed"))]
pub fn is_bundled(path: &str) -> bool {
    crate::embed::is_bundled(path)
}

#[cfg(feature = "noembed")]
pub fn is_bundled(path: &str) -> bool {
    crate::noembed::is_bundled(path)
}

static BUNDLED_SOURCE_DIR: OnceLock<String> = OnceLock::new();

pub fn bundled_source_dir() -> String {
    BUNDLED_SOURCE_DIR
        .get_or_init(|| {
            let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            dir.pop();
            dir.pop();
            dir.push("vendor/typescript-go/internal/bundled");
            dir.to_string_lossy().into_owned()
        })
        .clone()
}

static TESTING_LIB_PATH: OnceLock<String> = OnceLock::new();

// TestingLibPath returns the path to the source bundled libs directory.
// It's only valid to use in tests where the source code is available.
pub fn testing_lib_path() -> String {
    TESTING_LIB_PATH
        .get_or_init(|| {
            if !cfg!(test) {
                panic!("bundled: TestingLibPath should only be called during tests");
            }
            let mut path = PathBuf::from(bundled_source_dir());
            path.push("libs");
            path.to_string_lossy().replace('\\', "/")
        })
        .clone()
}
