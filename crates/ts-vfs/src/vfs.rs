use std::{io, sync::Arc, time::SystemTime};

// FS is a file system abstraction.
pub trait Fs {
    // UseCaseSensitiveFileNames returns true if the file system is case-sensitive.
    fn use_case_sensitive_file_names(&self) -> bool;

    // FileExists returns true if the file exists.
    fn file_exists(&self, path: &str) -> bool;

    // ReadFile reads the file specified by path and returns the content.
    // If the file fails to be read, ok will be false.
    fn read_file(&self, path: &str) -> (String, bool);

    fn write_file(&self, path: &str, data: &str) -> io::Result<()>;

    // AppendFile appends data to the file at path, creating it if it does not exist.
    fn append_file(&self, path: &str, data: &str) -> io::Result<()>;

    // Removes `path` and all its contents. Will return the first error it encounters.
    fn remove(&self, path: &str) -> io::Result<()>;

    // Chtimes changes the access and modification times of the named
    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()>;

    // DirectoryExists returns true if the path is a directory.
    fn directory_exists(&self, path: &str) -> bool;

    // GetAccessibleEntries returns the files/directories in the specified directory.
    // If any entry is a symlink, it will be followed.
    fn get_accessible_entries(&self, path: &str) -> Entries;

    fn stat(&self, path: &str) -> io::Result<FileInfo>;

    // WalkDir walks the file tree rooted at root, calling walkFn for each file or directory in the tree.
    // It is has the same behavior as [fs.WalkDir], but with paths as [string].
    fn walk_dir(&self, root: &str, walk_fn: &mut WalkDirFunc<'_>) -> io::Result<()>;

    // Realpath returns the "real path" of the specified path,
    // following symlinks and correcting filename casing.
    fn realpath(&self, path: &str) -> String;
}

impl<T: Fs + ?Sized> Fs for Arc<T> {
    fn use_case_sensitive_file_names(&self) -> bool {
        (**self).use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        (**self).file_exists(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        (**self).read_file(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        (**self).write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        (**self).append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        (**self).remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        (**self).chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        (**self).directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        (**self).get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        (**self).stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut WalkDirFunc<'_>) -> io::Result<()> {
        (**self).walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        (**self).realpath(path)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Entries {
    pub files: Vec<String>,
    pub directories: Vec<String>,
    // Symlinks contains the names of entries in Files or Directories that were
    // originally symbolic links (or reparse points) on disk. The names are the
    // same as those in Files/Directories (i.e., the link name, not the target).
    // nil means symlink information is not available and the entries may need
    // to be re-checked for symlinks.
    pub symlinks: Option<std::collections::HashSet<String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileType {
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
}

impl FileType {
    pub fn file() -> Self {
        Self {
            is_file: true,
            ..Self::default()
        }
    }

    pub fn directory() -> Self {
        Self {
            is_dir: true,
            ..Self::default()
        }
    }

    pub fn symlink() -> Self {
        Self {
            is_symlink: true,
            ..Self::default()
        }
    }

    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub fn is_file(&self) -> bool {
        self.is_file
    }

    pub fn is_symlink(&self) -> bool {
        self.is_symlink
    }
}

// DirEntry is [fs.DirEntry].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirEntry {
    name: String,
    file_type: FileType,
}

impl DirEntry {
    pub fn new(name: impl Into<String>, file_type: FileType) -> Self {
        Self {
            name: name.into(),
            file_type,
        }
    }

    pub fn file(name: impl Into<String>) -> Self {
        Self::new(name, FileType::file())
    }

    pub fn directory(name: impl Into<String>) -> Self {
        Self::new(name, FileType::directory())
    }

    pub fn symlink(name: impl Into<String>) -> Self {
        Self::new(name, FileType::symlink())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.file_type.clone())
    }
}

// FileInfo is [fs.FileInfo].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileInfo {
    name: String,
    size: u64,
    mode: FileType,
    modified_time: Option<SystemTime>,
    realpath: Option<String>,
}

impl FileInfo {
    pub fn new(
        name: impl Into<String>,
        size: u64,
        mode: FileType,
        modified_time: Option<SystemTime>,
    ) -> Self {
        Self {
            name: name.into(),
            size,
            mode,
            modified_time,
            realpath: None,
        }
    }

    pub fn file(name: impl Into<String>, size: u64, modified_time: Option<SystemTime>) -> Self {
        Self::new(name, size, FileType::file(), modified_time)
    }

    pub fn directory(name: impl Into<String>, modified_time: Option<SystemTime>) -> Self {
        Self::new(name, 0, FileType::directory(), modified_time)
    }

    pub fn symlink(name: impl Into<String>, size: u64, modified_time: Option<SystemTime>) -> Self {
        Self::new(name, size, FileType::symlink(), modified_time)
    }

    pub fn with_realpath(mut self, realpath: impl Into<String>) -> Self {
        self.realpath = Some(realpath.into());
        self
    }

    pub fn from_metadata(path: &str, metadata: std::fs::Metadata) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        let file_type = metadata.file_type();
        let mode = FileType {
            is_dir: file_type.is_dir(),
            is_file: file_type.is_file(),
            is_symlink: file_type.is_symlink(),
        };
        Self::new(name, metadata.len(), mode, metadata.modified().ok()).with_realpath(path)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn len(&self) -> u64 {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        self.modified_time
            .ok_or_else(|| io::ErrorKind::Unsupported.into())
    }

    pub fn mod_time(&self) -> Option<SystemTime> {
        self.modified_time
    }

    pub fn is_file(&self) -> bool {
        self.mode.is_file()
    }

    pub fn is_dir(&self) -> bool {
        self.mode.is_dir()
    }

    pub fn file_type(&self) -> &FileType {
        &self.mode
    }

    pub fn realpath(&self) -> Option<&str> {
        self.realpath.as_deref()
    }
}

pub fn err_invalid() -> io::ErrorKind {
    io::ErrorKind::InvalidInput
}

pub fn err_permission() -> io::ErrorKind {
    io::ErrorKind::PermissionDenied
}

pub fn err_exist() -> io::ErrorKind {
    io::ErrorKind::AlreadyExists
}

pub fn err_not_exist() -> io::ErrorKind {
    io::ErrorKind::NotFound
}

pub fn err_closed() -> io::ErrorKind {
    io::ErrorKind::BrokenPipe
}

// WalkDirFunc is [fs.WalkDirFunc].
pub type WalkDirFunc<'a> = dyn FnMut(&str, DirEntry, Option<io::Error>) -> io::Result<()> + 'a;

pub fn skip_all() -> io::ErrorKind {
    io::ErrorKind::Other
}

pub fn skip_dir() -> io::ErrorKind {
    io::ErrorKind::Other
}
