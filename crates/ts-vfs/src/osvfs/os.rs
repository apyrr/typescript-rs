use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::sync::OnceLock;
use std::time::SystemTime;

use filetime::FileTime;
use ts_core::{LimitedSemaphore, Semaphore, new_limited_semaphore};
use ts_tspath::path;

use crate::internal::{decode_bytes, root_length};
use crate::vfs::{DirEntry, Entries, FileInfo, FileType, Fs};

#[derive(Clone, Debug, Default)]
pub struct OsFs {
    use_case_sensitive_file_names: bool,
}

pub fn fs() -> OsFs {
    OsFs {
        use_case_sensitive_file_names: is_file_system_case_sensitive(),
    }
}

pub fn is_file_system_case_sensitive() -> bool {
    static CASE_SENSITIVE: OnceLock<bool> = OnceLock::new();
    *CASE_SENSITIVE.get_or_init(detect_case_sensitivity)
}

fn detect_case_sensitivity() -> bool {
    if cfg!(windows) {
        return false;
    }

    if cfg!(target_arch = "wasm32") {
        return true;
    }

    let exe = std::env::current_exe()
        .unwrap_or_else(|err| panic!("vfs: failed to get executable path: {err}"));
    let swapped = swap_case(&exe.to_string_lossy());
    match fs::metadata(&swapped) {
        Ok(_) => false,
        Err(err) if err.kind() == io::ErrorKind::NotFound => true,
        Err(err) => panic!("vfs: failed to stat {swapped:?}: {err}"),
    }
}

pub fn swap_case(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            let upper = ch.to_uppercase().next().unwrap_or(ch);
            if upper == ch {
                ch.to_lowercase().next().unwrap_or(ch)
            } else {
                upper
            }
        })
        .collect()
}

struct ReleaseOnDrop<'a>(Option<Box<dyn FnOnce() + Send + 'a>>);

impl Drop for ReleaseOnDrop<'_> {
    fn drop(&mut self) {
        if let Some(release) = self.0.take() {
            release();
        }
    }
}

fn blocking_op_sema() -> &'static LimitedSemaphore {
    static BLOCKING_OP_SEMA: OnceLock<LimitedSemaphore> = OnceLock::new();
    BLOCKING_OP_SEMA.get_or_init(|| new_limited_semaphore(128))
}

fn read_sema() -> &'static LimitedSemaphore {
    static READ_SEMA: OnceLock<LimitedSemaphore> = OnceLock::new();
    READ_SEMA.get_or_init(|| new_limited_semaphore(128))
}

fn write_sema() -> &'static LimitedSemaphore {
    static WRITE_SEMA: OnceLock<LimitedSemaphore> = OnceLock::new();
    WRITE_SEMA.get_or_init(|| new_limited_semaphore(32))
}

fn write_file_with_options(path: &str, data: &str, append: bool) -> io::Result<()> {
    root_length(path);
    let _permit = ReleaseOnDrop(Some(write_sema().acquire()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    let mut file = options.open(path)?;
    file.write_all(data.as_bytes())
}

fn ensure_directory_exists(directory_path: &str) -> io::Result<()> {
    let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
    fs::create_dir_all(directory_path)
}

fn write_file_ensuring_dir(path: &str, data: &str, append: bool) -> io::Result<()> {
    if let Err(err) = write_file_with_options(path, data, append) {
        let directory_path = path::get_directory_path(&path::normalize_path(path));
        ensure_directory_exists(&directory_path)?;
        write_file_with_options(path, data, append).map_err(|retry_err| {
            if retry_err.kind() == io::ErrorKind::NotFound {
                err
            } else {
                retry_err
            }
        })
    } else {
        Ok(())
    }
}

impl Fs for OsFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.use_case_sensitive_file_names
    }

    fn file_exists(&self, path: &str) -> bool {
        root_length(path);
        let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
        fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        root_length(path);
        let _permit = ReleaseOnDrop(Some(read_sema().acquire()));
        fs::read(path)
            .ok()
            .and_then(|contents| decode_bytes(&contents))
            .map(|contents| (contents, true))
            .unwrap_or_else(|| (String::new(), false))
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        write_file_ensuring_dir(path, data, false)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        write_file_ensuring_dir(path, data, true)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
        let Ok(metadata) = fs::metadata(path) else {
            return Ok(());
        };
        if metadata.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
        filetime::set_file_times(
            path,
            FileTime::from_system_time(atime),
            FileTime::from_system_time(mtime),
        )
    }

    fn directory_exists(&self, path: &str) -> bool {
        root_length(path);
        let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
        fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        root_length(path);
        let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
        let mut entries = Entries {
            symlinks: Some(HashSet::new()),
            ..Default::default()
        };
        if let Ok(read_dir) = fs::read_dir(path) {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let entry_file_type = entry.file_type();
                if entry_file_type
                    .as_ref()
                    .is_ok_and(|file_type| file_type.is_symlink())
                {
                    entries.symlinks.as_mut().unwrap().insert(name.clone());
                }
                if fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_dir()) {
                    entries.directories.push(name);
                } else if fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_file()) {
                    entries.files.push(name);
                }
            }
        }
        entries.directories.sort();
        entries.files.sort();
        entries
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        root_length(path);
        let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
        fs::metadata(path).map(|metadata| FileInfo::from_metadata(path, metadata))
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        root_length(root);
        let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
        fn walk(path: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
            for entry_result in fs::read_dir(path)? {
                match entry_result {
                    Ok(entry) => {
                        let child_path = entry.path().to_string_lossy().into_owned();
                        let std_file_type = entry.file_type()?;
                        let file_type = FileType {
                            is_dir: std_file_type.is_dir(),
                            is_file: std_file_type.is_file(),
                            is_symlink: std_file_type.is_symlink(),
                        };
                        let is_dir = file_type.is_dir();
                        walk_fn(
                            &child_path,
                            DirEntry::new(entry.file_name().to_string_lossy(), file_type),
                            None,
                        )?;
                        if is_dir {
                            walk(&child_path, walk_fn)?;
                        }
                    }
                    Err(err) => return Err(err),
                }
            }
            Ok(())
        }
        walk(root, walk_fn)?;
        Ok(())
    }

    fn realpath(&self, path: &str) -> String {
        let _permit = ReleaseOnDrop(Some(blocking_op_sema().acquire()));
        os_fs_realpath(path).unwrap_or_else(|| path.to_owned())
    }
}

pub fn os_fs_realpath(path: &str) -> Option<String> {
    root_length(path);
    let realpath = platform_realpath(path).ok()?;
    Some(path::normalize_slashes(&realpath))
}

fn platform_realpath(path: &str) -> io::Result<String> {
    #[cfg(target_os = "linux")]
    {
        super::realpath_linux::realpath(path)
    }
    #[cfg(target_os = "macos")]
    {
        super::realpath_darwin::realpath(path)
    }
    #[cfg(windows)]
    {
        super::realpath_windows::realpath(path)
    }
    #[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
    {
        super::realpath_other::realpath(path)
    }
}

pub fn get_global_typings_cache_location() -> String {
    let cache_dir = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    let cache_dir = cache_dir.to_string_lossy();
    let subdir = if cfg!(windows) {
        "Microsoft/TypeScript"
    } else {
        "typescript"
    };
    path::combine_paths(&cache_dir, &[subdir, &ts_core::version_major_minor()])
}
