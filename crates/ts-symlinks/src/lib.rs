#![forbid(unsafe_code)]
mod knownsymlinks;

#[cfg(test)]
mod knownsymlinks_bench_test;
#[cfg(test)]
mod knownsymlinks_test;

pub use knownsymlinks::{KnownDirectoryLink, KnownSymlinks, new_known_symlink};
