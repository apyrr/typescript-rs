mod fs;
mod readablebuildinfo;
mod runner;
#[cfg(test)]
mod showconfig_test;
mod sys;
#[cfg(test)]
mod testmain_test;
#[cfg(test)]
mod tsc_test;
#[cfg(test)]
mod tscbuild_test;
#[cfg(test)]
mod tscwatch_test;
#[cfg(test)]

#[rustfmt::skip]
pub use sys::{FileMap, TestClock, new_tsc_system, get_file_map_with_build, TestSys};
