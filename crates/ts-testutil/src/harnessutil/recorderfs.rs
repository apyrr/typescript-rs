use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::TestFile;
use ts_vfs as vfs;

#[derive(Clone)]
pub struct OutputRecorderFs<F> {
    pub fs: F,
    outputs: Arc<Mutex<RecordedOutputs>>,
}

#[derive(Clone, Default)]
struct RecordedOutputs {
    outputs_map: HashMap<String, usize>,
    outputs: Vec<TestFile>,
}

pub fn new_output_recorder_fs<F>(fs: F) -> OutputRecorderFs<F> {
    OutputRecorderFs {
        fs,
        outputs: Arc::new(Mutex::new(RecordedOutputs::default())),
    }
}

impl<F: vfs::Fs> OutputRecorderFs<F> {
    pub fn outputs(&self) -> Vec<TestFile> {
        self.outputs.lock().unwrap().outputs.clone()
    }

    fn record_output(&self, path: String, data: &str) {
        let mut outputs = self.outputs.lock().unwrap();
        if let Some(index) = outputs.outputs_map.get(&path).copied() {
            outputs.outputs[index] = TestFile {
                unit_name: path,
                content: data.to_string(),
            };
        } else {
            let index = outputs.outputs.len();
            outputs.outputs_map.insert(path.clone(), index);
            outputs.outputs.push(TestFile {
                unit_name: path,
                content: data.to_string(),
            });
        }
    }
}

impl<F: vfs::Fs> vfs::Fs for OutputRecorderFs<F> {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.fs.file_exists(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.fs.read_file(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.write_file(path, data)?;
        let path = self.fs.realpath(path);
        self.record_output(path, data);
        Ok(())
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
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
