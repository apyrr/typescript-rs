use std::{fs, path::Path};

use crate::*;
use ts_tspath as tspath;
use ts_vfs::{Fs, osvfs};

#[test]
fn testing_lib_path_exists() {
    let p = testing_lib_path();

    fs::metadata(&p).unwrap();

    let lib_dts = Path::new(&p).join("lib.d.ts");

    fs::metadata(lib_dts).unwrap();
}

#[test]
fn embedded_libs() {
    let fs = wrap_fs(osvfs::os::fs());
    let mut files = Vec::new();

    fs.walk_dir(&lib_path(), &mut |path, entry, err| {
        if let Some(err) = err {
            return Err(err);
        }
        if !entry.file_type()?.is_dir() {
            files.push(tspath::get_base_file_name(path));
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(files, LIB_NAMES);
}
