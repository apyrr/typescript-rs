use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use ts_collections as collections;
use ts_module as module;
use ts_symlinks as symlinks;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs::{DirEntry, Entries, FileInfo, Fs};

use crate::ProgramOptions;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tristate {
    True,
    False,
    Unknown,
}

impl Tristate {
    pub fn is_true(self) -> bool {
        matches!(self, Tristate::True)
    }
}

pub trait CompilerHostLike: Send + Sync {
    fn current_directory(&self) -> String;
    fn use_case_sensitive_file_names(&self) -> bool;
    fn file_exists(&self, path: &str) -> bool;
    fn directory_exists(&self, path: &str) -> bool;
    fn read_file(&self, path: &str) -> Option<String>;
    fn realpath(&self, path: &str) -> String;
}

pub struct ProjectReferenceDtsFakingHost<H> {
    pub host: Arc<H>,
    pub fs: ProjectReferenceDtsFakingVfs<H>,
}

impl<H> Clone for ProjectReferenceDtsFakingHost<H> {
    fn clone(&self) -> Self {
        Self {
            host: Arc::clone(&self.host),
            fs: self.fs.clone(),
        }
    }
}

impl<H: CompilerHostLike + 'static> ProjectReferenceDtsFakingHost<H> {
    pub fn new(
        host: H,
        project_reference_file_mapper: ProjectReferenceFileMapper,
        dts_directories: HashSet<tspath::Path>,
    ) -> Self {
        let host = Arc::new(host);
        Self {
            host: host.clone(),
            fs: ProjectReferenceDtsFakingVfs {
                host,
                project_reference_file_mapper,
                dts_directories,
                known_symlinks: Arc::new(Mutex::new(symlinks::KnownSymlinks::default())),
            },
        }
    }

    pub fn fs(&self) -> &ProjectReferenceDtsFakingVfs<H> {
        &self.fs
    }

    pub fn get_current_directory(&self) -> String {
        self.host.current_directory()
    }

    pub fn current_directory(&self) -> String {
        self.get_current_directory()
    }
}

impl<H: CompilerHostLike + 'static> module::ResolutionHost for ProjectReferenceDtsFakingHost<H> {
    fn get_current_directory(&self) -> String {
        self.host.current_directory()
    }

    fn fs(&self) -> &dyn Fs {
        &self.fs
    }
}

pub fn new_project_reference_dts_faking_host<H: CompilerHostLike + 'static>(
    host: H,
    project_reference_file_mapper: ProjectReferenceFileMapper,
    dts_directories: HashSet<tspath::Path>,
) -> ProjectReferenceDtsFakingHost<H> {
    ProjectReferenceDtsFakingHost::new(host, project_reference_file_mapper, dts_directories)
}

pub struct ProjectReferenceFileMapper {
    pub(crate) opts: Option<ProgramOptions>,
    pub(crate) host: Option<module::ResolutionHostBox>,
    pub(crate) config_to_project_reference:
        HashMap<tspath::Path, Option<tsoptions::ParsedCommandLine>>,
    pub(crate) references_in_config_file: HashMap<tspath::Path, Vec<tspath::Path>>,
    pub(crate) source_to_project_reference: HashMap<tspath::Path, SourceOutputAndProjectReference>,
    pub(crate) output_dts_to_project_reference:
        HashMap<tspath::Path, SourceOutputAndProjectReference>,

    // Store all realpaths from .d.ts in node_modules to source files from project references.
    pub(crate) realpath_dts_to_source:
        collections::SyncMap<tspath::Path, SourceOutputAndProjectReference>,
}

impl Clone for ProjectReferenceFileMapper {
    fn clone(&self) -> Self {
        Self {
            opts: self.opts.clone(),
            host: None,
            config_to_project_reference: self.config_to_project_reference.clone(),
            references_in_config_file: self.references_in_config_file.clone(),
            source_to_project_reference: self.source_to_project_reference.clone(),
            output_dts_to_project_reference: self.output_dts_to_project_reference.clone(),
            realpath_dts_to_source: self.realpath_dts_to_source.clone(),
        }
    }
}

impl Default for ProjectReferenceFileMapper {
    fn default() -> Self {
        Self {
            opts: None,
            host: None,
            config_to_project_reference: HashMap::new(),
            references_in_config_file: HashMap::new(),
            source_to_project_reference: HashMap::new(),
            output_dts_to_project_reference: HashMap::new(),
            realpath_dts_to_source: collections::SyncMap::default(),
        }
    }
}

pub type SourceOutputAndProjectReference = tsoptions::SourceOutputAndProjectReference;

pub struct ProjectReferenceDtsFakingVfs<H> {
    pub host: Arc<H>,
    pub project_reference_file_mapper: ProjectReferenceFileMapper,
    pub dts_directories: HashSet<tspath::Path>,
    pub known_symlinks: Arc<Mutex<symlinks::KnownSymlinks>>,
}

impl<H> Clone for ProjectReferenceDtsFakingVfs<H> {
    fn clone(&self) -> Self {
        Self {
            host: Arc::clone(&self.host),
            project_reference_file_mapper: self.project_reference_file_mapper.clone(),
            dts_directories: self.dts_directories.clone(),
            known_symlinks: Arc::clone(&self.known_symlinks),
        }
    }
}

impl<H: CompilerHostLike> ProjectReferenceDtsFakingVfs<H> {
    pub fn use_case_sensitive_file_names(&self) -> bool {
        self.host.use_case_sensitive_file_names()
    }

    pub fn file_exists(&self, path: &str) -> bool {
        if self.host.file_exists(path) {
            return true;
        }
        if !tspath::is_declaration_file_name(path) {
            return false;
        }
        self.file_or_directory_exists_using_source(path, true)
    }

    pub fn read_file(&self, path: &str) -> Option<String> {
        self.host.read_file(path)
    }

    pub fn write_file(&self, _path: &str, _data: &str) {
        panic!("should not be called by resolver");
    }

    pub fn append_file(&self, _path: &str, _data: &str) {
        panic!("should not be called by resolver");
    }

    pub fn remove(&self, _path: &str) {
        panic!("should not be called by resolver");
    }

    pub fn chtimes(&self, _path: &str, _atime: SystemTime, _mtime: SystemTime) -> io::Result<()> {
        panic!("should not be called by resolver");
    }

    pub fn directory_exists(&self, path: &str) -> bool {
        if self.host.directory_exists(path) {
            self.handle_directory_could_be_symlink(path);
            return true;
        }
        self.file_or_directory_exists_using_source(path, false)
    }

    pub fn realpath(&self, path: &str) -> String {
        self.known_symlinks
            .lock()
            .unwrap()
            .files()
            .get(&self.to_path(path))
            .cloned()
            .unwrap_or_else(|| self.host.realpath(path))
    }

    pub fn to_path(&self, path: &str) -> tspath::Path {
        tspath::to_path(
            path,
            &self.host.current_directory(),
            self.host.use_case_sensitive_file_names(),
        )
    }

    pub fn get_accessible_entries(&self, _path: &str) -> Entries {
        panic!("should not be called by resolver");
    }

    pub fn stat(&self, _path: &str) -> io::Result<FileInfo> {
        panic!("should not be called by resolver");
    }

    pub fn walk_dir(
        &self,
        _root: &str,
        _walk_fn: &mut dyn FnMut(&str, DirEntry, Option<io::Error>) -> io::Result<()>,
    ) -> io::Result<()> {
        panic!("should not be called by resolver");
    }

    pub fn handle_directory_could_be_symlink(&self, directory: &str) {
        if tspath::contains_ignored_path(directory) || !directory.contains("/node_modules/") {
            return;
        }
        let directory_path = tspath::ensure_trailing_directory_separator(&self.to_path(directory));
        if self
            .known_symlinks
            .lock()
            .unwrap()
            .directories()
            .contains_key(&directory_path)
        {
            return;
        }
        let real_directory = self.realpath(directory);
        if real_directory == directory {
            return;
        }
        let real_path = tspath::ensure_trailing_directory_separator(&self.to_path(&real_directory));
        if real_path == directory_path {
            return;
        }
        self.known_symlinks.lock().unwrap().set_directory(
            directory.to_string(),
            directory_path,
            Some(symlinks::KnownDirectoryLink {
                real: tspath::ensure_trailing_directory_separator(&real_directory),
                real_path,
            }),
        );
    }

    pub fn file_or_directory_exists_using_source(
        &self,
        file_or_directory: &str,
        is_file: bool,
    ) -> bool {
        let direct = if is_file {
            self.file_exists_if_project_reference_dts(file_or_directory)
        } else {
            self.directory_exists_if_project_reference_decl_dir(file_or_directory)
        };
        if direct != Tristate::Unknown {
            return direct.is_true();
        }
        let known_directory_links = self.known_symlinks.lock().unwrap().directories().clone();
        if known_directory_links.is_empty() {
            return false;
        }
        let file_or_directory_path = self.to_path(file_or_directory);
        if !file_or_directory_path.contains("/node_modules/") {
            return false;
        }
        if is_file
            && self
                .known_symlinks
                .lock()
                .unwrap()
                .files()
                .contains_key(&file_or_directory_path)
        {
            return true;
        }
        for (directory_path, known_directory_link) in known_directory_links {
            let Some(known_directory_link) = known_directory_link else {
                continue;
            };
            let Some(relative) = file_or_directory_path.strip_prefix(&directory_path) else {
                continue;
            };
            let candidate = format!("{}{}", known_directory_link.real_path, relative);
            let exists = if is_file {
                self.file_exists_if_project_reference_dts(&candidate)
                    .is_true()
            } else {
                self.directory_exists_if_project_reference_decl_dir(&candidate)
                    .is_true()
            };
            if exists {
                if is_file {
                    let absolute_path = tspath::get_normalized_absolute_path(
                        file_or_directory,
                        &self.host.current_directory(),
                    );
                    self.known_symlinks.lock().unwrap().set_file(
                        absolute_path.clone(),
                        file_or_directory_path.clone(),
                        format!(
                            "{}{}",
                            known_directory_link.real,
                            &absolute_path[directory_path.len()..]
                        ),
                    );
                }
                return true;
            }
        }
        false
    }

    pub fn file_exists_if_project_reference_dts(&self, file: &str) -> Tristate {
        if let Some(source) = self
            .project_reference_file_mapper
            .get_project_reference_from_output_dts(self.to_path(file))
        {
            if self.host.file_exists(&source.source) {
                Tristate::True
            } else {
                Tristate::False
            }
        } else {
            Tristate::Unknown
        }
    }

    pub fn directory_exists_if_project_reference_decl_dir(&self, dir: &str) -> Tristate {
        let dir_path = self.to_path(dir);
        let dir_path_with_trailing = format!("{dir_path}/");
        for decl_dir_path in &self.dts_directories {
            if &dir_path == decl_dir_path
                || decl_dir_path.starts_with(&dir_path_with_trailing)
                || dir_path.starts_with(&format!("{decl_dir_path}/"))
            {
                return Tristate::True;
            }
        }
        Tristate::Unknown
    }
}

impl<H: CompilerHostLike> Fs for ProjectReferenceDtsFakingVfs<H> {
    fn use_case_sensitive_file_names(&self) -> bool {
        ProjectReferenceDtsFakingVfs::use_case_sensitive_file_names(self)
    }

    fn file_exists(&self, path: &str) -> bool {
        ProjectReferenceDtsFakingVfs::file_exists(self, path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        match ProjectReferenceDtsFakingVfs::read_file(self, path) {
            Some(contents) => (contents, true),
            None => (String::new(), false),
        }
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        ProjectReferenceDtsFakingVfs::write_file(self, path, data);
        Ok(())
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        ProjectReferenceDtsFakingVfs::append_file(self, path, data);
        Ok(())
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        ProjectReferenceDtsFakingVfs::remove(self, path);
        Ok(())
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        ProjectReferenceDtsFakingVfs::chtimes(self, path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        ProjectReferenceDtsFakingVfs::directory_exists(self, path)
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        ProjectReferenceDtsFakingVfs::get_accessible_entries(self, path)
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        ProjectReferenceDtsFakingVfs::stat(self, path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut ts_vfs::WalkDirFunc<'_>) -> io::Result<()> {
        ProjectReferenceDtsFakingVfs::walk_dir(self, root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        ProjectReferenceDtsFakingVfs::realpath(self, path)
    }
}
