use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use ts_core as core;
use ts_lsproto::DocumentUriExt;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;
use ts_vfs::{self as vfs, Fs as _};

use crate::configfileregistry::{
    ConfigFileEntry, ConfigFileNames, ConfigFileRegistry, new_config_file_entry,
    new_extended_config_file_entry,
};
use crate::dirty::SyncMapEntryHandle;
use crate::logging::{LogTree, Logger};
use crate::{
    ExtendedConfigCache, ExtendedConfigParseArgs, FileChangeSummary, MIN_WATCH_LOCATION_DEPTH,
    PatternsAndIgnored, PendingReload, ProjectLoadKind, SessionOptions, SnapshotFsBuilder,
    SourceFs, dirty, get_recursive_glob_pattern, new_source_fs, parse_extended_config_cache_entry,
};

// configFileRegistryBuilder tracks changes made on top of a previous
// configFileRegistry, producing a new clone with `finalize()` after
// all changes have been made.
pub struct ConfigFileRegistryBuilder {
    pub has_relative_pattern_capability: bool,
    pub fs: SourceFs,
    pub is_open_file: Arc<dyn Fn(tspath::Path) -> bool + Send + Sync>,
    pub extended_config_cache: ExtendedConfigCache,
    pub snapshot_id: u64,
    pub session_options: SessionOptions,
    pub custom_config_file_name: String,

    pub base: ConfigFileRegistry,
    pub configs: dirty::SyncMap<tspath::Path, ConfigFileEntry>,
    pub config_file_names: dirty::Map<tspath::Path, ConfigFileNames>,
    pub custom_config_file_name_changed: bool,
}

pub fn new_config_file_registry_builder(
    has_relative_pattern_capability: bool,
    fs: SnapshotFsBuilder,
    old_config_file_registry: ConfigFileRegistry,
    extended_config_cache: ExtendedConfigCache,
    snapshot_id: u64,
    session_options: SessionOptions,
    custom_config_file_name: String,
    _logger: Option<&LogTree>,
) -> ConfigFileRegistryBuilder {
    let custom_config_file_name_changed =
        custom_config_file_name != old_config_file_registry.custom_config_file_name;

    ConfigFileRegistryBuilder {
        has_relative_pattern_capability,
        fs: new_source_fs(false, fs.clone(), fs.to_path.clone()),
        is_open_file: Arc::new(move |path| fs.is_open_file(&path)),
        base: old_config_file_registry.clone_registry(),
        session_options,
        extended_config_cache,
        snapshot_id,
        custom_config_file_name,
        custom_config_file_name_changed,
        configs: dirty::new_sync_map(old_config_file_registry.configs),
        config_file_names: dirty::new_map(old_config_file_registry.config_file_names),
    }
}

impl ConfigFileRegistryBuilder {
    pub fn fs(&self) -> vfs::FS {
        self.fs.fs()
    }

    // Finalize creates a new configFileRegistry based on the changes made in the builder.
    // If no changes were made, it returns the original base registry.
    pub fn finalize(&self) -> ConfigFileRegistry {
        let mut changed = false;
        let mut new_registry = self.base.clone_registry();

        let (configs, changed_configs) = self.configs.finalize();
        if changed_configs {
            if !changed {
                new_registry = new_registry.clone_registry();
                changed = true;
            }
            new_registry.configs = configs;
        }

        let (config_file_names, changed_names) = self.config_file_names.finalize();
        if changed_names {
            if !changed {
                new_registry = new_registry.clone_registry();
                changed = true;
            }
            new_registry.config_file_names = config_file_names;
        }

        if self.custom_config_file_name_changed {
            if !changed {
                new_registry = new_registry.clone_registry();
            }
            new_registry.custom_config_file_name = self.custom_config_file_name.clone();
        }

        new_registry
    }

    pub fn find_or_acquire_config_for_file(
        &self,
        config_file_name: &str,
        config_file_path: tspath::Path,
        file_path: tspath::Path,
        load_kind: ProjectLoadKind,
        logger: &LogTree,
    ) -> Option<tsoptions::ParsedCommandLine> {
        match load_kind {
            ProjectLoadKind::Find => {
                if let Some(entry) = self.configs.load(config_file_path.clone()) {
                    return entry.value().command_line.clone();
                }
                None
            }
            ProjectLoadKind::Create => {
                self.acquire_config_for_file(config_file_name, config_file_path, file_path, logger)
            }
            #[expect(
                unreachable_patterns,
                reason = "matches TypeScript-Go default panic for future load kinds"
            )]
            _ => panic!("unknown project load kind: {:?}", load_kind),
        }
    }

    // reloadIfNeeded updates the command line of the config file entry based on its
    // pending reload state. This function should only be called from within the
    // Change() method of a dirty map entry.
    pub fn reload_if_needed(
        &self,
        entry: &mut ConfigFileEntry,
        file_name: &str,
        path: tspath::Path,
        logger: &LogTree,
    ) {
        match entry.pending_reload {
            PendingReload::FileNames => {
                logger.logf(format!("Reloading file names for config: {file_name}"));
                entry.command_line = entry.command_line.clone().map(|command_line| {
                    command_line.reload_file_names_of_parsed_command_line(&self.fs)
                });
            }
            PendingReload::Full => {
                logger.logf(format!("Loading config file: {file_name}"));
                let old_command_line = entry.command_line.clone();
                entry.command_line = tsoptions::get_parsed_command_line_of_config_file_path(
                    file_name,
                    path.clone(),
                    None,
                    None,
                    self,
                    Some(self),
                )
                .0;
                if let Some(command_line) = &mut entry.command_line {
                    self.dedupe_extended_source_files(command_line);
                }
                self.update_extending_configs(path, entry.command_line.clone(), old_command_line);
                if entry.command_line.is_some() {
                    self.update_root_files_watch(file_name, entry);
                }
                logger.logf("Finished loading config file".to_string());
            }
            PendingReload::None => return,
        }
        entry.pending_reload = PendingReload::None;
    }

    fn dedupe_extended_source_files(&self, command_line: &mut tsoptions::ParsedCommandLine) {
        let Some(config_file) = &mut command_line.config_file else {
            return;
        };
        let mut seen = HashSet::new();
        config_file
            .extended_source_files
            .retain(|file_name| seen.insert(self.fs.to_path(file_name)));
    }

    pub fn update_extending_configs(
        &self,
        extending_config_path: tspath::Path,
        new_command_line: Option<tsoptions::ParsedCommandLine>,
        old_command_line: Option<tsoptions::ParsedCommandLine>,
    ) {
        let mut new_extended_config_paths = ts_collections::Set::<tspath::Path>::default();
        if let Some(new_command_line) = &new_command_line {
            for extended_config in new_command_line.extended_source_files() {
                let extended_config_path = self.fs.to_path(extended_config);
                new_extended_config_paths.add(extended_config_path.clone());
                let (entry, loaded) = self.configs.load_or_store(
                    extended_config_path.clone(),
                    self.configs.new_entry(
                        extended_config_path.clone(),
                        new_extended_config_file_entry(
                            extended_config.clone(),
                            extending_config_path.clone(),
                        ),
                    ),
                );
                if loaded {
                    entry.change_if(
                        |config| !config.retaining_configs.contains(&extending_config_path),
                        |config| {
                            config
                                .retaining_configs
                                .insert(extending_config_path.clone());
                        },
                    );
                }
            }
        }
        if let Some(old_command_line) = &old_command_line {
            for extended_config in old_command_line.extended_source_files() {
                let extended_config_path = self.fs.to_path(extended_config);
                if new_extended_config_paths.has(&extended_config_path) {
                    continue;
                }
                if let Some(entry) = self.configs.load(extended_config_path) {
                    entry.change_if(
                        |config| config.retaining_configs.contains(&extending_config_path),
                        |config| {
                            config.retaining_configs.remove(&extending_config_path);
                        },
                    );
                }
            }
        }
    }

    pub fn update_root_files_watch(&self, file_name: &str, entry: &mut ConfigFileEntry) {
        if entry.root_files_watch.is_none() {
            return;
        }

        let mut ignored: HashSet<String> = HashSet::new();
        let mut globs = Vec::new();
        let mut external_directories = Vec::new();
        let mut include_workspace = false;
        let mut include_tsconfig_dir = false;
        let tsconfig_dir = tspath::get_directory_path(file_name);
        let wildcard_directories: Vec<_> = entry
            .command_line
            .as_mut()
            .unwrap()
            .wildcard_directories()
            .keys()
            .cloned()
            .collect();
        let compare_paths_options = tspath::ComparePathsOptions {
            current_directory: self.session_options.current_directory.clone(),
            use_case_sensitive_file_names: self.fs().use_case_sensitive_file_names(),
            ..Default::default()
        };
        for dir in &wildcard_directories {
            if tspath::contains_path(
                &self.session_options.current_directory,
                dir,
                &compare_paths_options,
            ) {
                include_workspace = true;
            } else if tspath::contains_path(&tsconfig_dir, dir, &compare_paths_options) {
                include_tsconfig_dir = true;
            } else {
                external_directories.push(dir.clone());
            }
        }
        for file_name in entry.command_line.as_ref().unwrap().literal_file_names() {
            if tspath::contains_path(
                &self.session_options.current_directory,
                &file_name,
                &compare_paths_options,
            ) {
                include_workspace = true;
            } else if tspath::contains_path(&tsconfig_dir, &file_name, &compare_paths_options) {
                include_tsconfig_dir = true;
            } else {
                external_directories.push(tspath::get_directory_path(&file_name));
            }
        }

        if include_workspace {
            globs.push(get_recursive_glob_pattern(
                &self.session_options.current_directory,
            ));
        }
        if include_tsconfig_dir {
            globs.push(get_recursive_glob_pattern(&tsconfig_dir));
        }
        for file_name in entry.command_line.as_ref().unwrap().extended_source_files() {
            if include_workspace
                && tspath::contains_path(
                    &self.session_options.current_directory,
                    &file_name,
                    &compare_paths_options,
                )
            {
                continue;
            }
            globs.push(file_name.clone());
        }
        if !external_directories.is_empty() {
            let (common_parents, ignored_external_dirs) = tspath::get_common_parents(
                &external_directories,
                MIN_WATCH_LOCATION_DEPTH,
                &compare_paths_options,
            );
            for parent in common_parents {
                globs.push(get_recursive_glob_pattern(&parent));
            }
            ignored = ignored_external_dirs.into_iter().collect();
        }

        globs.sort();
        entry.root_files_watch = entry.root_files_watch.clone().map(|watch| {
            watch.clone_with(PatternsAndIgnored {
                directories_outside_workspace: Vec::new(),
                patterns_inside_workspace: globs,
                ignored,
            })
        });
    }

    // acquireConfigForProject loads a config file entry from the cache, or parses it if not already
    // cached, then adds the project (if provided) to `retainingProjects` to keep it alive
    // in the cache. Each `acquireConfigForProject` call that passes a `project` should be accompanied
    // by an eventual `releaseConfigForProject` call with the same project.
    pub fn acquire_config_for_project(
        &self,
        file_name: &str,
        path: tspath::Path,
        project_path: &tspath::Path,
        logger: &LogTree,
    ) -> Option<tsoptions::ParsedCommandLine> {
        let (entry, _) = self.configs.load_or_store(
            path.clone(),
            self.configs.new_entry(
                path.clone(),
                new_config_file_entry(self.has_relative_pattern_capability, file_name.to_string()),
            ),
        );
        entry.change_if(
            |config| {
                !config.retaining_projects.contains(project_path)
                    || config.pending_reload != PendingReload::None
            },
            |config| {
                config.retaining_projects.insert(project_path.clone());
                self.reload_if_needed(config, file_name, path.clone(), logger);
            },
        );
        entry.value().command_line.clone()
    }

    // acquireConfigForFile loads a config file entry from the cache, or parses it if not already
    // cached, then adds the open file to `retainingOpenFiles` to keep it alive in the cache.
    // Each `acquireConfigForFile` call that passes an `openFilePath`
    // should be accompanied by an eventual `releaseConfigForOpenFile` call with the same open file.
    pub fn acquire_config_for_file(
        &self,
        config_file_name: &str,
        config_file_path: tspath::Path,
        file_path: tspath::Path,
        logger: &LogTree,
    ) -> Option<tsoptions::ParsedCommandLine> {
        let (entry, _) = self.configs.load_or_store(
            config_file_path.clone(),
            self.configs.new_entry(
                config_file_path.clone(),
                new_config_file_entry(
                    self.has_relative_pattern_capability,
                    config_file_name.to_string(),
                ),
            ),
        );
        entry.change_if(
            |config| {
                ((self.is_open_file)(file_path.clone())
                    && !config.retaining_open_files.contains(&file_path))
                    || config.pending_reload != PendingReload::None
            },
            |config| {
                if (self.is_open_file)(file_path.clone()) {
                    config.retaining_open_files.insert(file_path.clone());
                }
                self.reload_if_needed(config, config_file_name, config_file_path.clone(), logger);
            },
        );
        entry.value().command_line.clone()
    }

    // releaseConfigForProject removes the project from the config entry. Once no projects
    // or files are associated with the config entry, it will be removed on the next call to `cleanup`.
    pub fn release_config_for_project(
        &self,
        config_file_path: tspath::Path,
        project_path: tspath::Path,
    ) {
        if let Some(entry) = self.configs.load(config_file_path) {
            entry.change_if(
                |config| config.retaining_projects.contains(&project_path),
                |config| {
                    config.retaining_projects.remove(&project_path);
                },
            );
        }
    }

    // didCloseFile removes the open file from the config entry. Once no projects
    // or files are associated with the config entry, it will be removed on the next call to `cleanup`.
    pub fn did_close_file(&mut self, path: tspath::Path) {
        if tspath::is_dynamic_file_name(&path.to_string()) {
            return;
        }
        self.config_file_names.delete(path.clone());
        self.configs.range_(|_, entry| {
            entry.change_if(
                |config| config.retaining_open_files.contains(&path),
                |config| {
                    config.retaining_open_files.remove(&path);
                },
            );
            true
        });
    }

    pub fn did_change_custom_config_file_name(&mut self, logger: &LogTree) -> bool {
        let _ = logger;
        if !self.custom_config_file_name_changed {
            return false;
        }
        self.config_file_names.clear();
        true
    }

    pub fn invalidate_cache(&mut self, logger: &LogTree) -> ChangeFileResult {
        let mut affected_projects: Option<HashMap<tspath::Path, ()>> = None;
        let mut affected_files: Option<HashMap<tspath::Path, ()>> = None;

        logger.logf("Too many files changed; marking all configs for reload".to_string());
        self.config_file_names.range_(|entry| {
            if affected_files.is_none() {
                affected_files = Some(HashMap::new());
            }
            affected_files
                .as_mut()
                .unwrap()
                .insert(entry.key().clone(), ());
            true
        });
        self.config_file_names.clear();

        self.configs.range_(|_, entry| {
            entry.change(|config| {
                let mut next = affected_projects.take().unwrap_or_default();
                next.extend(config.retaining_projects.iter().cloned().map(|p| (p, ())));
                affected_projects = Some(next);
                if config.pending_reload != PendingReload::Full {
                    let (text, ok) = self.fs().read_file(&config.file_name);
                    if !ok
                        || text
                            != config
                                .command_line
                                .as_ref()
                                .unwrap()
                                .config_file
                                .as_ref()
                                .unwrap()
                                .source_file
                                .text()
                    {
                        config.pending_reload = PendingReload::Full;
                    } else {
                        config.pending_reload = PendingReload::FileNames;
                    }
                }
            });
            true
        });

        ChangeFileResult {
            affected_projects: affected_projects.unwrap_or_default(),
            affected_files: affected_files.unwrap_or_default(),
        }
    }

    pub fn is_config_base_name(&self, base_name: &str) -> bool {
        base_name == "tsconfig.json"
            || base_name == "jsconfig.json"
            || (!self.custom_config_file_name.is_empty()
                && base_name == self.custom_config_file_name)
    }

    pub fn did_change_files(
        &mut self,
        summary: FileChangeSummary,
        logger: &LogTree,
    ) -> ChangeFileResult {
        let mut affected_projects = None;
        let mut affected_files = None;
        let mut should_invalidate_cache = false;

        logger.logf("Summarizing file changes".to_string());
        let has_excessive_changes = summary.has_excessive_watch_events()
            && summary.includes_watch_change_outside_node_modules;
        let mut created_files = HashMap::with_capacity(summary.created.len());
        let mut deleted_files = HashMap::with_capacity(summary.deleted.len());
        let mut created_or_deleted_config_files = HashMap::new();
        let mut created_or_changed_or_deleted_files =
            HashMap::with_capacity(summary.changed.len() + summary.deleted.len());

        for uri in summary.changed.iter() {
            if tspath::contains_ignored_path(&uri.to_string()) {
                continue;
            }
            let file_name = uri.file_name();
            let path = self.fs.to_path(&file_name);
            let base_name = tspath::get_base_file_name(&path.to_string());
            if self.is_config_base_name(&base_name) {
                created_or_deleted_config_files.insert(path.clone(), ());
            }
            created_or_changed_or_deleted_files.insert(path, ());
        }
        for uri in summary.deleted.iter() {
            if tspath::contains_ignored_path(&uri.to_string()) {
                continue;
            }
            let file_name = uri.file_name();
            let path = self.fs.to_path(&file_name);
            deleted_files.insert(path.clone(), file_name.to_string());
            let base_name = tspath::get_base_file_name(&path.to_string());
            if self.is_config_base_name(&base_name) {
                created_or_deleted_config_files.insert(path.clone(), ());
            }
            created_or_changed_or_deleted_files.insert(path, ());
        }
        for uri in summary.created.iter() {
            if tspath::contains_ignored_path(&uri.to_string()) {
                continue;
            }
            let file_name = uri.file_name();
            let path = self.fs.to_path(&file_name);
            created_files.insert(path.clone(), file_name.to_string());
            let base_name = tspath::get_base_file_name(&path.to_string());
            if self.is_config_base_name(&base_name) {
                created_or_deleted_config_files.insert(path.clone(), ());
            }
            created_or_changed_or_deleted_files.insert(path, ());
        }
        // Handle closed files - this ranges over config entries and could be combined
        // with the file change handling, but a separate loop is simpler and a snapshot
        // change with both closing and watch changes seems rare.
        for uri in summary.closed.iter() {
            let file_name = uri.file_name();
            let path = self.fs.to_path(&file_name);
            self.did_close_file(path);
        }

        // Handle changes to stored config files
        logger.logf("Checking if any changed files are config files".to_string());
        for path in created_or_changed_or_deleted_files.keys() {
            if let Some(entry) = self.configs.load(path.clone()) {
                if has_excessive_changes {
                    return self.invalidate_cache(logger);
                }

                affected_projects = Some(core::copy_map_into(
                    affected_projects.take(),
                    &self.handle_config_change(&entry, logger),
                ));
                for extending_config_path in &entry.value().retaining_configs {
                    if let Some(extending_config_entry) =
                        self.configs.load(extending_config_path.clone())
                    {
                        affected_projects = Some(core::copy_map_into(
                            affected_projects.take(),
                            &self.handle_config_change(&extending_config_entry, logger),
                        ));
                    }
                }
                // This was a config file, so assume it's not also a root file
                created_files.remove(path);
            }
        }

        // Handle created/deleted files named "tsconfig.json" or "jsconfig.json"
        for path in created_or_deleted_config_files.keys() {
            if has_excessive_changes {
                return self.invalidate_cache(logger);
            }
            let directory_path = tspath::get_directory_path(path);
            let compare_paths_options = tspath::ComparePathsOptions {
                current_directory: self.session_options.current_directory.clone(),
                use_case_sensitive_file_names: self.fs().use_case_sensitive_file_names(),
                ..Default::default()
            };
            self.config_file_names.range_(|entry| {
                if tspath::contains_path(&directory_path, &entry.key(), &compare_paths_options) {
                    if affected_files.is_none() {
                        affected_files = Some(HashMap::new());
                    }
                    affected_files
                        .as_mut()
                        .unwrap()
                        .insert(entry.key().clone(), ());
                    entry.delete();
                }
                true
            });
        }

        // Handle deletions of wildcard-included root files
        for (path, file_name) in &deleted_files {
            self.configs.range_(|_, entry| {
                let entry_key = entry.key();
                entry.change_if(
                    |config| {
                        if config.pending_reload != PendingReload::None
                            || config.command_line.is_none()
                        {
                            return false;
                        }
                        if config
                            .command_line
                            .as_ref()
                            .unwrap()
                            .file_names_by_path()
                            .contains_key(path)
                        {
                            return config
                                .command_line
                                .as_ref()
                                .unwrap()
                                .get_matched_file_spec(file_name)
                                .is_empty();
                        }
                        false
                    },
                    |config| {
                        config.pending_reload = PendingReload::FileNames;
                        if affected_projects.is_none() {
                            affected_projects = Some(HashMap::new());
                        }
                        affected_projects
                            .as_mut()
                            .unwrap()
                            .extend(config.retaining_projects.iter().cloned().map(|p| (p, ())));
                        logger.logf(format!("Root files for config {entry_key} changed"));
                        should_invalidate_cache = has_excessive_changes;
                    },
                );
                !should_invalidate_cache
            });
            if should_invalidate_cache {
                return self.invalidate_cache(logger);
            }
        }

        // Handle possible root file creation
        if !created_files.is_empty() {
            self.configs.range_(|_, entry| {
                let entry_key = entry.key();
                entry.change_if(
                    |config| {
                        if config.command_line.is_none()
                            || config.root_files_watch.is_none()
                            || config.pending_reload != PendingReload::None
                        {
                            return false;
                        }
                        logger.logf(format!(
                            "Checking if any of {} created files match root files for config {}",
                            created_files.len(),
                            entry_key
                        ));
                        let command_line = config.command_line.as_ref().unwrap();
                        for (path, file_name) in &created_files {
                            if command_line.possibly_matches_file_name(file_name) {
                                return true;
                            }
                            if command_line.possibly_matches_directory_name(path)
                                && self.fs.directory_exists(file_name)
                            {
                                // If we got a creation event for a directory, it's probably a symlink. We don't need to
                                // test realpath here; this is enough confidence to trigger a filename reload.
                                return true;
                            }
                        }
                        false
                    },
                    |config| {
                        config.pending_reload = PendingReload::FileNames;
                        if affected_projects.is_none() {
                            affected_projects = Some(HashMap::new());
                        }
                        affected_projects
                            .as_mut()
                            .unwrap()
                            .extend(config.retaining_projects.iter().cloned().map(|p| (p, ())));
                        logger.logf(format!("Root files for config {entry_key} changed"));
                        should_invalidate_cache = has_excessive_changes;
                    },
                );
                !should_invalidate_cache
            });
            if should_invalidate_cache {
                return self.invalidate_cache(logger);
            }
        }

        ChangeFileResult {
            affected_projects: affected_projects.unwrap_or_default(),
            affected_files: affected_files.unwrap_or_default(),
        }
    }

    pub fn handle_config_change(
        &self,
        entry: &Arc<Mutex<dirty::SyncMapEntry<tspath::Path, ConfigFileEntry>>>,
        logger: &LogTree,
    ) -> HashMap<tspath::Path, ()> {
        let mut affected_projects = HashMap::new();
        let mut entry = entry.lock().unwrap_or_else(|err| err.into_inner());
        let changed = entry.change_if(
            |config| config.pending_reload != PendingReload::Full,
            |config| config.pending_reload = PendingReload::Full,
        );
        if changed {
            logger.logf(format!("Config file {} changed", entry.key()));
            affected_projects.extend(
                entry
                    .value()
                    .retaining_projects
                    .iter()
                    .cloned()
                    .map(|p| (p, ())),
            );
        }

        affected_projects
    }

    pub fn compute_config_file_name(
        &self,
        file_name: &str,
        skip_search_in_directory_of_file: bool,
        logger: &LogTree,
    ) -> String {
        let search_path = tspath::get_directory_path(file_name);
        // Prefer custom config file if provided; search ancestors with correct skip behavior.
        if !self.custom_config_file_name.is_empty() {
            let mut skip = skip_search_in_directory_of_file;
            let result = tspath::for_each_ancestor_directory(search_path.clone(), |directory| {
                if !skip {
                    let custom_path =
                        tspath::combine_paths(directory, &[&self.custom_config_file_name]);
                    if self.fs().file_exists(&custom_path) {
                        return Some(custom_path);
                    }
                }
                if directory.ends_with("/node_modules") {
                    return Some(String::new());
                }
                skip = false;
                None
            })
            .unwrap_or_default();
            if !result.is_empty() {
                logger.logf(format!(
                    "computeConfigFileName:: File: {}:: Result: {}",
                    file_name, result
                ));
                return result;
            }
        }

        // When searching for ancestor of a config file, determine which config types to skip
        // in the starting directory. This matches TSServer's forEachConfigFileLocation behavior:
        // - For ancestor of tsconfig.json: skip tsconfig.json but still check jsconfig.json
        // - For ancestor of jsconfig.json: skip both tsconfig.json and jsconfig.json
        let mut skip_tsconfig = skip_search_in_directory_of_file;
        let mut skip_jsconfig =
            skip_search_in_directory_of_file && !file_name.ends_with("/tsconfig.json");
        let result = tspath::for_each_ancestor_directory(search_path, |directory| {
            if !skip_tsconfig {
                let tsconfig_path = tspath::combine_paths(directory, &["tsconfig.json"]);
                if self.fs().file_exists(&tsconfig_path) {
                    return Some(tsconfig_path);
                }
            }
            if !skip_jsconfig {
                let jsconfig_path = tspath::combine_paths(directory, &["jsconfig.json"]);
                if self.fs().file_exists(&jsconfig_path) {
                    return Some(jsconfig_path);
                }
            }
            if directory.ends_with("/node_modules") {
                return Some(String::new());
            }
            skip_tsconfig = false;
            skip_jsconfig = false;
            None
        })
        .unwrap_or_default();
        logger.logf(format!(
            "computeConfigFileName:: File: {}:: Result: {}",
            file_name, result
        ));
        result
    }

    pub fn get_config_file_name_for_file(
        &mut self,
        file_name: &str,
        path: tspath::Path,
        logger: &LogTree,
    ) -> String {
        if tspath::is_dynamic_file_name(file_name) {
            return String::new();
        }

        if let Some(entry) = self.config_file_names.get(path.clone()) {
            return entry.value().nearest_config_file_name.clone();
        }

        let config_name = self.compute_config_file_name(file_name, false, logger);
        if (self.is_open_file)(path.clone()) {
            self.config_file_names.add(
                path,
                ConfigFileNames {
                    nearest_config_file_name: config_name.clone(),
                    ancestors: HashMap::new(),
                },
            );
        }
        config_name
    }

    pub fn for_each_config_file_name_for(
        &mut self,
        path: tspath::Path,
        mut cb: impl FnMut(String),
    ) {
        if tspath::is_dynamic_file_name(&path.to_string()) {
            return;
        }

        if let Some(entry) = self.config_file_names.get(path) {
            let mut config_file_name = entry.value().nearest_config_file_name.clone();
            while !config_file_name.is_empty() {
                cb(config_file_name.clone());
                if let Some(ancestor_config_name) = entry.value().ancestors.get(&config_file_name) {
                    config_file_name = ancestor_config_name.clone();
                } else {
                    return;
                }
            }
        }
    }

    pub fn get_ancestor_config_file_name(
        &mut self,
        file_name: &str,
        path: tspath::Path,
        config_file_name: &str,
        logger: &LogTree,
    ) -> String {
        if tspath::is_dynamic_file_name(file_name) {
            return String::new();
        }

        let Some(mut entry) = self.config_file_names.get(path.clone()) else {
            return String::new();
        };

        if let Some(ancestor_config_name) = entry.value().ancestors.get(config_file_name) {
            return ancestor_config_name.clone();
        }

        // Look for config in parent folders of config file
        let result = self.compute_config_file_name(config_file_name, true, logger);

        if (self.is_open_file)(path) {
            entry.change(|value| {
                value
                    .ancestors
                    .insert(config_file_name.to_string(), result.clone());
            });
        }
        result
    }

    pub fn cleanup(&self) {
        self.configs.range_(|_, entry| {
            entry.delete_if(|value| {
                value.retaining_projects.is_empty()
                    && value.retaining_open_files.is_empty()
                    && value.retaining_configs.is_empty()
            });
            true
        });
    }
}

impl Clone for ConfigFileRegistryBuilder {
    fn clone(&self) -> Self {
        Self {
            has_relative_pattern_capability: self.has_relative_pattern_capability,
            fs: self.fs.clone(),
            is_open_file: self.is_open_file.clone(),
            extended_config_cache: self.extended_config_cache.clone(),
            snapshot_id: self.snapshot_id,
            session_options: self.session_options.clone(),
            custom_config_file_name: self.custom_config_file_name.clone(),
            base: self.base.clone_registry(),
            configs: self.configs.clone(),
            config_file_names: self.config_file_names.clone(),
            custom_config_file_name_changed: self.custom_config_file_name_changed,
        }
    }
}

pub struct ChangeFileResult {
    pub affected_projects: HashMap<tspath::Path, ()>,
    pub affected_files: HashMap<tspath::Path, ()>,
}

impl ChangeFileResult {
    pub fn is_empty(&self) -> bool {
        self.affected_projects.is_empty() && self.affected_files.is_empty()
    }
}

impl tsoptions::ParseConfigHost for ConfigFileRegistryBuilder {
    fn fs(&self) -> &dyn vfs::Fs {
        &self.fs
    }

    fn get_current_directory(&self) -> String {
        self.session_options.current_directory.clone()
    }
}

impl tsoptions::ExtendedConfigCache for ConfigFileRegistryBuilder {
    fn get_extended_config(
        &self,
        file_name: String,
        path: tspath::Path,
        resolution_stack: Vec<String>,
        host: &dyn tsoptions::ParseConfigHost,
    ) -> tsoptions::ExtendedConfigCacheEntry {
        let mut content = String::new();
        if let Some(fh) = self.fs.get_file_by_path(&file_name, &path) {
            content = fh.content();
        }

        let args = ExtendedConfigParseArgs {
            file_name,
            content,
            fs: self.fs.clone(),
            resolution_stack,
        };

        self.extended_config_cache
            .load_and_acquire(path, self.snapshot_id, &args, |path, args| {
                parse_extended_config_cache_entry(path, args, host, self)
            })
            .extended_config_cache_entry
            .unwrap_or_default()
    }
}
