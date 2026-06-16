use std::sync::{Arc, Mutex};

use ts_ast as ast;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_ls::RegistryCloneHost;
use ts_module as module;
use ts_packagejson as packagejson;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::overlayfs::FileContent;
use crate::snapshotfs::ToPath;
use crate::{
    FileHandle, FileHandleRef, FileSource, ParseCache, ParseCacheKey, ProjectCollection,
    SnapshotFsBuilder, SourceFs, new_disk_file, new_parse_cache_key, new_source_fs,
};

impl FileContent for FileHandleRef {
    fn content(&self) -> String {
        (**self).content()
    }

    fn hash(&self) -> u128 {
        (**self).hash()
    }
}

impl FileHandle for FileHandleRef {
    fn file_name(&self) -> String {
        (**self).file_name()
    }

    fn version(&self) -> i32 {
        (**self).version()
    }

    fn matches_disk_text(&self) -> bool {
        (**self).matches_disk_text()
    }

    fn is_overlay(&self) -> bool {
        (**self).is_overlay()
    }

    fn lsp_line_map(&self) -> &ts_ls::LspLineMap {
        (**self).lsp_line_map()
    }

    fn ecma_line_info(&self) -> &ts_sourcemap::ECMALineInfo {
        (**self).ecma_line_info()
    }

    fn kind(&self) -> ts_core::ScriptKind {
        (**self).kind()
    }
}

pub struct AutoImportBuilderFs {
    pub snapshot_fs_builder: SnapshotFsBuilder,
    pub untracked_files: collections::SyncMap<tspath::Path, FileHandleRef>,
}

impl AutoImportBuilderFs {
    pub fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync> {
        self.snapshot_fs_builder.fs()
    }

    pub fn get_file(&mut self, file_name: &str) -> Option<FileHandleRef> {
        let path = (self.snapshot_fs_builder.to_path)(file_name);
        self.get_file_by_path(file_name, &path)
    }

    pub fn get_file_by_path(
        &mut self,
        file_name: &str,
        path: &tspath::Path,
    ) -> Option<FileHandleRef> {
        // We want to avoid long-term caching of files referenced only by auto-imports, so we
        // override get_file_by_path to avoid collecting more files into the snapshot_fs_builder's
        // disk_files. (Note the reason we can't just use the finalized SnapshotFs is that changed
        // files not read during other parts of the snapshot clone will be marked as dirty, but
        // not yet refreshed from disk.)
        if let Some(overlay) = self.snapshot_fs_builder.overlays.get(path) {
            return Some(overlay.clone());
        }

        if self.snapshot_fs_builder.disk_files.load(path).is_some() {
            return self
                .snapshot_fs_builder
                .disk_files
                .value(path)
                .and_then(|disk_file| {
                    self.snapshot_fs_builder
                        .reload_entry_if_needed(&disk_file)
                        .map(|file| file as FileHandleRef)
                });
        }

        let (fh, ok) = self.untracked_files.load(path);
        if ok {
            return fh;
        }

        let (content, ok) = self.snapshot_fs_builder.fs().read_file(file_name);
        let fh = if ok {
            Some(Arc::new(new_disk_file(file_name.to_string(), content)) as FileHandleRef)
        } else {
            None
        };
        let (fh, _) = self.untracked_files.load_or_store(path.clone(), fh);
        fh
    }

    pub fn get_accessible_entries(&mut self, path: &str) -> vfs::Entries {
        self.snapshot_fs_builder.get_accessible_entries(path)
    }

    pub fn file_exists(&mut self, file_name: &str, path: &tspath::Path) -> bool {
        self.snapshot_fs_builder.file_exists(file_name, path)
    }
}

impl FileSource for AutoImportBuilderFs {
    fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync> {
        AutoImportBuilderFs::fs(self)
    }

    fn get_file(&mut self, file_name: &str) -> Option<FileHandleRef> {
        AutoImportBuilderFs::get_file(self, file_name)
    }

    fn get_file_by_path(&mut self, file_name: &str, path: &tspath::Path) -> Option<FileHandleRef> {
        AutoImportBuilderFs::get_file_by_path(self, file_name, path)
    }

    fn file_exists(&mut self, file_name: &str, path: &tspath::Path) -> bool {
        AutoImportBuilderFs::file_exists(self, file_name, path)
    }

    fn get_accessible_entries(&mut self, path: &str) -> vfs::Entries {
        AutoImportBuilderFs::get_accessible_entries(self, path)
    }
}

pub struct AutoImportRegistryCloneHost {
    pub project_collection: ProjectCollection,
    pub parse_cache: ParseCache,
    pub fs: SourceFs,
    pub current_directory: String,
    pub files_mu: Mutex<Vec<ParseCacheKey>>,
}

impl Clone for AutoImportRegistryCloneHost {
    fn clone(&self) -> Self {
        Self {
            project_collection: self.project_collection.clone_collection(),
            parse_cache: self.parse_cache.clone(),
            fs: self.fs.clone(),
            current_directory: self.current_directory.clone(),
            files_mu: Mutex::new(
                self.files_mu
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone(),
            ),
        }
    }
}

pub fn new_auto_import_registry_clone_host(
    project_collection: ProjectCollection,
    parse_cache: ParseCache,
    snapshot_fs_builder: SnapshotFsBuilder,
    current_directory: String,
    to_path: ToPath,
) -> AutoImportRegistryCloneHost {
    AutoImportRegistryCloneHost {
        project_collection,
        parse_cache,
        fs: new_source_fs(
            false,
            AutoImportBuilderFs {
                snapshot_fs_builder,
                untracked_files: collections::SyncMap::default(),
            },
            to_path,
        ),
        current_directory,
        files_mu: Mutex::new(Vec::new()),
    }
}

impl module::ResolutionHost for AutoImportRegistryCloneHost {
    fn get_current_directory(&self) -> String {
        self.current_directory.clone()
    }

    fn fs(&self) -> &dyn vfs::Fs {
        &self.fs
    }
}

impl RegistryCloneHost for AutoImportRegistryCloneHost {
    fn fs(&self) -> vfs::FS {
        self.fs.fs()
    }

    fn get_default_project(
        &self,
        path: tspath::Path,
    ) -> (tspath::Path, Option<&compiler::Program>) {
        let Some(project) = self.project_collection.get_default_project(path) else {
            return (tspath::Path::default(), None);
        };
        (project.config_file_path.clone(), project.get_program())
    }

    fn get_program_for_project(&self, project_path: tspath::Path) -> Option<&compiler::Program> {
        let project = self.project_collection.get_project_by_path(project_path)?;
        project.get_program()
    }

    fn get_package_json(&self, file_name: &str) -> Option<packagejson::InfoCacheEntry> {
        let fh = self.fs.get_file(file_name);
        let package_directory = tspath::get_directory_path(file_name);
        let Some(fh) = fh else {
            return Some(packagejson::InfoCacheEntry {
                directory_exists: vfs::Fs::directory_exists(&self.fs, &package_directory),
                package_directory,
                contents: None,
            });
        };

        match packagejson::parse(fh.content().as_bytes()) {
            Ok(fields) => Some(packagejson::InfoCacheEntry {
                directory_exists: true,
                package_directory: tspath::get_directory_path(file_name),
                contents: Some(packagejson::PackageJson::new(fields, true)),
            }),
            Err(_) => Some(packagejson::InfoCacheEntry {
                directory_exists: true,
                package_directory: tspath::get_directory_path(file_name),
                contents: Some(packagejson::PackageJson::default()),
            }),
        }
    }

    fn get_source_file(&self, file_name: &str, path: tspath::Path) -> Option<ast::SourceFile> {
        let fh = self.fs.get_file(file_name)?;
        let opts = ast::SourceFileParseOptions {
            file_name: file_name.to_string(),
            path,
            ..Default::default()
        };
        let key = new_parse_cache_key(opts, fh.hash(), fh.kind());
        let result = self.parse_cache.acquire(key.clone(), fh).into_source_file();

        self.files_mu
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(key);

        Some(result)
    }

    fn dispose(&self) {
        let files = self.files_mu.lock().unwrap_or_else(|err| err.into_inner());
        for key in files.iter() {
            self.parse_cache.deref(key);
        }
    }
}
