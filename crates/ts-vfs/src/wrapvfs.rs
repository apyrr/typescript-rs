use std::{io, time::SystemTime};

use crate::{Entries, FileInfo, Fs};

pub type UseCaseSensitiveFileNamesReplacement = Box<dyn Fn() -> bool + Send + Sync>;
pub type FileExistsReplacement = Box<dyn Fn(&str) -> bool + Send + Sync>;
pub type ReadFileReplacement = Box<dyn Fn(&str) -> (String, bool) + Send + Sync>;
pub type WriteFileReplacement = Box<dyn Fn(&str, &str) -> io::Result<()> + Send + Sync>;
pub type RemoveReplacement = Box<dyn Fn(&str) -> io::Result<()> + Send + Sync>;
pub type ChtimesReplacement =
    Box<dyn Fn(&str, SystemTime, SystemTime) -> io::Result<()> + Send + Sync>;
pub type GetAccessibleEntriesReplacement = Box<dyn Fn(&str) -> Entries + Send + Sync>;
pub type StatReplacement = Box<dyn Fn(&str) -> io::Result<FileInfo> + Send + Sync>;
pub type WalkDirReplacement =
    Box<dyn for<'a> Fn(&str, &mut crate::WalkDirFunc<'a>) -> io::Result<()> + Send + Sync>;
pub type RealpathReplacement = Box<dyn Fn(&str) -> String + Send + Sync>;

#[derive(Default)]
pub struct Replacements {
    pub use_case_sensitive_file_names: Option<UseCaseSensitiveFileNamesReplacement>,
    pub file_exists: Option<FileExistsReplacement>,
    pub read_file: Option<ReadFileReplacement>,
    pub write_file: Option<WriteFileReplacement>,
    pub append_file: Option<WriteFileReplacement>,
    pub remove: Option<RemoveReplacement>,
    pub chtimes: Option<ChtimesReplacement>,
    pub directory_exists: Option<FileExistsReplacement>,
    pub get_accessible_entries: Option<GetAccessibleEntriesReplacement>,
    pub stat: Option<StatReplacement>,
    pub walk_dir: Option<WalkDirReplacement>,
    pub realpath: Option<RealpathReplacement>,
}

pub fn wrap<F: Fs>(fs: F, replacements: Replacements) -> WrappedFs<F> {
    WrappedFs { fs, replacements }
}

pub struct WrappedFs<F: Fs> {
    fs: F,
    replacements: Replacements,
}

impl<F: Fs> Fs for WrappedFs<F> {
    // UseCaseSensitiveFileNames implements [vfs.FS].
    fn use_case_sensitive_file_names(&self) -> bool {
        if let Some(replacement) = &self.replacements.use_case_sensitive_file_names {
            return replacement();
        }
        self.fs.use_case_sensitive_file_names()
    }

    // FileExists implements [vfs.FS].
    fn file_exists(&self, path: &str) -> bool {
        if let Some(replacement) = &self.replacements.file_exists {
            return replacement(path);
        }
        self.fs.file_exists(path)
    }

    // ReadFile implements [vfs.FS].
    fn read_file(&self, path: &str) -> (String, bool) {
        if let Some(replacement) = &self.replacements.read_file {
            return replacement(path);
        }
        self.fs.read_file(path)
    }

    // WriteFile implements [vfs.FS].
    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        if let Some(replacement) = &self.replacements.write_file {
            return replacement(path, data);
        }
        self.fs.write_file(path, data)
    }

    // AppendFile implements [vfs.FS].
    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        if let Some(replacement) = &self.replacements.append_file {
            return replacement(path, data);
        }
        self.fs.append_file(path, data)
    }

    // Remove implements [vfs.FS].
    fn remove(&self, path: &str) -> io::Result<()> {
        if let Some(replacement) = &self.replacements.remove {
            return replacement(path);
        }
        self.fs.remove(path)
    }

    // Chtimes implements [vfs.FS].
    fn chtimes(&self, path: &str, a_time: SystemTime, m_time: SystemTime) -> io::Result<()> {
        if let Some(replacement) = &self.replacements.chtimes {
            return replacement(path, a_time, m_time);
        }
        self.fs.chtimes(path, a_time, m_time)
    }

    // DirectoryExists implements [vfs.FS].
    fn directory_exists(&self, path: &str) -> bool {
        if let Some(replacement) = &self.replacements.directory_exists {
            return replacement(path);
        }
        self.fs.directory_exists(path)
    }

    // GetAccessibleEntries implements [vfs.FS].
    fn get_accessible_entries(&self, path: &str) -> Entries {
        if let Some(replacement) = &self.replacements.get_accessible_entries {
            return replacement(path);
        }
        self.fs.get_accessible_entries(path)
    }

    // Stat implements [vfs.FS].
    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        if let Some(replacement) = &self.replacements.stat {
            return replacement(path);
        }
        self.fs.stat(path)
    }

    // WalkDir implements [vfs.FS].
    fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        if let Some(replacement) = &self.replacements.walk_dir {
            return replacement(root, walk_fn);
        }
        self.fs.walk_dir(root, walk_fn)
    }

    // Realpath implements [vfs.FS].
    fn realpath(&self, path: &str) -> String {
        if let Some(replacement) = &self.replacements.realpath {
            return replacement(path);
        }
        self.fs.realpath(path)
    }
}
