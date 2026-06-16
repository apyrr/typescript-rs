use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ts_ast as ast;
use ts_checker as checker;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_module as module;
use ts_packagejson as packagejson;
use ts_project_support::{dirty, logging};
use ts_symlinks as symlinks;
use ts_tspath as tspath;
use ts_vfs as vfs;

use crate::autoimport::{
    Export, Index, PathAndFileName, RegistryCloneHost, add_package_json_dependencies,
};
use crate::lsconv;
use crate::lsutil;

pub const KNOWN_RECURSIVE_SEARCH_PACKAGES: &[&str] = &[
    "@material-ui/core",
    "@material-ui/icons",
    "@sap/cds",
    "@testing-library/react-native",
    "ajv",
    "asap",
    "async",
    "aws-sdk",
    "braintree-web",
    "core-js",
    "core-js-pure",
    "crypto-js",
    "cypress-mochawesome-reporter",
    "dd-trace",
    "dumi",
    "dva",
    "egg-mock",
    "electron-log",
    "es-abstract",
    "es6-promise",
    "eslint-config-taro",
    "expo",
    "expo-router",
    "flow-remove-types",
    "gatsby",
    "glamor",
    "gluegun",
    "graphology-indices",
    "graphology-traversal",
    "graphology-utils",
    "jest-expo",
    "lodash",
    "lodash-es",
    "moment",
    "mz",
    "next",
    "pdfjs-dist",
    "protobufjs",
    "react-app-polyfill",
    "react-dev-utils",
    "react-devtools-inline",
    "recast",
    "semver",
    "stylelint-config-html",
    "umi",
    "web3-provider-engine",
    "webpack",
];

pub type NewProgramStructure = i32;

pub const NEW_PROGRAM_STRUCTURE_FALSE: NewProgramStructure = 0;
pub const NEW_PROGRAM_STRUCTURE_SAME_FILE_NAMES: NewProgramStructure = 1;
pub const NEW_PROGRAM_STRUCTURE_DIFFERENT_FILE_NAMES: NewProgramStructure = 2;

// bucketBuildPreferences holds user preferences that affect how a bucket is
// built. When any of these change between builds, the bucket must be rebuilt.
// Adding a new preference here automatically integrates it into the rebuild
// checks via Equal.
#[derive(Clone, Debug, Default)]
pub struct BucketBuildPreferences {
    pub file_exclude_patterns: Vec<String>,
    pub auto_import_entrypoint_directory_search: core::Tristate,
}

pub fn bucket_build_preferences_from_user_preferences(
    prefs: lsutil::UserPreferences,
) -> BucketBuildPreferences {
    BucketBuildPreferences {
        file_exclude_patterns: prefs.auto_import_file_exclude_patterns,
        auto_import_entrypoint_directory_search: prefs.auto_import_entrypoint_directory_search,
    }
}

impl BucketBuildPreferences {
    pub fn equal(&self, other: &BucketBuildPreferences) -> bool {
        core::unordered_equal(&self.file_exclude_patterns, &other.file_exclude_patterns)
            && self.auto_import_entrypoint_directory_search
                == other.auto_import_entrypoint_directory_search
    }
}

// BucketState represents the dirty state of a bucket.
// In general, a bucket can be used for an auto-imports request if it is clean
// or if the only edited file is the one that was requested for auto-imports.
// Most edits within a file will not change the imports available to that file.
// However, one exception causes the bucket to be rebuilt after a change to a
// single file: local files are newly added to the project by a manual import.
// This can only happen after a full (non-clone) program update. When this
// happens, the `newProgramStructure` flag is set until the next time the bucket
// is rebuilt, when this condition will be checked.
#[derive(Clone, Debug, Default)]
pub struct BucketState {
    // dirtyFile is the file that was edited last, if any. It does not necessarily
    // indicate that no other files have been edited, so it should be ignored if
    // `multipleFilesDirty` is set. It should not be used for node_modules buckets,
    // which rely on `dirtyPackages` instead.
    pub dirty_file: tspath::Path,
    pub multiple_files_dirty: bool,
    pub new_program_structure: NewProgramStructure,
    // buildPreferences holds the user preferences that were in effect when
    // the bucket was built. If changed, the bucket should be rebuilt.
    pub build_preferences: BucketBuildPreferences,
    // dirtyPackages is the set of package names that need to be re-indexed.
    // This is used for granular updates: when a file in a local workspace package
    // changes, only that package needs to be re-extracted rather than rebuilding
    // the entire node_modules bucket.
    // If nil, no granular updates are pending.
    // If set but multipleFilesDirty is true, the entire bucket needs to be rebuilt.
    pub dirty_packages: Option<collections::Set<String>>,
    // recursiveSearchPackages tracks which packages were recursively directory-searched
    // when the bucket was built. nil means all non-exports packages were searched
    // (e.g. when the autoImportEntrypointDirectorySearch preference is enabled).
    // A non-nil set lists only the specific packages that were searched.
    // Used for rebuild detection: a rebuild is triggered when target packages are
    // not a subset of the currently searched packages.
    pub recursive_search_packages: Option<collections::Set<String>>,
}

impl BucketState {
    pub fn dirty(&self) -> bool {
        self.multiple_files_dirty
            || !self.dirty_file.is_empty()
            || self.new_program_structure > 0
            || self
                .dirty_packages
                .as_ref()
                .is_some_and(|set| set.len() > 0)
    }

    pub fn dirty_file(&self) -> tspath::Path {
        if self.multiple_files_dirty {
            return String::new();
        }
        self.dirty_file.clone()
    }

    pub fn dirty_packages(&self) -> Option<&collections::Set<String>> {
        if self.multiple_files_dirty {
            return None;
        }
        self.dirty_packages.as_ref()
    }

    pub fn recursive_search_packages(&self) -> Option<&collections::Set<String>> {
        self.recursive_search_packages.as_ref()
    }

    pub fn possibly_needs_rebuild_for_file(
        &self,
        file: tspath::Path,
        preferences: lsutil::UserPreferences,
    ) -> bool {
        self.new_program_structure > 0
            || self.has_dirty_file_besides(&file)
            || !self
                .build_preferences
                .equal(&bucket_build_preferences_from_user_preferences(preferences))
            || self
                .dirty_packages
                .as_ref()
                .is_some_and(|set| set.len() > 0)
    }

    pub fn has_dirty_file_besides(&self, file: &tspath::Path) -> bool {
        self.multiple_files_dirty || (!self.dirty_file.is_empty() && &self.dirty_file != file)
    }
}

// recursiveSearchSubset reports whether target is a subset of current.
// nil represents "all packages" — a superset of every concrete set.
// Returns true if the current set already covers everything the target needs,
// meaning no rebuild is required for recursive search purposes.
pub fn recursive_search_subset(
    target: Option<&collections::Set<String>>,
    current: Option<&collections::Set<String>>,
) -> bool {
    match (target, current) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(_), None) => true,
        (Some(target), Some(current)) => target.is_subset_of(current),
    }
}

#[derive(Clone, Debug, Default)]
pub struct RegistryBucket {
    pub state: BucketState,
    pub paths: HashMap<tspath::Path, String>,
    pub package_files: HashMap<String, HashMap<tspath::Path, String>>,
    pub resolved_package_names: Option<collections::Set<String>>,
    pub dependency_names: Option<collections::Set<String>>,
    pub ambient_module_names: HashMap<String, Vec<String>>,
    pub index: Option<Index<Export>>,
}

pub fn new_registry_bucket() -> RegistryBucket {
    RegistryBucket {
        state: BucketState {
            multiple_files_dirty: true,
            new_program_structure: NEW_PROGRAM_STRUCTURE_DIFFERENT_FILE_NAMES,
            ..Default::default()
        },
        ..Default::default()
    }
}

impl RegistryBucket {
    // markProjectFileDirty should only be called within a Change call on the dirty map.
    // Buckets are considered immutable once in a finalized registry. Should only
    // be used for project buckets.
    pub fn mark_project_file_dirty(&mut self, file: tspath::Path) {
        if self.state.has_dirty_file_besides(&file) {
            self.state.multiple_files_dirty = true;
        } else {
            self.state.dirty_file = file;
        }
    }

    // markNodeModulesDirty should only be called within a Change call on the dirty map.
    // Buckets are considered immutable once in a finalized registry. If packageName is
    // non-empty, that package is marked for granular update. Otherwise, the entire bucket
    // is marked dirty.
    pub fn mark_node_modules_dirty(&mut self, package_name: &str) {
        if self.state.multiple_files_dirty {
            return;
        }
        if package_name.is_empty() {
            self.state.multiple_files_dirty = true;
            return;
        }
        if self.state.dirty_packages.is_none() {
            self.state.dirty_packages = Some(collections::Set::default());
        }
        self.state
            .dirty_packages
            .as_mut()
            .unwrap()
            .add(package_name.to_string());
    }
}

#[derive(Clone, Debug, Default)]
pub struct Directory {
    pub name: String,
    pub package_json: Option<packagejson::InfoCacheEntry>,
    pub has_node_modules: bool,
}

#[derive(Clone, Default)]
pub struct Registry {
    pub(crate) to_path: Option<Arc<dyn Fn(String) -> tspath::Path + Send + Sync>>,
    pub(crate) user_preferences: lsutil::UserPreferences,
    pub(crate) directories: HashMap<tspath::Path, Directory>,
    pub(crate) node_modules: HashMap<tspath::Path, RegistryBucket>,
    pub(crate) projects: HashMap<tspath::Path, RegistryBucket>,
    pub(crate) unique_package_count: usize,
    // entrypoints maps from file path to the resolved entrypoints for that file, shared across all node_modules buckets.
    pub(crate) entrypoints: HashMap<tspath::Path, Vec<module::ResolvedEntrypoint>>,
    // specifierCache maps from importing file to target file to specifier.
    pub(crate) specifier_cache: HashMap<tspath::Path, collections::SyncMap<tspath::Path, String>>,
}

pub fn new_registry(
    to_path: impl Fn(String) -> tspath::Path + Send + Sync + 'static,
    preferences: lsutil::UserPreferences,
) -> Registry {
    Registry {
        to_path: Some(Arc::new(to_path)),
        user_preferences: preferences,
        directories: HashMap::new(),
        ..Default::default()
    }
}

impl Registry {
    pub fn is_prepared_for_importing_file(
        &self,
        file_name: &str,
        project_path: tspath::Path,
        preferences: lsutil::UserPreferences,
    ) -> bool {
        let Some(project_bucket) = self.projects.get(&project_path) else {
            return false;
        };
        let Some(to_path) = &self.to_path else {
            return false;
        };
        let path = to_path(file_name.to_string());
        if project_bucket
            .state
            .possibly_needs_rebuild_for_file(path.clone(), preferences.clone())
        {
            return false;
        }

        let mut dir_path = tspath::get_directory_path(&path);
        loop {
            if let Some(dir_bucket) = self.node_modules.get(&dir_path) {
                if dir_bucket
                    .state
                    .possibly_needs_rebuild_for_file(path.clone(), preferences.clone())
                {
                    return false;
                }
            }
            let parent = tspath::get_directory_path(&dir_path);
            if parent == dir_path {
                break;
            }
            dir_path = parent;
        }
        true
    }

    pub fn node_modules_directories(&self) -> HashMap<tspath::Path, String> {
        let mut dirs = HashMap::new();
        for (dir_path, dir) in &self.directories {
            if dir.has_node_modules {
                dirs.insert(
                    tspath::combine_paths(dir_path, &["node_modules"]),
                    tspath::combine_paths(&dir.name, &["node_modules"]),
                );
            }
        }
        dirs
    }

    pub fn clone_registry(
        &self,
        ctx: core::Context,
        change: RegistryChange,
        host: &dyn RegistryCloneHost,
        logger: Option<&logging::LogTree>,
    ) -> Result<Registry, String> {
        let mut builder = new_registry_builder(self, host);
        if let Some(user_preferences) = change.user_preferences.clone() {
            builder.user_preferences = user_preferences;
            if !core::unordered_equal(
                &builder
                    .user_preferences
                    .auto_import_specifier_exclude_regexes,
                &self.user_preferences.auto_import_specifier_exclude_regexes,
            ) {
                builder.specifier_cache.clear();
            }
        }
        let change = RegistryChange {
            user_preferences: None,
            ..change
        };
        builder.update_bucket_and_directory_existence(change.clone(), logger);
        builder.mark_buckets_dirty(change.clone(), logger);
        if !change.requested_file.is_empty() {
            builder
                .update_indexes(ctx, change, logger)
                .map_err(|err| err.message)?;
        }
        Ok(builder.build())
    }

    pub fn get_cache_stats(&self) -> CacheStats {
        let mut stats = CacheStats {
            unique_package_count: self.unique_package_count,
            ..Default::default()
        };

        for (path, bucket) in &self.projects {
            let export_count = bucket
                .index
                .as_ref()
                .map(|idx| idx.entries.len())
                .unwrap_or(0);
            stats.project_buckets.push(BucketStats {
                path: path.clone(),
                export_count,
                file_count: bucket.paths.len(),
                state: bucket.state.clone(),
                dependency_names: bucket.dependency_names.clone(),
                package_names: None,
            });
        }

        for (path, bucket) in &self.node_modules {
            let export_count = bucket
                .index
                .as_ref()
                .map(|idx| idx.entries.len())
                .unwrap_or(0);
            let mut file_count = 0usize;
            let mut package_names = collections::Set::default();
            for (name, paths) in &bucket.package_files {
                package_names.add(name.clone());
                file_count += paths.len();
            }
            stats.node_modules_buckets.push(BucketStats {
                path: path.clone(),
                export_count,
                file_count,
                state: bucket.state.clone(),
                dependency_names: bucket.dependency_names.clone(),
                package_names: Some(package_names),
            });
        }

        stats.project_buckets.sort_by(|a, b| a.path.cmp(&b.path));
        stats
            .node_modules_buckets
            .sort_by(|a, b| a.path.cmp(&b.path));
        stats
    }
}

#[derive(Clone, Debug, Default)]
pub struct BucketStats {
    pub path: tspath::Path,
    pub export_count: usize,
    pub file_count: usize,
    pub state: BucketState,
    pub dependency_names: Option<collections::Set<String>>,
    pub package_names: Option<collections::Set<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct CacheStats {
    pub project_buckets: Vec<BucketStats>,
    pub node_modules_buckets: Vec<BucketStats>,
    pub unique_package_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct RegistryChange {
    pub(crate) requested_file: tspath::Path,
    pub(crate) open_files: HashMap<tspath::Path, String>,
    pub(crate) changed: collections::Set<lsproto::DocumentUri>,
    pub(crate) created: collections::Set<lsproto::DocumentUri>,
    pub(crate) deleted: collections::Set<lsproto::DocumentUri>,
    pub(crate) rebuilt_programs: HashMap<tspath::Path, bool>,
    pub(crate) user_preferences: Option<lsutil::UserPreferences>,
}

impl RegistryChange {
    pub fn new(
        requested_file: tspath::Path,
        open_files: HashMap<tspath::Path, String>,
        changed: collections::Set<lsproto::DocumentUri>,
        created: collections::Set<lsproto::DocumentUri>,
        deleted: collections::Set<lsproto::DocumentUri>,
        rebuilt_programs: HashMap<tspath::Path, bool>,
        user_preferences: Option<lsutil::UserPreferences>,
    ) -> Self {
        Self {
            requested_file,
            open_files,
            changed,
            created,
            deleted,
            rebuilt_programs,
            user_preferences,
        }
    }
}

pub struct RegistryBuilder<'a> {
    pub host: &'a dyn RegistryCloneHost,
    pub base: &'a Registry,
    pub user_preferences: lsutil::UserPreferences,
    pub directories: dirty::MapBuilder<tspath::Path, Directory, Directory>,
    pub node_modules: dirty::MapBuilder<tspath::Path, RegistryBucket, RegistryBucket>,
    pub projects: dirty::MapBuilder<tspath::Path, RegistryBucket, RegistryBucket>,
    pub specifier_cache: dirty::MapBuilder<
        tspath::Path,
        collections::SyncMap<tspath::Path, String>,
        collections::SyncMap<tspath::Path, String>,
    >,
    pub resolver_options: module::ResolverOptions,
    pub unique_package_count: usize,
    pub entrypoints: dirty::MapBuilder<
        tspath::Path,
        Vec<module::ResolvedEntrypoint>,
        Vec<module::ResolvedEntrypoint>,
    >,
}

pub fn new_registry_builder<'a>(
    registry: &'a Registry,
    host: &'a dyn RegistryCloneHost,
) -> RegistryBuilder<'a> {
    RegistryBuilder {
        host,
        base: registry,
        user_preferences: registry.user_preferences.clone(),
        directories: dirty::new_map_builder(
            registry.directories.clone(),
            core::identity,
            core::identity,
        ),
        node_modules: dirty::new_map_builder(
            registry.node_modules.clone(),
            core::identity,
            core::identity,
        ),
        projects: dirty::new_map_builder(registry.projects.clone(), core::identity, core::identity),
        specifier_cache: dirty::new_map_builder(
            registry.specifier_cache.clone(),
            core::identity,
            core::identity,
        ),
        resolver_options: module::ResolverOptions::default(),
        unique_package_count: registry.unique_package_count,
        entrypoints: dirty::new_map_builder(
            registry.entrypoints.clone(),
            core::identity,
            core::identity,
        ),
    }
}

impl<'a> RegistryBuilder<'a> {
    pub fn build(&self) -> Registry {
        Registry {
            to_path: self.base.to_path.clone(),
            user_preferences: self.user_preferences.clone(),
            directories: self.directories.build(),
            node_modules: self.node_modules.build(),
            projects: self.projects.build(),
            specifier_cache: self.specifier_cache.build(),
            unique_package_count: self.unique_package_count,
            entrypoints: self.entrypoints.build(),
        }
    }

    pub fn update_bucket_and_directory_existence(
        &mut self,
        change: RegistryChange,
        _logger: Option<&logging::LogTree>,
    ) {
        let mut needed_projects = HashMap::<tspath::Path, ()>::new();
        let mut needed_directories = HashMap::<tspath::Path, String>::new();
        for (path, file_name) in &change.open_files {
            let (project_path, _) = self.host.get_default_project(path.clone());
            needed_projects.insert(project_path, ());
            if tspath::is_dynamic_file_name(file_name) {
                continue;
            }
            let mut dir = file_name.clone();
            let mut dir_path = path.clone();
            loop {
                dir = tspath::get_directory_path(&dir);
                let last_dir_path = dir_path;
                dir_path = tspath::get_directory_path(&last_dir_path);
                if dir_path == last_dir_path {
                    break;
                }
                if needed_directories.contains_key(&dir_path) {
                    break;
                }
                needed_directories.insert(dir_path.clone(), dir.clone());
            }

            if !self.specifier_cache.has(path) {
                self.specifier_cache
                    .set(path.clone(), collections::SyncMap::default());
            }
        }

        if !change.requested_file.is_empty() {
            let (project_path, _) = self.host.get_default_project(change.requested_file.clone());
            needed_projects.insert(project_path, ());
            if !self.specifier_cache.has(&change.requested_file) {
                self.specifier_cache.set(
                    change.requested_file.clone(),
                    collections::SyncMap::default(),
                );
            }
        }

        for path in self.base.specifier_cache.keys() {
            if !change.open_files.contains_key(path) && path != &change.requested_file {
                self.specifier_cache.delete(path.clone());
            }
        }

        for project_path in needed_projects.keys() {
            if !self.base.projects.contains_key(project_path) {
                self.projects
                    .set(project_path.clone(), new_registry_bucket());
            }
        }
        for project_path in self.base.projects.keys() {
            if !needed_projects.contains_key(project_path) {
                self.projects.delete(project_path.clone());
            }
        }

        for (dir_path, dir_name) in &needed_directories {
            let package_json_uri = lsconv::file_name_to_document_uri(&tspath::combine_paths(
                dir_name,
                &["package.json"],
            ));
            let package_json_changed = change.changed.has(&package_json_uri)
                || change.deleted.has(&package_json_uri)
                || change.created.has(&package_json_uri);
            if self.base.directories.contains_key(dir_path) && !package_json_changed {
                continue;
            }
            self.update_directory(dir_path.clone(), dir_name.clone(), package_json_changed);
        }

        for (dir_path, dir) in &self.base.directories {
            if !needed_directories.contains_key(dir_path) {
                self.directories.delete(dir_path.clone());
                if dir.has_node_modules {
                    self.node_modules.delete(dir_path.clone());
                }
            }
        }
    }

    fn update_directory(
        &mut self,
        dir_path: tspath::Path,
        dir_name: String,
        package_json_changed: bool,
    ) {
        let package_json_file_name = tspath::combine_paths(&dir_name, &["package.json"]);
        let has_node_modules = crate::autoimport::RegistryCloneHost::fs(self.host)
            .directory_exists(&tspath::combine_paths(&dir_name, &["node_modules"]));
        let next = Directory {
            name: dir_name,
            package_json: self.host.get_package_json(&package_json_file_name),
            has_node_modules,
        };
        let should_update = self
            .directories
            .build()
            .get(&dir_path)
            .is_none_or(|dir| package_json_changed || dir.has_node_modules != has_node_modules);
        if should_update {
            self.directories.set(dir_path.clone(), next);
        }

        if package_json_changed {
            // package.json changes affecting node_modules are handled by comparing dependencies in updateIndexes
            return;
        }

        if has_node_modules {
            if !self.node_modules.has(&dir_path) {
                self.node_modules.set(dir_path, new_registry_bucket());
            }
        } else if self.node_modules.has(&dir_path) {
            self.node_modules.delete(dir_path);
        }
    }

    pub fn mark_buckets_dirty(
        &mut self,
        change: RegistryChange,
        _logger: Option<&logging::LogTree>,
    ) {
        // Mark new program structures
        for (project_path, new_file_names) in &change.rebuilt_programs {
            let Some(mut bucket) = self.projects.build().get(project_path).cloned() else {
                continue;
            };
            bucket.state.new_program_structure = if *new_file_names {
                NEW_PROGRAM_STRUCTURE_DIFFERENT_FILE_NAMES
            } else {
                NEW_PROGRAM_STRUCTURE_SAME_FILE_NAMES
            };
            self.projects.set(project_path.clone(), bucket);
        }

        // Mark files dirty, bailing out if all buckets already have multiple files dirty
        let mut clean_node_modules_buckets = HashMap::new();
        for (dir_path, bucket) in self.node_modules.build() {
            if !bucket.state.multiple_files_dirty {
                clean_node_modules_buckets.insert(dir_path, bucket);
            }
        }
        let mut clean_project_buckets = HashMap::new();
        for (dir_path, bucket) in self.projects.build() {
            if !bucket.state.multiple_files_dirty {
                clean_project_buckets.insert(dir_path, bucket);
            }
        }

        let to_path = self.base.to_path.clone();
        let mut mark_files_dirty = |uris: &collections::Set<lsproto::DocumentUri>| {
            if clean_node_modules_buckets.is_empty() && clean_project_buckets.is_empty() {
                return;
            }
            let Some(to_path) = to_path.as_ref() else {
                return;
            };
            for uri in uris.keys().into_iter().flatten() {
                let path = to_path(uri.file_name());
                if !clean_node_modules_buckets.is_empty() {
                    // For node_modules, mark the bucket dirty if anything changes in the directory.
                    // The path could be either a symlink path (containing /node_modules/) or a realpath
                    // (for symlinked project references). Both are recorded in Paths for granular updates.
                    if let Some(node_modules_index) = path.find("/node_modules/") {
                        let dir_path = path[..node_modules_index].to_string();
                        if let Some(mut bucket) = clean_node_modules_buckets.remove(&dir_path) {
                            let package_name = bucket.paths.get(&path).cloned().unwrap_or_default();
                            bucket.mark_node_modules_dirty(&package_name);
                            let multiple_files_dirty = bucket.state.multiple_files_dirty;
                            self.node_modules.set(dir_path.clone(), bucket.clone());
                            if multiple_files_dirty {
                                clean_node_modules_buckets.insert(dir_path, bucket);
                            }
                        }
                    } else {
                        // Check if this path (possibly a realpath of a workspace package) is in any bucket's Paths.
                        // This handles local workspace packages where the realpath doesn't contain /node_modules/.
                        let bucket_dir_paths = clean_node_modules_buckets
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>();
                        for bucket_dir_path in bucket_dir_paths {
                            let Some(mut bucket) =
                                clean_node_modules_buckets.remove(&bucket_dir_path)
                            else {
                                continue;
                            };
                            let Some(package_name) = bucket.paths.get(&path).cloned() else {
                                clean_node_modules_buckets.insert(bucket_dir_path, bucket);
                                continue;
                            };
                            bucket.mark_node_modules_dirty(&package_name);
                            let multiple_files_dirty = bucket.state.multiple_files_dirty;
                            self.node_modules
                                .set(bucket_dir_path.clone(), bucket.clone());
                            if multiple_files_dirty {
                                clean_node_modules_buckets.insert(bucket_dir_path, bucket);
                            }
                        }
                    }
                }

                // For projects, mark the bucket dirty if the bucket contains the file directly.
                // Any other significant change, like a created failed lookup location, is
                // handled by newProgramStructure.
                let project_dir_paths = clean_project_buckets.keys().cloned().collect::<Vec<_>>();
                for project_dir_path in project_dir_paths {
                    let Some(mut bucket) = clean_project_buckets.remove(&project_dir_path) else {
                        continue;
                    };
                    if !bucket.paths.contains_key(&path) {
                        clean_project_buckets.insert(project_dir_path, bucket);
                        continue;
                    }
                    // Project buckets don't use package-based granular updates
                    bucket.mark_project_file_dirty(path.clone());
                    let multiple_files_dirty = bucket.state.multiple_files_dirty;
                    self.projects.set(project_dir_path.clone(), bucket.clone());
                    if multiple_files_dirty {
                        clean_project_buckets.insert(project_dir_path, bucket);
                    }
                }
            }
        };

        mark_files_dirty(&change.created);
        mark_files_dirty(&change.deleted);
        mark_files_dirty(&change.changed);
    }

    pub fn update_indexes(
        &mut self,
        ctx: core::Context,
        change: RegistryChange,
        logger: Option<&logging::LogTree>,
    ) -> Result<(), core::Error> {
        #[derive(Default)]
        struct NodeModulesBucketTask {
            dependency_names: Option<collections::Set<String>>,
            dir_name: String,
            dir_path: tspath::Path,
            is_update: bool,
            existing_bucket: Option<RegistryBucket>,
            dirty_packages: Option<collections::Set<String>>,
            package_names: Option<collections::Set<String>>,
            directory_package_names: Option<collections::Set<String>>,
            discovered: Vec<DiscoveredPackage>,
            discover_err: Option<String>,
        }

        let (project_path, _) = self.host.get_default_project(change.requested_file.clone());
        if project_path.is_empty() {
            return Ok(());
        }

        // Compute resolved package names and project reference output mappings for all projects upfront.
        // Resolved package names are needed to compute node_modules dependencies so packages that are
        // directly imported by programs are included even if not listed in package.json.
        // Project reference output mappings are needed to redirect extraction from output .d.ts files
        // to source files for packages that are project references.
        // We need all projects because a node_modules directory can be used by multiple projects.
        let mut all_resolved_package_names = HashMap::new();
        let mut project_reference_outputs = HashMap::new();
        // Compute which packages have implicit deep imports (subpath imports in packages
        // without exports). These packages need recursive directory search to discover
        // all auto-importable files, even when the preference is disabled.
        let mut all_deep_import_packages = collections::Set::default();
        for project_path in self.projects.build().keys() {
            if let Some(program) = self.host.get_program_for_project(project_path.clone()) {
                all_resolved_package_names.insert(
                    project_path.clone(),
                    crate::autoimport::get_resolved_package_names(ctx.clone(), program)?,
                );
                crate::autoimport::add_project_reference_output_mappings(
                    program,
                    &mut project_reference_outputs,
                );
                for name in program
                    .deep_import_package_names_for_auto_imports()
                    .keys()
                    .into_iter()
                    .flatten()
                {
                    all_deep_import_packages.add(name.clone());
                }
            }
        }

        let file_exclude_patterns = self
            .user_preferences
            .parsed_auto_import_file_exclude_patterns(
                crate::autoimport::RegistryCloneHost::fs(self.host).use_case_sensitive_file_names(),
            );

        // Determine which packages need recursive directory search for this build.
        // nil means all packages (preference is enabled for all).
        let mut target_recursive_packages = (!self
            .user_preferences
            .auto_import_entrypoint_directory_search
            .is_true())
        .then_some(all_deep_import_packages);

        // --- Collect node_modules tasks ---
        let node_modules_snapshot = self.node_modules.build();
        let directories_snapshot = self.directories.build();
        let mut node_modules_tasks = Vec::<NodeModulesBucketTask>::new();
        tspath::for_each_ancestor_directory_path(change.requested_file.clone(), |dir_path| {
            let Some(node_modules_bucket) = node_modules_snapshot.get(dir_path) else {
                return None::<()>;
            };
            let dir_name = directories_snapshot
                .get(dir_path)
                .map(|dir| dir.name.clone())
                .unwrap_or_default();
            let dependencies = self.compute_dependencies_for_node_modules_directory(
                change.clone(),
                all_resolved_package_names.clone(),
                &dir_name,
                dir_path.clone(),
            );
            let bucket_state = &node_modules_bucket.state;
            // !!! Optimization: handle different dependency set via granular updates
            let needs_full_rebuild = bucket_state.multiple_files_dirty
                || node_modules_bucket.dependency_names != dependencies
                || !bucket_state.build_preferences.equal(
                    &bucket_build_preferences_from_user_preferences(self.user_preferences.clone()),
                )
                || !recursive_search_subset(
                    target_recursive_packages.as_ref(),
                    bucket_state.recursive_search_packages.as_ref(),
                );
            let dirty_packages = bucket_state.dirty_packages().cloned();
            let can_do_granular_update = !needs_full_rebuild
                && dirty_packages
                    .as_ref()
                    .is_some_and(|packages| packages.len() > 0);

            if needs_full_rebuild {
                node_modules_tasks.push(NodeModulesBucketTask {
                    dependency_names: dependencies,
                    dir_name: dir_name.to_string(),
                    dir_path: dir_path.to_string(),
                    ..Default::default()
                });
            } else if can_do_granular_update {
                node_modules_tasks.push(NodeModulesBucketTask {
                    dependency_names: dependencies,
                    dir_name: dir_name.to_string(),
                    dir_path: dir_path.to_string(),
                    is_update: true,
                    existing_bucket: Some(node_modules_bucket.clone()),
                    dirty_packages,
                    ..Default::default()
                });
            }
            None::<()>
        });

        // --- Phase 1: Discovery ---
        // Resolve package.json and realpath for each package in each bucket.
        for task in &mut node_modules_tasks {
            if task.is_update {
                task.package_names = task.dirty_packages.clone();
            } else {
                match crate::autoimport::get_package_names_in_node_modules(
                    &tspath::combine_paths(&task.dir_name, &["node_modules"]),
                    crate::autoimport::RegistryCloneHost::fs(self.host).as_ref(),
                ) {
                    Ok(directory_package_names) => {
                        task.directory_package_names = Some(directory_package_names.clone());
                        task.package_names = task
                            .dependency_names
                            .clone()
                            .or(Some(directory_package_names));
                    }
                    Err(err) => {
                        task.discover_err = Some(err.to_string());
                        continue;
                    }
                }
            }
            if let Some(package_names) = task.package_names.as_ref() {
                task.discovered = self.discover_bucket_packages(
                    package_names,
                    &task.dir_name,
                    task.dir_path.clone(),
                );
            }
        }

        // --- Phase 2: Extraction ---
        // Extract from main packages first. If a main package has no TypeScript entrypoints,
        // we fall back to extracting from @types in a second pass. Packages with no main
        // package extract directly from @types in the primary pass.
        let mut seen = HashMap::<String, bool>::new();
        let mut extraction_cache = HashMap::<String, PerPackageExtractionResult>::new();
        // Collect all packages that have an @types fallback. After the primary pass, we
        // filter to only those whose main extraction failed, then deduplicate by typesRealpath.
        let mut types_fallback_candidates = Vec::<DiscoveredPackage>::new();
        for task in &node_modules_tasks {
            if task.discover_err.is_some() {
                continue;
            }
            for package in &task.discovered {
                if !package.realpath.is_empty() {
                    if !seen.contains_key(&package.realpath) {
                        seen.insert(package.realpath.clone(), true);
                        let enable_dir_search = target_recursive_packages
                            .as_ref()
                            .is_none_or(|packages| packages.has(&package.package_name))
                            || KNOWN_RECURSIVE_SEARCH_PACKAGES
                                .contains(&package.package_name.as_str());
                        // Record actual directory-searched packages so the stored set
                        // reflects reality for rebuild detection and stats.
                        if enable_dir_search {
                            if let Some(packages) = target_recursive_packages.as_mut() {
                                packages.add(package.package_name.clone());
                            }
                        }
                        if let Some(package_json) = package.package_json.as_ref() {
                            if let Some(result) = self.extract_package(
                                ctx.clone(),
                                package_json,
                                &package.package_name,
                                project_reference_outputs.clone(),
                                file_exclude_patterns.as_ref(),
                                enable_dir_search,
                            ) {
                                extraction_cache.insert(package.realpath.clone(), result);
                            }
                        }
                    }
                    if !package.types_realpath.is_empty() {
                        types_fallback_candidates.push(package.clone());
                    }
                } else if !package.types_realpath.is_empty()
                    && !seen.contains_key(&package.types_realpath)
                {
                    seen.insert(package.types_realpath.clone(), true);
                    // @types packages always get directory search
                    if let Some(packages) = target_recursive_packages.as_mut() {
                        packages.add(package.package_name.clone());
                    }
                    if let Some(types_package_json) = package.types_package_json.as_ref() {
                        if let Some(result) = self.extract_package(
                            ctx.clone(),
                            types_package_json,
                            &package.package_name,
                            project_reference_outputs.clone(),
                            file_exclude_patterns.as_ref(),
                            true,
                        ) {
                            extraction_cache.insert(package.types_realpath.clone(), result);
                        }
                    }
                }
            }
        }

        // For packages whose main extraction yielded nothing, fall back to @types.
        for package in types_fallback_candidates {
            if extraction_cache.contains_key(&package.realpath)
                || seen.contains_key(&package.types_realpath)
            {
                continue;
            }
            seen.insert(package.types_realpath.clone(), true);
            // @types fallback packages always get directory search
            if let Some(packages) = target_recursive_packages.as_mut() {
                packages.add(package.package_name.clone());
            }
            if let Some(types_package_json) = package.types_package_json.as_ref() {
                if let Some(result) = self.extract_package(
                    ctx.clone(),
                    types_package_json,
                    &package.package_name,
                    project_reference_outputs.clone(),
                    file_exclude_patterns.as_ref(),
                    true,
                ) {
                    extraction_cache.insert(package.types_realpath.clone(), result);
                }
            }
        }
        self.unique_package_count = seen.len();

        // --- Phase 3: Bucket building ---
        // Each bucket installs the shared extraction results and builds its index.
        let mut all_results = Vec::<RegistryBuildResult>::new();
        for task in node_modules_tasks {
            let mut result = RegistryBuildResult {
                entry_key: task.dir_path.clone(),
                err: task.discover_err.clone(),
                ..Default::default()
            };
            if result.err.is_none() {
                if task.is_update {
                    if let (Some(existing_bucket), Some(dirty_packages)) =
                        (task.existing_bucket.as_ref(), task.dirty_packages.as_ref())
                    {
                        self.update_node_modules_bucket(
                            ctx.clone(),
                            &mut result,
                            existing_bucket,
                            dirty_packages,
                            task.discovered,
                            extraction_cache.clone(),
                            target_recursive_packages.clone(),
                            logger,
                        );
                    }
                } else {
                    self.build_node_modules_bucket(
                        ctx.clone(),
                        &mut result,
                        task.dependency_names,
                        task.dir_path,
                        task.discovered,
                        task.directory_package_names,
                        extraction_cache.clone(),
                        target_recursive_packages.clone(),
                        logger,
                    );
                }
            }
            all_results.push(result);
        }

        // Project bucket (not part of the three-phase pipeline — no cross-bucket dedup needed).
        if let Some(project_bucket) = self.projects.build().get(&project_path).cloned() {
            let program = self.host.get_program_for_project(project_path.clone());
            let resolved_package_names = all_resolved_package_names.get(&project_path).cloned();
            let mut should_rebuild = project_bucket
                .state
                .has_dirty_file_besides(&change.requested_file)
                || !project_bucket.state.build_preferences.equal(
                    &bucket_build_preferences_from_user_preferences(self.user_preferences.clone()),
                );
            if !should_rebuild && project_bucket.state.new_program_structure > 0 {
                if project_bucket.resolved_package_names != resolved_package_names
                    || program.is_some_and(|program| {
                        has_new_non_node_modules_files(program, &project_bucket)
                    })
                {
                    should_rebuild = true;
                } else {
                    let mut bucket = project_bucket.clone();
                    bucket.state.new_program_structure = NEW_PROGRAM_STRUCTURE_FALSE;
                    self.projects.set(project_path.clone(), bucket);
                }
            }
            if should_rebuild {
                let mut result = RegistryBuildResult {
                    entry_key: project_path.clone(),
                    ..Default::default()
                };
                self.build_project_bucket(
                    ctx,
                    &mut result,
                    project_path,
                    resolved_package_names,
                    logger,
                );
                all_results.push(result);
            }
        }

        for result in all_results {
            if result.err.is_some() {
                continue;
            }
            for path in result.removed_entrypoint_paths {
                if self.entrypoints.has(&path) {
                    self.entrypoints.delete(path);
                }
            }
            for (path, entries) in result.entrypoints {
                self.entrypoints.set(path, entries);
            }
            if let Some(bucket) = result.bucket {
                if self.node_modules.has(&result.entry_key) {
                    self.node_modules.set(result.entry_key, bucket);
                } else if self.projects.has(&result.entry_key) {
                    self.projects.set(result.entry_key, bucket);
                }
            }
        }
        Ok(())
    }

    pub fn build_project_bucket(
        &mut self,
        ctx: core::Context,
        result: &mut RegistryBuildResult,
        project_path: tspath::Path,
        resolved_package_names: Option<collections::Set<String>>,
        _logger: Option<&logging::LogTree>,
    ) {
        let Some(program) = self.host.get_program_for_project(project_path) else {
            result.err = Some("program not found for project bucket".to_string());
            return;
        };
        let file_exclude_patterns = self
            .user_preferences
            .parsed_auto_import_file_exclude_patterns(
                crate::autoimport::RegistryCloneHost::fs(self.host).use_case_sensitive_file_names(),
            );
        let symlink_cache = program.get_symlink_cache_for_auto_imports();
        let mut exports = HashMap::new();
        let to_path = self.base.to_path.clone();

        for file in program.source_files_for_auto_imports() {
            if is_ignored_file(program, &file) {
                continue;
            }
            if file_exclude_patterns
                .as_ref()
                .is_some_and(|patterns| patterns.match_string(&file.file_name()))
            {
                continue;
            }
            // Skip all node_modules files - they are always handled by node_modules buckets.
            // This simplifies the logic and ensures exports are indexed consistently.
            if file.file_name().contains("/node_modules/") {
                continue;
            }
            // Skip files that are realpaths of symlinks in node_modules.
            // These files will be indexed via their symlinked path in node_modules buckets.
            if has_symlink_to_node_modules(file.path(), &symlink_cache) {
                continue;
            }

            let to_path_for_extractor = to_path.as_ref().map(|to_path| {
                let to_path = to_path.clone();
                Box::new(move |file_name: String| to_path(file_name))
                    as Box<dyn Fn(String) -> tspath::Path + Send + Sync>
            });
            let file_exports = program
                .with_type_checker_for_file_using(
                    compiler::CheckerAccess::context(&ctx),
                    &file,
                    |checker| {
                        let mut extractor = crate::autoimport::new_export_extractor(
                            String::new(),
                            checker,
                            crate::autoimport::get_module_resolver(
                                self.host,
                                |path| path.to_string(),
                                self.resolver_options.clone(),
                            ),
                            to_path_for_extractor,
                            None,
                        );
                        Ok::<_, core::Error>(extractor.extract_from_file(&file))
                    },
                )
                .expect("project auto-import extraction requires scoped checker access");
            exports.insert(file.path(), file_exports);
        }

        let mut index = Index::default();
        let mut paths = HashMap::with_capacity(exports.len());
        for (path, file_exports) in exports {
            paths.insert(path, String::new()); // Empty string for project buckets
            for export in file_exports {
                index.insert_as_words(export);
            }
        }

        result.bucket = Some(RegistryBucket {
            paths,
            index: Some(index),
            resolved_package_names,
            state: BucketState {
                build_preferences: bucket_build_preferences_from_user_preferences(
                    self.user_preferences.clone(),
                ),
                ..Default::default()
            },
            ..Default::default()
        });
    }

    pub fn compute_dependencies_for_node_modules_directory(
        &self,
        change: RegistryChange,
        all_resolved_package_names: HashMap<tspath::Path, collections::Set<String>>,
        _dir_name: &str,
        dir_path: tspath::Path,
    ) -> Option<collections::Set<String>> {
        let compare_options = tspath::ComparePathsOptions {
            use_case_sensitive_file_names: crate::autoimport::RegistryCloneHost::fs(self.host)
                .use_case_sensitive_file_names(),
            current_directory: self.host.get_current_directory(),
        };

        // If any open files are in scope of this directory but not in scope of any package.json,
        // we need to add all packages in this node_modules directory.
        for path in change.open_files.keys() {
            if tspath::contains_path(&dir_path, path, &compare_options)
                && self
                    .get_nearest_ancestor_directory_with_package_json(path.clone())
                    .is_none()
            {
                return None;
            }
        }

        // Get all package.jsons that have this node_modules directory in their spine
        let mut dependencies = collections::Set::default();
        for (directory_path, directory) in self.directories.build() {
            if !tspath::contains_path(&dir_path, &directory_path, &compare_options) {
                continue;
            }
            let Some(package_json) = directory.package_json.as_ref() else {
                continue;
            };
            let Some(contents) = package_json.get_contents() else {
                continue;
            };
            add_package_json_dependencies(contents, &mut dependencies);
        }

        // Add packages that are directly imported by programs but not listed in package.json.
        // This ensures node_modules files are always in node_modules buckets.
        // Include packages from all projects that have this node_modules directory in their spine.
        for resolved_package_names in all_resolved_package_names.values() {
            for name in resolved_package_names.keys().into_iter().flatten() {
                dependencies.add(name.clone());
            }
        }

        Some(dependencies)
    }

    pub fn discover_bucket_packages(
        &self,
        package_names: &collections::Set<String>,
        dir_name: &str,
        dir_path: tspath::Path,
    ) -> Vec<DiscoveredPackage> {
        let mut result = Vec::with_capacity(package_names.len());
        let compare_options = tspath::ComparePathsOptions {
            use_case_sensitive_file_names: crate::autoimport::RegistryCloneHost::fs(self.host)
                .use_case_sensitive_file_names(),
            current_directory: String::new(),
        };
        for package_name in package_names.keys().into_iter().flatten() {
            let types_package_name = module::get_types_package_name(package_name);
            let package_json = self.host.get_package_json(&tspath::combine_paths(
                dir_name,
                &["node_modules", package_name, "package.json"],
            ));
            let types_package_json = (package_name != &types_package_name)
                .then(|| {
                    self.host.get_package_json(&tspath::combine_paths(
                        dir_name,
                        &["node_modules", &types_package_name, "package.json"],
                    ))
                })
                .flatten()
                .filter(|info| info.directory_exists);
            let realpath = package_json
                .as_ref()
                .filter(|info| info.directory_exists)
                .map(|info| {
                    crate::autoimport::RegistryCloneHost::fs(self.host)
                        .realpath(&info.package_directory)
                })
                .unwrap_or_default();
            let types_realpath = types_package_json
                .as_ref()
                .map(|info| {
                    crate::autoimport::RegistryCloneHost::fs(self.host)
                        .realpath(&info.package_directory)
                })
                .unwrap_or_default();
            let is_local = !realpath.is_empty()
                && !realpath.contains("/node_modules/")
                && tspath::contains_path(
                    &self.host.get_current_directory(),
                    &realpath,
                    &compare_options,
                );
            result.push(DiscoveredPackage {
                package_name: package_name.to_string(),
                package_json,
                realpath,
                types_package_json,
                types_realpath,
                dir_path: dir_path.clone(),
                is_local,
            });
        }
        result
    }

    pub fn extract_package(
        &self,
        _ctx: core::Context,
        package_json: &packagejson::InfoCacheEntry,
        package_name: &str,
        project_reference_outputs: HashMap<tspath::Path, String>,
        file_exclude_patterns: Option<&vfs::vfsmatch::SpecMatcher>,
        enable_directory_search: bool,
    ) -> Option<PerPackageExtractionResult> {
        if !package_json.directory_exists {
            return None;
        }
        let (to_realpath, to_symlink) = crate::autoimport::get_package_realpath_funcs(
            crate::autoimport::RegistryCloneHost::fs(self.host),
            &package_json.package_directory,
        );
        let resolver = crate::autoimport::get_module_resolver(
            self.host,
            to_realpath,
            self.resolver_options.clone(),
        );
        let mut package_entrypoints = resolver.get_entrypoints_from_package_json_info(
            package_json,
            package_name,
            enable_directory_search,
        );
        if package_entrypoints.is_empty() {
            return None;
        }

        let mut skipped_entrypoints = 0;
        if let Some(file_exclude_patterns) = file_exclude_patterns {
            let count = package_entrypoints.len();
            package_entrypoints.retain(|entrypoint| {
                !file_exclude_patterns.match_string(&entrypoint.resolved_file_name)
            });
            skipped_entrypoints = count.saturating_sub(package_entrypoints.len()) as i32;
        }
        if package_entrypoints.is_empty() {
            return None;
        }

        let mut result = PerPackageExtractionResult {
            entrypoints: package_entrypoints.clone(),
            skipped_entrypoints,
            ..Default::default()
        };

        // Resolve entrypoint source files.
        let mut seen_files = collections::new_set_with_size_hint(package_entrypoints.len());
        let mut root_files = Vec::with_capacity(package_entrypoints.len());
        let mut symlinks = HashMap::<tspath::Path, PathAndFileName>::new();
        let Some(to_path) = self.base.to_path.as_ref() else {
            return None;
        };
        for entrypoint in &package_entrypoints {
            let mut file_name = entrypoint.symlink_or_realpath().to_string();
            let mut realpath_file_name = entrypoint.resolved_file_name.clone();
            let mut realpath_path = to_path(realpath_file_name.clone());

            if let Some(input_file_name) = project_reference_outputs.get(&realpath_path) {
                file_name = to_symlink(input_file_name);
                realpath_file_name = input_file_name.clone();
                realpath_path = to_path(realpath_file_name.clone());
            }

            if !seen_files.add_if_absent(realpath_path.clone()) {
                continue;
            }
            if file_name != realpath_file_name {
                let symlink_path = to_path(file_name.clone());
                symlinks.insert(
                    realpath_path.clone(),
                    PathAndFileName {
                        path: symlink_path,
                        file_name,
                    },
                );
                result.is_symlinked = true;
            }
            if let Some(file) = self
                .host
                .get_source_file(&realpath_file_name, realpath_path)
            {
                root_files.push(file);
            }
        }

        let to_path_for_extractor = self.base.to_path.as_ref().map(|to_path| {
            let to_path = to_path.clone();
            Box::new(move |file_name: String| to_path(file_name))
                as Box<dyn Fn(String) -> tspath::Path + Send + Sync>
        });
        let (to_realpath, _) = crate::autoimport::get_package_realpath_funcs(
            crate::autoimport::RegistryCloneHost::fs(self.host),
            &package_json.package_directory,
        );
        let extractor_resolver = crate::autoimport::get_module_resolver(
            self.host,
            to_realpath,
            self.resolver_options.clone(),
        );
        let (to_realpath, _) = crate::autoimport::get_package_realpath_funcs(
            crate::autoimport::RegistryCloneHost::fs(self.host),
            &package_json.package_directory,
        );
        let to_path_for_alias_resolver = to_path.clone();
        let alias_resolver = crate::autoimport::new_alias_resolver(
            root_files,
            symlinks,
            self.host,
            resolver,
            move |file_name| to_path_for_alias_resolver(file_name.to_string()),
            |_source, _module_name| {},
        );
        let mut semantic_state = checker::CheckerState::new_for_slot_index(0);
        let mut checker =
            checker::Checker::new_checker_with_state(&alias_resolver, None, &mut semantic_state);
        let mut extractor = crate::autoimport::new_export_extractor(
            package_name.to_string(),
            &mut checker,
            extractor_resolver,
            to_path_for_extractor,
            Some(Box::new(move |file_name| to_realpath(&file_name))),
        );

        let mut non_module_files = collections::Set::default();
        for entrypoint in &alias_resolver.root_files {
            let file_exports = extractor.extract_from_file(entrypoint);
            for name in entrypoint.ambient_module_names() {
                result
                    .ambient_modules
                    .entry(name.clone())
                    .or_default()
                    .push(entrypoint.file_name());
            }
            result
                .package_files
                .insert(entrypoint.path(), entrypoint.file_name());
            let symlink = alias_resolver.symlinks.get(&entrypoint.path());
            if let Some(symlink) = symlink {
                result
                    .package_files
                    .insert(symlink.path.clone(), symlink.file_name.clone());
            }

            let has_exports =
                !file_exports.is_empty() && entrypoint.external_module_indicator().is_some();
            result.exports.insert(entrypoint.path(), file_exports);
            if !has_exports {
                non_module_files.add(entrypoint.path());
                if let Some(symlink) = symlink {
                    non_module_files.add(symlink.path.clone());
                }
            }
        }

        // Discard entrypoints for non-module files and empty modules.
        result.entrypoints.retain(|entrypoint| {
            let path = to_path(entrypoint.resolved_file_name.clone());
            !non_module_files.has(&path)
        });

        result.stats_exports = extractor
            .stats()
            .exports
            .load(std::sync::atomic::Ordering::Relaxed);
        result.stats_used_checker = extractor
            .stats()
            .used_checker
            .load(std::sync::atomic::Ordering::Relaxed);
        Some(result)
    }

    pub fn build_node_modules_bucket(
        &mut self,
        _ctx: core::Context,
        result: &mut RegistryBuildResult,
        dependencies: Option<collections::Set<String>>,
        dir_path: tspath::Path,
        discovered: Vec<DiscoveredPackage>,
        directory_package_names: Option<collections::Set<String>>,
        extraction_cache: HashMap<String, PerPackageExtractionResult>,
        recursive_search_packages: Option<collections::Set<String>>,
        _logger: Option<&logging::LogTree>,
    ) {
        let extraction = install_extractions(discovered, extraction_cache);

        // Build PackageFiles with all directory package names; indexed packages have
        // non-nil maps, unindexed packages have nil maps.
        let mut all_package_files = HashMap::new();
        if let Some(directory_package_names) = directory_package_names.as_ref() {
            for package_name in directory_package_names.keys().into_iter().flatten() {
                all_package_files.insert(
                    package_name.clone(),
                    extraction
                        .package_files
                        .get(package_name)
                        .cloned()
                        .unwrap_or_default(),
                );
            }
        } else {
            all_package_files = extraction.package_files.clone();
        }

        // Build Paths as reverse mapping from path to package name.
        // Only include paths for local workspace packages (eligible for granular updates).
        let mut paths = HashMap::new();
        for package_name in extraction.workspace_packages.keys().into_iter().flatten() {
            if let Some(files) = extraction.package_files.get(package_name) {
                for path in files.keys() {
                    paths.insert(path.clone(), package_name.clone());
                }
            }
        }

        let mut bucket = RegistryBucket {
            index: Some(Index::default()),
            dependency_names: dependencies,
            package_files: all_package_files,
            ambient_module_names: extraction.ambient_module_names,
            paths,
            state: BucketState {
                build_preferences: bucket_build_preferences_from_user_preferences(
                    self.user_preferences.clone(),
                ),
                recursive_search_packages,
                ..Default::default()
            },
            ..Default::default()
        };

        for file_exports in extraction.exports.values() {
            for export in file_exports {
                if let Some(index) = bucket.index.as_mut() {
                    index.insert_as_words(export.clone());
                }
            }
        }

        let to_path = self.base.to_path.as_ref();
        for entrypoint_set in &extraction.entrypoints {
            for entrypoint in entrypoint_set {
                let file_name = if entrypoint.resolved_file_name.is_empty() {
                    entrypoint.file_name.clone()
                } else {
                    entrypoint.resolved_file_name.clone()
                };
                let path = to_path
                    .map(|to_path| to_path(file_name.clone()))
                    .unwrap_or(file_name);
                result
                    .entrypoints
                    .entry(path)
                    .or_default()
                    .push(entrypoint.clone());
            }
        }

        // Compute old entrypoint paths to remove from the registry-level map.
        // For a full rebuild, all entrypoints belonging to the old bucket's packages must be removed.
        if let Some(old_bucket) = self.base.node_modules.get(&dir_path) {
            for files in old_bucket.package_files.values() {
                for path in files.keys() {
                    if self.base.entrypoints.contains_key(path) {
                        result.removed_entrypoint_paths.push(path.clone());
                    }
                }
            }
        }

        result.bucket = Some(bucket);
        result.possible_failed_ambient_module_lookup_sources =
            extraction.possible_failed_ambient_module_lookup_sources;
        result.possible_failed_ambient_module_lookup_targets =
            extraction.possible_failed_ambient_module_lookup_targets;
    }

    pub fn update_node_modules_bucket(
        &mut self,
        _ctx: core::Context,
        result: &mut RegistryBuildResult,
        existing_bucket: &RegistryBucket,
        dirty_packages: &collections::Set<String>,
        discovered: Vec<DiscoveredPackage>,
        extraction_cache: HashMap<String, PerPackageExtractionResult>,
        recursive_search_packages: Option<collections::Set<String>>,
        _logger: Option<&logging::LogTree>,
    ) {
        let extraction = install_extractions(discovered, extraction_cache);

        // Clone the existing index, excluding exports from dirty packages
        let mut new_index = existing_bucket
            .index
            .as_ref()
            .and_then(|index| {
                index.clone_filtered(|export| !dirty_packages.has(&export.package_name))
            })
            .unwrap_or_default();

        // Clone PackageFiles, removing dirty packages
        let mut new_package_files = existing_bucket.package_files.clone();
        for package_name in dirty_packages.keys().into_iter().flatten() {
            new_package_files.remove(package_name);
        }
        // Add newly extracted package files
        new_package_files.extend(extraction.package_files.clone());

        // Clone Paths, removing dirty package paths
        let mut new_paths = HashMap::with_capacity(existing_bucket.paths.len());
        for (path, package_name) in &existing_bucket.paths {
            if dirty_packages.has(package_name) {
                continue;
            }
            new_paths.insert(path.clone(), package_name.clone());
        }
        // Add paths for newly extracted workspace packages
        for package_name in extraction.workspace_packages.keys().into_iter().flatten() {
            if let Some(files) = extraction.package_files.get(package_name) {
                for path in files.keys() {
                    new_paths.insert(path.clone(), package_name.clone());
                }
            }
        }

        // Clone AmbientModuleNames, removing dirty package entries
        let to_path = self.base.to_path.as_ref();
        let mut new_ambient_module_names =
            HashMap::with_capacity(existing_bucket.ambient_module_names.len());
        for (module_name, file_names) in &existing_bucket.ambient_module_names {
            let mut filtered = Vec::new();
            for file_name in file_names {
                let path = to_path
                    .map(|to_path| to_path(file_name.clone()))
                    .unwrap_or_else(|| file_name.clone());
                if existing_bucket
                    .paths
                    .get(&path)
                    .is_some_and(|package_name| dirty_packages.has(package_name))
                {
                    continue;
                }
                filtered.push(file_name.clone());
            }
            if !filtered.is_empty() {
                new_ambient_module_names.insert(module_name.clone(), filtered);
            }
        }
        // Add newly extracted ambient module names
        for (module_name, file_names) in &extraction.ambient_module_names {
            new_ambient_module_names
                .entry(module_name.clone())
                .or_default()
                .extend(file_names.clone());
        }

        // Collect entrypoint paths that need to be removed from the registry-level map
        // (paths belonging to dirty packages)
        let mut removed_entrypoint_paths = Vec::new();
        for path in self.base.entrypoints.keys() {
            if existing_bucket
                .paths
                .get(path)
                .is_some_and(|package_name| dirty_packages.has(package_name))
            {
                removed_entrypoint_paths.push(path.clone());
            }
        }
        // Build new entrypoints from extraction
        let mut new_entrypoints = HashMap::new();
        for entrypoint_set in &extraction.entrypoints {
            for entrypoint in entrypoint_set {
                let file_name = if entrypoint.resolved_file_name.is_empty() {
                    entrypoint.file_name.clone()
                } else {
                    entrypoint.resolved_file_name.clone()
                };
                let path = to_path
                    .map(|to_path| to_path(file_name.clone()))
                    .unwrap_or(file_name);
                new_entrypoints
                    .entry(path)
                    .or_insert_with(Vec::new)
                    .push(entrypoint.clone());
            }
        }

        // Insert newly extracted exports into the index
        for file_exports in extraction.exports.values() {
            for export in file_exports {
                new_index.insert_as_words(export.clone());
            }
        }

        result.bucket = Some(RegistryBucket {
            index: Some(new_index),
            dependency_names: existing_bucket.dependency_names.clone(),
            package_files: new_package_files,
            ambient_module_names: new_ambient_module_names,
            paths: new_paths,
            state: BucketState {
                build_preferences: bucket_build_preferences_from_user_preferences(
                    self.user_preferences.clone(),
                ),
                recursive_search_packages,
                ..Default::default()
            },
            ..Default::default()
        });
        result.entrypoints = new_entrypoints;
        result.removed_entrypoint_paths = removed_entrypoint_paths;
        result.possible_failed_ambient_module_lookup_sources =
            extraction.possible_failed_ambient_module_lookup_sources;
        result.possible_failed_ambient_module_lookup_targets =
            extraction.possible_failed_ambient_module_lookup_targets;
    }

    pub fn get_nearest_ancestor_directory_with_package_json(
        &self,
        file_path: tspath::Path,
    ) -> Option<Directory> {
        let directories = self.directories.build();
        tspath::for_each_ancestor_directory_path(
            tspath::get_directory_path(&file_path),
            |dir_path| {
                directories
                    .get(dir_path)
                    .filter(|dir| dir.package_json.is_some())
                    .cloned()
            },
        )
    }

    pub fn resolve_ambient_module_name(
        &self,
        module_name: &str,
        from_path: tspath::Path,
    ) -> Vec<String> {
        tspath::for_each_ancestor_directory_path(from_path, |dir_path| {
            self.base
                .node_modules
                .get(dir_path)
                .and_then(|bucket| bucket.ambient_module_names.get(module_name))
                .cloned()
        })
        .unwrap_or_default()
    }
}

pub fn has_new_non_node_modules_files(
    program: &compiler::Program,
    bucket: &RegistryBucket,
) -> bool {
    if bucket.state.new_program_structure != NEW_PROGRAM_STRUCTURE_DIFFERENT_FILE_NAMES {
        return false;
    }
    for file in program.source_files_for_auto_imports() {
        if file.file_name().contains("/node_modules/") || is_ignored_file(program, &file) {
            continue;
        }
        if !bucket.paths.contains_key(&file.path()) {
            return true;
        }
    }
    false
}

pub fn is_ignored_file(program: &compiler::Program, file: &ast::SourceFile) -> bool {
    program.is_source_file_default_library_for_auto_imports(file.path())
        || program.is_global_typings_file_for_auto_imports(&file.file_name())
}

// hasSymlinkToNodeModules checks if a file's realpath has a symlink that points
// to a node_modules directory. This is used to skip files in the project bucket
// that would be duplicated by the node_modules bucket via their symlink.
pub fn has_symlink_to_node_modules(
    file_path: tspath::Path,
    symlink_cache: &symlinks::KnownSymlinks,
) -> bool {
    // First check if the file itself has a symlink to node_modules
    if symlink_cache
        .files_by_realpath()
        .get(&file_path)
        .is_some_and(|symlink_paths| {
            symlink_paths
                .iter()
                .any(|symlink_path| symlink_path.contains("/node_modules/"))
        })
    {
        return true;
    }

    // Fall back to checking ancestor directories
    tspath::for_each_ancestor_directory_path(file_path, |dir_path| {
        let dir_path = tspath::ensure_trailing_directory_separator(dir_path);
        symlink_cache
            .directories_by_realpath()
            .get(&dir_path)
            .is_some_and(|symlink_paths| {
                symlink_paths
                    .iter()
                    .any(|symlink_path| symlink_path.contains("/node_modules/"))
            })
            .then_some(())
    })
    .is_some()
}

#[derive(Debug, Default)]
pub struct FailedAmbientModuleLookupSource {
    pub mu: Mutex<()>,
    pub file_name: String,
    pub package_name: String,
}

impl Clone for FailedAmbientModuleLookupSource {
    fn clone(&self) -> Self {
        Self {
            mu: Mutex::new(()),
            file_name: self.file_name.clone(),
            package_name: self.package_name.clone(),
        }
    }
}

#[derive(Default)]
pub struct RegistryBuildResult {
    pub entry_key: tspath::Path,
    pub err: Option<String>,
    pub bucket: Option<RegistryBucket>,
    pub entrypoints: HashMap<tspath::Path, Vec<module::ResolvedEntrypoint>>,
    pub removed_entrypoint_paths: Vec<tspath::Path>,
    pub possible_failed_ambient_module_lookup_sources:
        collections::SyncMap<tspath::Path, FailedAmbientModuleLookupSource>,
    pub possible_failed_ambient_module_lookup_targets: collections::Set<String>,
}

#[derive(Clone, Debug, Default)]
pub struct DiscoveredPackage {
    pub package_name: String,
    pub package_json: Option<packagejson::InfoCacheEntry>,
    pub realpath: String,
    pub types_package_json: Option<packagejson::InfoCacheEntry>,
    pub types_realpath: String,
    pub dir_path: tspath::Path,
    pub is_local: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PerPackageExtractionResult {
    pub package_files: HashMap<tspath::Path, String>,
    pub entrypoints: Vec<module::ResolvedEntrypoint>,
    pub exports: HashMap<tspath::Path, Vec<Export>>,
    pub ambient_modules: HashMap<String, Vec<String>>,
    pub stats_exports: i32,
    pub stats_used_checker: i32,
    pub skipped_entrypoints: i32,
    pub is_symlinked: bool,
    pub failed_ambient_module_lookup_sources:
        HashMap<tspath::Path, FailedAmbientModuleLookupSource>,
    pub failed_ambient_module_lookup_targets: collections::Set<String>,
}

#[derive(Default)]
pub struct PackageExtractionResult {
    pub exports: HashMap<tspath::Path, Vec<Export>>,
    pub package_files: HashMap<String, HashMap<tspath::Path, String>>,
    pub ambient_module_names: HashMap<String, Vec<String>>,
    pub entrypoints: Vec<Vec<module::ResolvedEntrypoint>>,
    pub workspace_packages: collections::Set<String>,
    pub possible_failed_ambient_module_lookup_sources:
        collections::SyncMap<tspath::Path, FailedAmbientModuleLookupSource>,
    pub possible_failed_ambient_module_lookup_targets: collections::Set<String>,
    pub stats_exports: i32,
    pub stats_used_checker: i32,
    pub skipped_entrypoints_count: i32,
}

pub fn install_extractions(
    discovered: Vec<DiscoveredPackage>,
    extraction_cache: HashMap<String, PerPackageExtractionResult>,
) -> PackageExtractionResult {
    let mut result = PackageExtractionResult::default();

    for package in discovered {
        let extraction = extraction_cache
            .get(&package.realpath)
            .or_else(|| extraction_cache.get(&package.types_realpath));
        let Some(extraction) = extraction else {
            continue;
        };

        for (path, exports) in &extraction.exports {
            result.exports.insert(path.clone(), exports.clone());
        }

        result
            .package_files
            .entry(package.package_name.clone())
            .or_insert_with(|| HashMap::with_capacity(extraction.package_files.len()))
            .extend(extraction.package_files.clone());

        for (name, file_names) in &extraction.ambient_modules {
            result
                .ambient_module_names
                .entry(name.clone())
                .or_default()
                .extend(file_names.clone());
        }

        if !extraction.entrypoints.is_empty() {
            result.entrypoints.push(extraction.entrypoints.clone());
        }

        for (path, source) in &extraction.failed_ambient_module_lookup_sources {
            let _ = result
                .possible_failed_ambient_module_lookup_sources
                .load_or_store(path.clone(), Some(source.clone()));
        }

        for target in extraction
            .failed_ambient_module_lookup_targets
            .keys()
            .into_iter()
            .flatten()
        {
            result
                .possible_failed_ambient_module_lookup_targets
                .add(target.clone());
        }

        if extraction.is_symlinked && package.is_local {
            result.workspace_packages.add(package.package_name.clone());
        }
        result.stats_exports += extraction.stats_exports;
        result.stats_used_checker += extraction.stats_used_checker;
        result.skipped_entrypoints_count += extraction.skipped_entrypoints;
    }

    result
}
