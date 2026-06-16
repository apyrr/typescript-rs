use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::time::SystemTime;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiffEntry {
    pub content: String,
    pub mtime: Option<SystemTime>,
    pub is_written: bool,
    pub symlink_target: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Snapshot {
    pub snap: HashMap<String, DiffEntry>,
    pub default_libs: HashSet<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MapFsEntry {
    pub data: Vec<u8>,
    pub mtime: Option<SystemTime>,
    pub is_regular: bool,
    pub symlink_target: Option<String>,
}

pub trait MapFs {
    fn entries(&self) -> BTreeMap<String, MapFsEntry>;
    fn get_file_info(&self, path: &str) -> Option<MapFsEntry>;
}

pub struct FsDiffer<F> {
    pub fs: F,
    pub default_libs: Option<Box<dyn Fn() -> HashSet<String>>>,
    pub written_files: HashSet<String>,
    pub serialized_diff: Option<Snapshot>,
}

impl<F: MapFs> FsDiffer<F> {
    pub fn serialized_diff(&self) -> Option<&Snapshot> {
        self.serialized_diff.as_ref()
    }

    pub fn baseline_fs_with_diff(&mut self, baseline: &mut impl Write) -> std::io::Result<()> {
        let mut snap = HashMap::new();
        let mut diffs = BTreeMap::new();

        for (path, file) in self.fs.entries() {
            if let Some(target) = file.symlink_target {
                let new_entry = DiffEntry {
                    symlink_target: target,
                    ..DiffEntry::default()
                };
                snap.insert(path.clone(), new_entry.clone());
                self.add_fs_entry_diff(&mut diffs, Some(&new_entry), &path);
            } else if file.is_regular {
                let content = sanitize_internal_symbol_name(&String::from_utf8_lossy(&file.data));
                let new_entry = DiffEntry {
                    content,
                    mtime: file.mtime,
                    is_written: self.written_files.contains(&path),
                    symlink_target: String::new(),
                };
                snap.insert(path.clone(), new_entry.clone());
                self.add_fs_entry_diff(&mut diffs, Some(&new_entry), &path);
            }
        }

        if let Some(serialized_diff) = &self.serialized_diff {
            for path in serialized_diff.snap.keys() {
                if self.fs.get_file_info(path).is_none() {
                    self.add_fs_entry_diff(&mut diffs, None, path);
                }
            }
        }

        let default_libs = self
            .default_libs
            .as_ref()
            .map(|default_libs| default_libs())
            .unwrap_or_default();
        self.serialized_diff = Some(Snapshot { snap, default_libs });

        for (path, diff) in diffs {
            writeln!(baseline, "//// [{path}] {diff}")?;
        }
        writeln!(baseline)?;
        self.written_files.clear();
        Ok(())
    }

    pub fn add_fs_entry_diff(
        &self,
        diffs: &mut BTreeMap<String, String>,
        new_dir_content: Option<&DiffEntry>,
        path: &str,
    ) {
        let old_dir_content = self
            .serialized_diff
            .as_ref()
            .and_then(|snapshot| snapshot.snap.get(path));
        let old_default_libs = self
            .serialized_diff
            .as_ref()
            .map(|snapshot| &snapshot.default_libs);
        let current_default_libs = self
            .default_libs
            .as_ref()
            .map(|default_libs| default_libs())
            .unwrap_or_default();

        match (old_dir_content, new_dir_content) {
            (None, Some(new_content)) => {
                if !current_default_libs.contains(path) {
                    if !new_content.symlink_target.is_empty() {
                        diffs.insert(
                            path.to_string(),
                            format!("-> {} *new*", new_content.symlink_target),
                        );
                    } else {
                        diffs.insert(path.to_string(), format!("*new* \n{}", new_content.content));
                    }
                }
            }
            (Some(_), None) => {
                diffs.insert(path.to_string(), "*deleted*".to_string());
            }
            (Some(old_content), Some(new_content)) => {
                if new_content.content != old_content.content {
                    diffs.insert(
                        path.to_string(),
                        format!("*modified* \n{}", new_content.content),
                    );
                } else if new_content.is_written {
                    diffs.insert(path.to_string(), "*rewrite with same content*".to_string());
                } else if new_content.mtime != old_content.mtime {
                    diffs.insert(path.to_string(), "*mTime changed*".to_string());
                } else if old_default_libs.is_some_and(|libs| libs.contains(path))
                    && !current_default_libs.contains(path)
                {
                    diffs.insert(path.to_string(), format!("*Lib*\n{}", new_content.content));
                }
            }
            (None, None) => {}
        }
    }
}

pub fn sanitize_internal_symbol_name(s: &str) -> String {
    if !s.contains("\u{FFFD}@") {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("\u{FFFD}@") {
        out.push_str(&rest[..start]);
        let symbol_start = start;
        let after_marker = &rest[start + "\u{FFFD}@".len()..];
        let Some(name_end) = after_marker.find('@') else {
            out.push_str(&rest[symbol_start..]);
            return out;
        };
        let after_name = &after_marker[name_end + 1..];
        let digit_len = after_name
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .map(char::len_utf8)
            .sum::<usize>();
        if digit_len == 0 {
            out.push_str(&rest[..start + "\u{FFFD}@".len() + name_end + 1]);
            rest = after_name;
            continue;
        }
        out.push_str(&rest[symbol_start..start + "\u{FFFD}@".len() + name_end + 1]);
        out.push_str("<symbolId>");
        rest = &after_name[digit_len..];
    }
    out.push_str(rest);
    out
}

#[allow(dead_code)]
fn sorted_keys(set: &HashSet<String>) -> BTreeSet<String> {
    set.iter().cloned().collect()
}
