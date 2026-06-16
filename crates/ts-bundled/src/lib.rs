#![forbid(unsafe_code)]
mod bundled;
#[cfg(test)]
mod bundled_test;
#[cfg(not(feature = "noembed"))]
mod embed;
#[cfg(not(feature = "noembed"))]
mod embed_generated;
mod libs_generated;
#[cfg(feature = "noembed")]
mod noembed;

pub use bundled::{
    EMBEDDED, bundled_source_dir, embedded, is_bundled, lib_path, testing_lib_path, wrap_fs,
};
pub use libs_generated::LIB_NAMES;
