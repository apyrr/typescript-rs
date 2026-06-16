use std::{io, time::SystemTime};

use ts_collections as collections;
use ts_core as core;
use ts_json as json;
use ts_testutil::{fsbaselineutil, harnessutil};
use ts_tspath as tspath;
use ts_vfs::{self as vfs, Fs};

use crate::incremental;
use crate::tsctests::readablebuildinfo::to_readable_build_info;

pub struct TestFs<F: vfs::Fs> {
    pub fs: F,
    pub default_libs: Option<collections::SyncSet<String>>,
    pub written_files: collections::SyncSet<String>,
}

impl<F: vfs::Fs> TestFs<F> {
    pub fn remove_ignore_lib_path(&self, path: &str) {
        if let Some(default_libs) = &self.default_libs {
            let path = path.to_owned();
            if default_libs.has(&path) {
                default_libs.delete(&path);
            }
        }
    }

    pub fn read_file_handling_build_info(&self, path: &str) -> (String, bool) {
        let (mut contents, ok) = self.fs.read_file(path);
        if ok && tspath::file_extension_is(path, tspath::EXTENSION_TS_BUILD_INFO) {
            // read buildinfo and modify version
            let mut build_info = incremental::BuildInfo::default();
            let err = json::unmarshal(contents.as_bytes(), &mut build_info, &[]);
            if err.is_ok() && build_info.version == harnessutil::FAKE_TS_VERSION {
                build_info.version = core::version().to_owned();
                let new_contents = json::marshal(&build_info, &[]);
                match new_contents {
                    Ok(new_contents) => {
                        contents = String::from_utf8(new_contents).unwrap_or_else(|err| {
                            panic!(
                                "testFs.ReadFile: failed to decode build info after fixing version: {err}"
                            )
                        });
                    }
                    Err(err) => {
                        panic!(
                            "testFs.ReadFile: failed to marshal build info after fixing version: {err}"
                        );
                    }
                }
            }
        }
        (contents, ok)
    }

    pub fn write_file_handling_build_info(&self, path: &str, mut data: String) -> io::Result<()> {
        if tspath::file_extension_is(path, tspath::EXTENSION_TS_BUILD_INFO) {
            let mut build_info = incremental::BuildInfo::default();
            if let Err(err) = json::unmarshal(data.as_bytes(), &mut build_info, &[]) {
                panic!(
                    "testFs.WriteFile: failed to unmarshal build info: - use underlying FS's write method if this is intended use for testcase{err}"
                );
            }

            if build_info.version == core::version() {
                // Change it to harnessutil.FakeTSVersion
                build_info.version = harnessutil::FAKE_TS_VERSION.to_owned();
                let new_data = json::marshal(&build_info, &[]).map_err(|err| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "testFs.WriteFile: failed to marshal build info after fixing version: {err}"
                        ),
                    )
                })?;
                data = String::from_utf8(new_data).unwrap_or_else(|err| {
                    panic!(
                        "testFs.WriteFile: failed to decode build info after fixing version: {err}"
                    )
                });
            }

            // Write readable build info version
            self.write_file(
                &format!("{path}.readable.baseline.txt"),
                &to_readable_build_info(
                    &build_info,
                    fsbaselineutil::sanitize_internal_symbol_name(&data),
                ),
            )
            .map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("testFs.WriteFile: failed to write readable build info: {err}"),
                )
            })?;
        }
        self.fs.write_file(path, &data)
    }
}

impl<F: vfs::Fs> vfs::Fs for TestFs<F> {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.fs.file_exists(path)
    }

    // ReadFile reads the file specified by path and returns the content.
    // If the file fails to be read, ok will be false.
    fn read_file(&self, path: &str) -> (String, bool) {
        self.remove_ignore_lib_path(path);
        self.read_file_handling_build_info(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.remove_ignore_lib_path(path);
        self.written_files.add(path.to_owned());
        self.write_file_handling_build_info(path, data.to_owned())
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.append_file(path, data)
    }

    // Removes `path` and all its contents. Will return the first error it encounters.
    fn remove(&self, path: &str) -> io::Result<()> {
        self.remove_ignore_lib_path(path);
        self.fs.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.fs.chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.fs.directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        self.fs.get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        self.fs.stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        self.fs.walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        self.fs.realpath(path)
    }
}
