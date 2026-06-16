//go:build !noembed
#![allow(dead_code)]

use std::{io, time::SystemTime};

use ts_vfs as vfs;

use crate::{
    embed_generated::{EMBEDDED_CONTENTS, LIBS_ENTRIES},
    libs_generated::LIB_NAMES,
};

pub const EMBEDDED: bool = true;

const SCHEME: &str = "bundled:///";

fn embedded_contents_get(path: &str) -> Option<&'static str> {
    EMBEDDED_CONTENTS
        .iter()
        .find_map(|(name, contents)| (*name == path).then_some(*contents))
}

fn split_path(path: &str) -> Option<&str> {
    path.strip_prefix(SCHEME)
}

pub fn lib_path() -> String {
    format!("{SCHEME}libs")
}

pub fn is_bundled(path: &str) -> bool {
    split_path(path).is_some()
}

// wrappedFS is implemented directly rather than going through [io/fs.FS].
// Our vfs.FS works with file contents in terms of strings, and that's
// what go:embed does under the hood, but going through fs.FS will cause
// copying to []byte and back.

pub struct WrappedFS<FS> {
    fs: FS,
}

pub fn wrap_fs<FS>(fs: FS) -> WrappedFS<FS> {
    WrappedFS { fs }
}

impl<FS: vfs::Fs> vfs::Fs for WrappedFS<FS> {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        if let Some(rest) = split_path(path) {
            return embedded_contents_get(rest).is_some();
        }
        self.fs.file_exists(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        if let Some(rest) = split_path(path) {
            return match embedded_contents_get(rest) {
                Some(contents) => (contents.to_owned(), true),
                None => (String::new(), false),
            };
        }
        self.fs.read_file(path)
    }

    fn directory_exists(&self, path: &str) -> bool {
        if let Some(rest) = split_path(path) {
            return rest == "libs";
        }
        self.fs.directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        if let Some(rest) = split_path(path) {
            let mut result = vfs::Entries::default();
            if rest.is_empty() {
                result.directories = vec!["libs".to_owned()];
            } else if rest == "libs" {
                result.files = LIB_NAMES.iter().map(|name| (*name).to_owned()).collect();
            }
            return result;
        }
        self.fs.get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        if let Some(rest) = split_path(path) {
            if rest.is_empty() || rest == "libs" {
                return Ok(vfs::FileInfo::directory(rest.to_owned(), None));
            }
            if let Some(lib) = embedded_contents_get(rest) {
                let lib_name = rest.strip_prefix("libs/").unwrap_or(rest);
                return Ok(vfs::FileInfo::file(
                    lib_name.to_owned(),
                    lib.len() as u64,
                    None,
                ));
            }
            return Err(io::ErrorKind::NotFound.into());
        }
        self.fs.stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        if let Some(rest) = split_path(root) {
            self.walk_dir_embedded(rest, walk_fn)
        } else {
            self.fs.walk_dir(root, walk_fn)
        }
    }

    fn realpath(&self, path: &str) -> String {
        if split_path(path).is_some() {
            return path.to_owned();
        }
        self.fs.realpath(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        if split_path(path).is_some() {
            panic!("cannot write to embedded file system");
        }
        self.fs.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        if split_path(path).is_some() {
            panic!("cannot write to embedded file system");
        }
        self.fs.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        if split_path(path).is_some() {
            panic!("cannot remove from embedded file system");
        }
        self.fs.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        if split_path(path).is_some() {
            panic!("cannot change times on embedded file system");
        }
        self.fs.chtimes(path, atime, mtime)
    }
}

impl<FS: vfs::Fs> WrappedFS<FS> {
    fn walk_dir_embedded(&self, rest: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        let entries: Vec<vfs::DirEntry> = match rest {
            "" => vec![vfs::DirEntry::directory("libs".to_owned())],
            "libs" => LIBS_ENTRIES
                .iter()
                .map(|(name, _size)| vfs::DirEntry::file((*name).to_owned()))
                .collect(),
            _ => return Ok(()),
        };

        for entry in entries {
            let name = format!("{rest}/{}", entry.name());

            walk_fn(&format!("{SCHEME}{name}"), entry.clone(), None)?;
            if entry.file_type()?.is_dir() {
                self.walk_dir_embedded(name.trim_start_matches('/'), walk_fn)?;
            }
        }

        Ok(())
    }
}

pub struct FileInfo {
    pub is_dir: bool,
    pub name: String,
    pub size: i64,
}

impl FileInfo {
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub fn mod_time(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    pub fn mode(&self) -> vfs::FileType {
        if self.is_dir {
            vfs::FileType::directory()
        } else {
            vfs::FileType::file()
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> i64 {
        self.size
    }

    pub fn sys(&self) {}

    pub fn info(&self) -> (&Self, Option<io::Error>) {
        (self, None)
    }

    pub fn file_type(&self) -> vfs::FileType {
        self.mode()
    }
}
