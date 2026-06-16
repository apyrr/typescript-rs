use std::{collections::HashMap, io, sync::Arc, time::SystemTime};

use serde::Deserialize;
use ts_core as context;
use ts_json as json;
use ts_vfs as vfs;

use crate::Error;

pub trait CallbackClient {
    fn call(
        &self,
        ctx: &context::Context,
        method: &str,
        params: json::Value,
    ) -> Result<json::Value, Error>;
}

// CallbackFs wraps a base filesystem and delegates certain operations to the
// client via RPC callbacks. This allows the API client to provide a virtual
// filesystem, for example in-memory files for testing.
//
// The callbacks to enable are specified at construction time via the
// --callbacks CLI flag. The connection is set via set_connection after the
// transport connection is established.
pub struct CallbackFs {
    base: Arc<dyn vfs::Fs + Send + Sync>,
    enabled_callbacks: HashMap<String, bool>,

    // conn and ctx are set after connection is established.
    conn: Option<Box<dyn CallbackClient + Send + Sync>>,
    ctx: Option<context::Context>,
}

const CALLBACK_READ_FILE: &str = "readFile";
const CALLBACK_FILE_EXISTS: &str = "fileExists";
const CALLBACK_DIRECTORY_EXISTS: &str = "directoryExists";
const CALLBACK_GET_ACCESSIBLE_ENTRIES: &str = "getAccessibleEntries";
const CALLBACK_REALPATH: &str = "realpath";

pub fn is_callback_name(name: &str) -> bool {
    matches!(
        name,
        CALLBACK_READ_FILE
            | CALLBACK_FILE_EXISTS
            | CALLBACK_DIRECTORY_EXISTS
            | CALLBACK_GET_ACCESSIBLE_ENTRIES
            | CALLBACK_REALPATH
    )
}

// new_callback_fs creates a new CallbackFs wrapping the given base filesystem.
// The callbacks slice specifies which filesystem operations should be delegated
// to the client, for example "readFile" or "fileExists".
pub fn new_callback_fs(base: Arc<dyn vfs::Fs + Send + Sync>, callbacks: &[String]) -> CallbackFs {
    let mut enabled = HashMap::with_capacity(callbacks.len());
    for cb in callbacks {
        if !is_callback_name(cb) {
            panic!("unknown callback name: {cb}");
        }
        enabled.insert(cb.clone(), true);
    }
    CallbackFs {
        base,
        enabled_callbacks: enabled,
        conn: None,
        ctx: None,
    }
}

impl CallbackFs {
    // set_connection sets the RPC connection for callbacks. This must be called
    // after the transport connection is established but before any filesystem
    // operations that need callbacks.
    pub fn set_connection(
        &mut self,
        ctx: context::Context,
        conn: Box<dyn CallbackClient + Send + Sync>,
    ) {
        self.ctx = Some(ctx);
        self.conn = Some(conn);
    }

    fn is_enabled(&self, name: &str) -> bool {
        self.enabled_callbacks.get(name).copied().unwrap_or(false)
    }

    fn call(&self, name: &str, arg: &str) -> Result<json::Value, Error> {
        let Some(conn) = self.conn.as_ref() else {
            return Err(Error::new(format!(
                "CallbackFS: {name} called before connection set"
            )));
        };
        let ctx = self
            .ctx
            .as_ref()
            .expect("CallbackFS connection set without context");
        conn.call(ctx, name, json::Value::String(arg.to_owned()))
    }

    fn use_case_sensitive_file_names_impl(&self) -> bool {
        self.base.use_case_sensitive_file_names()
    }

    // The readFile callback uses a wrapped response format to distinguish three
    // states:
    //   - undefined (fall back to real FS): null/missing result
    //   - null (not found, no fallback): {"content": null}
    //   - string content: {"content": "..."}
    fn read_file_impl(&self, path: &str) -> (String, bool) {
        if self.is_enabled(CALLBACK_READ_FILE) {
            let result = self.call(CALLBACK_READ_FILE, path).unwrap_or_else(|err| {
                panic!("{err}");
            });
            if !result.is_null() {
                let wrapper: ReadFileResponse =
                    serde_json::from_value(result).unwrap_or_else(|err| {
                        panic!("{err}");
                    });
                let Some(content) = wrapper.content else {
                    return (String::new(), false);
                };
                return (content, true);
            }
        }
        self.base.read_file(path)
    }

    fn file_exists_impl(&self, path: &str) -> bool {
        if self.is_enabled(CALLBACK_FILE_EXISTS) {
            let result = self.call(CALLBACK_FILE_EXISTS, path).unwrap_or_else(|err| {
                panic!("{err}");
            });
            if !result.is_null() {
                return result.as_bool().unwrap_or(false);
            }
        }
        self.base.file_exists(path)
    }

    fn directory_exists_impl(&self, path: &str) -> bool {
        if self.is_enabled(CALLBACK_DIRECTORY_EXISTS) {
            let result = self
                .call(CALLBACK_DIRECTORY_EXISTS, path)
                .unwrap_or_else(|err| {
                    panic!("{err}");
                });
            if !result.is_null() {
                return result.as_bool().unwrap_or(false);
            }
        }
        self.base.directory_exists(path)
    }

    fn get_accessible_entries_impl(&self, path: &str) -> vfs::Entries {
        if self.is_enabled(CALLBACK_GET_ACCESSIBLE_ENTRIES) {
            let result = self
                .call(CALLBACK_GET_ACCESSIBLE_ENTRIES, path)
                .unwrap_or_else(|err| {
                    panic!("{err}");
                });
            if !result.is_null() {
                let raw_entries: Option<AccessibleEntriesResponse> = serde_json::from_value(result)
                    .unwrap_or_else(|err| {
                        panic!("{err}");
                    });
                if let Some(raw_entries) = raw_entries {
                    return vfs::Entries {
                        files: raw_entries.files,
                        directories: raw_entries.directories,
                        ..Default::default()
                    };
                }
            }
        }
        self.base.get_accessible_entries(path)
    }

    fn realpath_impl(&self, path: &str) -> String {
        if self.is_enabled(CALLBACK_REALPATH) {
            let result = self.call(CALLBACK_REALPATH, path).unwrap_or_else(|err| {
                panic!("{err}");
            });
            if !result.is_null() {
                return serde_json::from_value(result).unwrap_or_else(|err| {
                    panic!("{err}");
                });
            }
        }
        self.base.realpath(path)
    }
}

impl vfs::Fs for CallbackFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.use_case_sensitive_file_names_impl()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.file_exists_impl(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.read_file_impl(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.base.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.base.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.base.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.base.chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.directory_exists_impl(path)
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        self.get_accessible_entries_impl(path)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        self.base.stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        self.base.walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        self.realpath_impl(path)
    }
}

#[derive(Deserialize)]
struct ReadFileResponse {
    content: Option<String>,
}

#[derive(Deserialize)]
struct AccessibleEntriesResponse {
    files: Vec<String>,
    directories: Vec<String>,
}
