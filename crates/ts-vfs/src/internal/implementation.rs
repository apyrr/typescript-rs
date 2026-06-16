use std::{fmt, io, sync::Arc};

use crate::vfs::{DirEntry, Entries, FileInfo, Fs};

pub type RootFor = Arc<dyn Fn(&str) -> Option<Arc<dyn Fs + Send + Sync>> + Send + Sync>;
pub type IsReparsePoint = Arc<dyn Fn(&str) -> bool + Send + Sync>;

#[derive(Clone, Default)]
pub struct Common {
    pub root_for: Option<RootFor>,
    pub is_reparse_point: Option<IsReparsePoint>,
}

impl fmt::Debug for Common {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Common")
            .field("root_for", &self.root_for.as_ref().map(|_| "<callback>"))
            .field(
                "is_reparse_point",
                &self.is_reparse_point.as_ref().map(|_| "<callback>"),
            )
            .finish()
    }
}

pub fn root_length(path: &str) -> usize {
    let root = ts_tspath::path::get_encoded_root_length(path);
    if root == 0 {
        panic!("vfs: path {path:?} is not absolute");
    }
    if root < 0 {
        (!root) as usize
    } else {
        root as usize
    }
}

pub fn split_path(path: &str) -> (String, String) {
    let path = ts_tspath::path::normalize_path(path);
    let length = root_length(&path);
    let root = path[..length].to_owned();
    let rest = ts_tspath::path::remove_trailing_directory_separator(&path[length..]).to_owned();
    (root, rest)
}

impl Common {
    pub fn root_and_path(&self, path: &str) -> (Option<Arc<dyn Fs + Send + Sync>>, String, String) {
        let (root, mut rest) = split_path(path);
        if rest.is_empty() {
            rest = ".".to_owned();
        }
        let fsys = self.root_for.as_ref().and_then(|root_for| root_for(&root));
        (fsys, root, rest)
    }

    pub fn stat(&self, path: &str) -> io::Result<FileInfo> {
        let (Some(fsys), _, rest) = self.root_and_path(path) else {
            return Err(io::ErrorKind::NotFound.into());
        };
        fsys.stat(&rest)
    }

    pub fn file_exists(&self, path: &str) -> bool {
        self.stat(path).is_ok_and(|info| info.is_file())
    }

    pub fn directory_exists(&self, path: &str) -> bool {
        self.stat(path).is_ok_and(|info| info.is_dir())
    }

    pub fn get_accessible_entries(&self, path: &str) -> Entries {
        let mut result = Entries {
            symlinks: Some(Default::default()),
            ..Entries::default()
        };

        for entry in self.get_entries(path) {
            let Ok(entry_type) = entry.file_type() else {
                continue;
            };

            if add_to_result(&mut result, entry.name(), &entry_type, false) {
                continue;
            }

            if entry_type.is_symlink() {
                if let Ok(stat) = self.stat(&join_path(path, entry.name())) {
                    add_to_result(&mut result, entry.name(), stat.file_type(), true);
                }
                continue;
            }

            if self
                .is_reparse_point
                .as_ref()
                .is_some_and(|is_reparse_point| is_reparse_point(&join_path(path, entry.name())))
                && let Ok(stat) = self.stat(&join_path(path, entry.name()))
            {
                add_to_result(&mut result, entry.name(), stat.file_type(), true);
            }
        }

        result
    }

    fn get_entries(&self, path: &str) -> Vec<DirEntry> {
        let (Some(fsys), _, rest) = self.root_and_path(path) else {
            return Vec::new();
        };

        let entries = fsys.get_accessible_entries(&rest);
        entries
            .directories
            .into_iter()
            .map(DirEntry::directory)
            .chain(entries.files.into_iter().map(DirEntry::file))
            .collect()
    }

    pub fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        let (Some(fsys), root_name, rest) = self.root_and_path(root) else {
            return Ok(());
        };

        fsys.walk_dir(&rest, &mut |path, entry, err| {
            let path = if path == "." { "" } else { path };
            walk_fn(&format!("{root_name}{path}"), entry, err)
        })
    }

    pub fn read_file(&self, path: &str) -> (String, bool) {
        let (Some(fsys), _, rest) = self.root_and_path(path) else {
            return (String::new(), false);
        };

        let (contents, ok) = fsys.read_file(&rest);
        if !ok {
            return (String::new(), false);
        }
        if contents.is_empty() {
            return (String::new(), true);
        }
        decode_bytes(contents.as_bytes())
            .map(|contents| (contents, true))
            .unwrap_or_else(|| (String::new(), false))
    }
}

fn add_to_result(
    result: &mut Entries,
    name: &str,
    mode: &crate::vfs::FileType,
    is_link: bool,
) -> bool {
    if mode.is_dir() {
        result.directories.push(name.to_owned());
    } else if mode.is_file() {
        result.files.push(name.to_owned());
    } else {
        return false;
    }

    if is_link {
        result
            .symlinks
            .get_or_insert_with(Default::default)
            .insert(name.to_owned());
    }
    true
}

fn join_path(left: &str, right: &str) -> String {
    if left.ends_with('/') {
        format!("{left}{right}")
    } else {
        format!("{left}/{right}")
    }
}

pub fn decode_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec()).ok();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false);
    }
    String::from_utf8(bytes.to_vec()).ok()
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Option<String> {
    let words = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&words).ok()
}
