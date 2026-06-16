use std::{env, sync::OnceLock};

use crate::bundled::testing_lib_path;
use ts_tspath as tspath;
use ts_vfs::{Fs, osvfs};

//go:build noembed

pub const EMBEDDED: bool = false;

pub fn wrap_fs<FS>(fs: FS) -> FS {
    fs
}

static EXECUTABLE_DIR: OnceLock<Result<String, String>> = OnceLock::new();

fn executable_dir() -> String {
    match EXECUTABLE_DIR
        .get_or_init(|| match env::current_exe() {
            Ok(exe) => {
                let exe = tspath::normalize_slashes(&exe.to_string_lossy());
                let exe = osvfs::os::fs().realpath(&exe);
                Ok(tspath::get_directory_path(&exe))
            }
            Err(err) => Err(format!("bundled: failed to get executable path: {err}")),
        })
        .clone()
    {
        Ok(dir) => dir,
        Err(message) => panic!("{message}"),
    }
}

static LIB_PATH: OnceLock<Result<String, String>> = OnceLock::new();

pub fn lib_path() -> String {
    match LIB_PATH
        .get_or_init(|| {
            if cfg!(test) {
                return Ok(testing_lib_path());
            }
            let dir = executable_dir();

            let lib_dts = tspath::combine_paths(&dir, &["lib.d.ts"]);
            if osvfs::os::fs().stat(&lib_dts).is_err() {
                return Err(format!(
                    "bundled: {lib_dts} does not exist; this executable may be misplaced"
                ));
            }

            Ok(dir)
        })
        .clone()
    {
        Ok(dir) => dir,
        Err(message) => panic!("{message}"),
    }
}

pub fn is_bundled(_path: &str) -> bool {
    false
}
