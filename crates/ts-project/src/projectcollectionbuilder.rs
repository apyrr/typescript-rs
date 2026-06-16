use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::dirty;
use crate::dirty::Value;
use crate::logging::{LogTree, Logger};
use crate::{
    AtaStateChange, ChangeFileResult, Client, ClientHandle, ConfigFileRegistry,
    ConfigFileRegistryBuilder, ExtendedConfigCache, FileChangeSummary, FileHandle,
    INFERRED_PROJECT_NAME, Kind, ParseCache, ProgramUpdateKind, Project, ProjectCollection,
    ProjectTreeRequest, SessionOptions, SnapshotFsBuilder,
    find_default_configured_project_from_program_inclusion, new_compiler_host,
    new_config_file_registry_builder, new_configured_project, new_inferred_project,
};

type ProjectEntry = Arc<Mutex<dirty::SyncMapEntry<tspath::Path, Project>>>;

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
#[repr(i32)]
pub enum ProjectLoadKind {
    // Project is not created or updated, only looked up in cache
    #[default]
    Find = 0,
    // Project is created and then its graph is updated
    Create = 1,
}

pub struct ProjectCollectionBuilder {
    pub session_options: SessionOptions,
    pub parse_cache: ParseCache,
    pub extended_config_cache: ExtendedConfigCache,
    pub to_path: Arc<dyn Fn(String) -> tspath::Path + Send + Sync>,

    pub ctx: core::Context,
    pub fs: SnapshotFsBuilder,
    pub base: ProjectCollection,
    pub compiler_options_for_inferred_projects: Option<core::CompilerOptions>,
    pub config_file_registry_builder: ConfigFileRegistryBuilder,

    pub client: Option<ClientHandle>, // optional; used for project loading notifications

    pub new_snapshot_id: u64,
    pub program_structure_changed: bool,
    pub default_projects_invalidated: bool,

    pub file_default_projects: HashMap<tspath::Path, tspath::Path>,
    pub configured_projects: dirty::SyncMap<tspath::Path, Project>,
    pub inferred_project: dirty::Box<Project>,

    pub api_opened_projects: HashMap<tspath::Path, ()>,
}

impl Clone for ProjectCollectionBuilder {
    fn clone(&self) -> Self {
        Self {
            session_options: self.session_options.clone(),
            parse_cache: self.parse_cache.clone(),
            extended_config_cache: self.extended_config_cache.clone(),
            to_path: self.to_path.clone(),
            ctx: self.ctx.clone(),
            fs: self.fs.clone(),
            base: self.base.clone_collection(),
            compiler_options_for_inferred_projects: self
                .compiler_options_for_inferred_projects
                .clone(),
            config_file_registry_builder: self.config_file_registry_builder.clone(),
            client: self.client.clone(),
            new_snapshot_id: self.new_snapshot_id,
            program_structure_changed: self.program_structure_changed,
            default_projects_invalidated: self.default_projects_invalidated,
            file_default_projects: self.file_default_projects.clone(),
            configured_projects: self.configured_projects.clone(),
            inferred_project: self.inferred_project.clone(),
            api_opened_projects: self.api_opened_projects.clone(),
        }
    }
}

#[derive(Clone)]
pub struct SearchNode {
    pub config_file_name: String,
    pub load_kind: ProjectLoadKind,
    pub logger: Option<Arc<LogTree>>,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct SearchNodeKey {
    pub config_file_name: String,
    pub load_kind: ProjectLoadKind,
}

#[derive(Default)]
pub struct SearchResult {
    pub project: Option<ProjectEntry>,
    pub retain: collections::Set<tspath::Path>,
}

pub fn new_project_collection_builder(
    ctx: core::Context,
    new_snapshot_id: u64,
    fs: SnapshotFsBuilder,
    old_project_collection: ProjectCollection,
    old_config_file_registry: ConfigFileRegistry,
    old_api_opened_projects: HashMap<tspath::Path, ()>,
    compiler_options_for_inferred_projects: Option<core::CompilerOptions>,
    session_options: SessionOptions,
    custom_config_file_name: String,
    parse_cache: ParseCache,
    extended_config_cache: ExtendedConfigCache,
    client: Option<ClientHandle>,
) -> ProjectCollectionBuilder {
    let to_path_fs = fs.clone();
    ProjectCollectionBuilder {
        ctx: ctx.clone(),
        fs: fs.clone(),
        to_path: Arc::new(move |file_name| (to_path_fs.to_path)(&file_name)),
        compiler_options_for_inferred_projects,
        session_options: session_options.clone(),
        parse_cache,
        extended_config_cache: extended_config_cache.clone(),
        base: old_project_collection.clone_collection(),
        config_file_registry_builder: new_config_file_registry_builder(
            false,
            fs.clone(),
            old_config_file_registry,
            extended_config_cache,
            new_snapshot_id,
            session_options,
            custom_config_file_name,
            None,
        ),
        new_snapshot_id,
        configured_projects: dirty::new_sync_map(old_project_collection.configured_projects),
        inferred_project: dirty::new_box(
            old_project_collection.inferred_project.unwrap_or_default(),
        ),
        api_opened_projects: old_api_opened_projects.clone(),
        client,
        program_structure_changed: false,
        default_projects_invalidated: false,
        file_default_projects: HashMap::new(),
    }
}

impl ProjectCollectionBuilder {
    pub fn finalize(&self, _logger: Option<&LogTree>) -> (ProjectCollection, ConfigFileRegistry) {
        let mut changed = false;
        let mut new_project_collection = self.base.clone_collection();

        let (configured_projects, configured_projects_changed) =
            self.configured_projects.finalize();
        if configured_projects_changed {
            if !changed {
                new_project_collection = new_project_collection.clone_collection();
                changed = true;
            }
            new_project_collection.configured_projects = configured_projects;
        }

        if self.file_default_projects != self.base.file_default_projects {
            if !changed {
                new_project_collection = new_project_collection.clone_collection();
                changed = true;
            }
            new_project_collection.file_default_projects = self.file_default_projects.clone();
        }

        let (new_inferred_project, inferred_project_changed) = self.inferred_project.finalize();
        if inferred_project_changed {
            if !changed {
                new_project_collection = new_project_collection.clone_collection();
                changed = true;
            }
            new_project_collection.inferred_project =
                (!new_inferred_project.config_file_name.is_empty()).then_some(new_inferred_project);
        }

        let config_file_registry = self.config_file_registry_builder.finalize();
        if self.base.config_file_registry.as_ref().is_none_or(|base| {
            base.get_config_file_name(String::new())
                != config_file_registry.get_config_file_name(String::new())
        }) {
            if !changed {
                new_project_collection = new_project_collection.clone_collection();
                changed = true;
            }
            new_project_collection.config_file_registry =
                Some(config_file_registry.clone_registry());
        }

        if self.api_opened_projects != self.base.api_opened_projects {
            if !changed {
                new_project_collection = new_project_collection.clone_collection();
            }
            new_project_collection.api_opened_projects = self.api_opened_projects.clone();
        }

        (new_project_collection, config_file_registry)
    }

    pub fn for_each_project(&self, mut f: impl FnMut(ProjectEntry) -> bool) {
        let mut keep_going = true;
        self.configured_projects.range_(|_, entry| {
            keep_going = f(entry.clone());
            keep_going
        });
        if !keep_going {
            return;
        }
        if self.inferred_project.value().config_file_name.is_empty() {
            return;
        }
        let inferred = self.inferred_project.value();
        let entry = self
            .configured_projects
            .new_entry(INFERRED_PROJECT_NAME.to_string(), inferred);
        f(entry);
    }

    pub fn handle_api_request(
        &mut self,
        api_request: &crate::ApiSnapshotRequest,
        logger: &LogTree,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut projects_to_close = HashMap::new();
        if let Some(close_projects) = &api_request.close_projects {
            if let Some(keys) = close_projects.keys() {
                for project_path in keys {
                    projects_to_close.insert(project_path.clone(), ());
                    self.api_opened_projects.remove(project_path);
                }
            }
        }

        if let Some(open_projects) = &api_request.open_projects {
            if let Some(keys) = open_projects.keys() {
                for config_file_name in keys {
                    let config_path = (self.to_path)(config_file_name.clone());
                    if self
                        .find_or_create_project(
                            config_file_name,
                            config_path.clone(),
                            ProjectLoadKind::Create,
                            Some(logger),
                        )
                        .is_some()
                    {
                        self.api_opened_projects.insert(config_path, ());
                    } else {
                        return Err(
                            format!("project not found for open: {config_file_name}").into()
                        );
                    }
                }
            }
        }

        for config_path in self.api_opened_projects.keys().cloned().collect::<Vec<_>>() {
            if let Some(entry) = self.configured_projects.load(config_path.clone()) {
                self.update_program(entry, logger);
            } else {
                return Err(format!("project not found for update: {config_path}").into());
            }
        }

        for overlay in self.fs.overlays.values().cloned().collect::<Vec<_>>() {
            let file_name = overlay.file_name();
            let path = (self.to_path)(file_name.clone());
            if let Some(entry) = self.find_default_configured_project(&file_name, path) {
                projects_to_close.remove(&entry.lock().unwrap().value().config_file_path);
            }
        }

        for project_path in projects_to_close.keys().cloned().collect::<Vec<_>>() {
            if let Some(entry) = self.configured_projects.load(project_path) {
                self.delete_configured_project(entry, Some(logger));
            }
        }
        Ok(())
    }

    pub fn did_change_files(&mut self, summary: FileChangeSummary, logger: &LogTree) {
        let changed_files = summary
            .changed
            .iter()
            .map(|uri| (self.to_path)(uri.file_name()))
            .collect::<Vec<_>>();

        let config_change_logger = logger.fork("Checking for changes affecting config files");
        let config_change_result = self
            .config_file_registry_builder
            .did_change_files(summary.clone(), &config_change_logger);
        log_change_file_result(&config_change_result, &config_change_logger);

        self.program_structure_changed =
            self.mark_projects_affected_by_config_changes(config_change_result, logger);

        // PORT NOTE: reshaped for borrowck. Preserve forEachProject order while
        // allowing each entry handler to call back into the builder mutably.
        let mut project_entries = Vec::new();
        self.for_each_project(|entry| {
            project_entries.push(entry);
            true
        });
        for entry in project_entries {
            let mut entry = entry;
            let is_inferred_project = entry.lock().unwrap().value().kind == Kind::Inferred;
            if summary.has_excessive_non_create_watch_events() {
                entry.lock().unwrap().change(|p| {
                    p.dirty = true;
                    p.dirty_file_path = tspath::Path::default();
                    logger.logf(format!(
                        "Marking project as dirty due to excessive watch changes: {}",
                        p.config_file_path
                    ));
                });
                if is_inferred_project {
                    self.inferred_project.set(entry.lock().unwrap().value());
                }
                continue;
            }

            let changed_inferred = self.mark_files_changed(
                entry.clone(),
                changed_files.clone(),
                lsproto::FileChangeType::CHANGED,
                logger,
            );
            if is_inferred_project && changed_inferred {
                self.inferred_project.set(entry.lock().unwrap().value());
            }

            if entry.lock().unwrap().value().kind == Kind::Inferred && !summary.closed.is_empty() {
                let project = self.inferred_project.value();
                let root_files_map = project
                    .command_line
                    .as_ref()
                    .map(|command_line| command_line.file_names_by_path())
                    .unwrap_or_default();
                let mut new_root_files = project
                    .command_line
                    .as_ref()
                    .map(|command_line| command_line.file_names().to_vec())
                    .unwrap_or_default();
                for uri in summary.closed.iter() {
                    let file_name = uri.file_name();
                    let path = (self.to_path)(file_name.to_string());
                    if root_files_map.contains_key(&path) {
                        if let Some(index) =
                            new_root_files.iter().position(|root| root == &file_name)
                        {
                            new_root_files.remove(index);
                        }
                    }
                }
                self.update_inferred_project_roots(new_root_files, Some(logger));
                entry = self.configured_projects.new_entry(
                    INFERRED_PROJECT_NAME.to_string(),
                    self.inferred_project.value(),
                );
            }

            if !summary.deleted.is_empty() {
                let deleted_paths = summary
                    .deleted
                    .iter()
                    .map(|uri| (self.to_path)(uri.file_name().to_string()))
                    .collect();
                let deleted_inferred = self.mark_files_changed(
                    entry.clone(),
                    deleted_paths,
                    lsproto::FileChangeType::DELETED,
                    logger,
                );
                if is_inferred_project && deleted_inferred {
                    self.inferred_project.set(entry.lock().unwrap().value());
                }
            }

            if !summary.created.is_empty() {
                let created_paths = summary
                    .created
                    .iter()
                    .map(|uri| (self.to_path)(uri.file_name().to_string()))
                    .collect();
                let created_inferred = self.mark_files_changed(
                    entry.clone(),
                    created_paths,
                    lsproto::FileChangeType::CREATED,
                    logger,
                );
                if is_inferred_project && created_inferred {
                    self.inferred_project.set(entry.lock().unwrap().value());
                }
            }
        }

        if !summary.opened.to_string().is_empty() || !summary.reopened.to_string().is_empty() {
            let mut to_remove_projects = collections::Set::default();
            let file_name = if !summary.opened.to_string().is_empty() {
                summary.opened.file_name().to_string()
            } else {
                summary.reopened.file_name().to_string()
            };
            let path = (self.to_path)(file_name.clone());
            let open_file_result =
                self.ensure_configured_project_and_ancestors_for_file(&file_name, path, logger);
            self.configured_projects.range_(|project_path, _| {
                to_remove_projects.add(project_path);
                true
            });

            let mut inferred_project_files = Vec::new();
            for overlay in self.fs.overlays.values().cloned().collect::<Vec<_>>() {
                let open_file = overlay.file_name();
                let open_file_path = (self.to_path)(open_file.clone());
                if let Some(project) =
                    self.find_default_configured_project(&open_file, open_file_path.clone())
                {
                    let project_value = project.lock().unwrap().value();
                    to_remove_projects.delete(&project_value.config_file_path);
                    if let Some(program) = project_value.get_program() {
                        program.range_resolved_project_reference(|reference_path, _, _, _| {
                            if self
                                .configured_projects
                                .load(reference_path.clone())
                                .is_some()
                            {
                                to_remove_projects.delete(&reference_path);
                            }
                            true
                        });
                    }
                    let mut config_file_names = Vec::new();
                    self.config_file_registry_builder
                        .for_each_config_file_name_for(open_file_path, |config_file_name| {
                            config_file_names.push(config_file_name);
                        });
                    for config_file_name in config_file_names {
                        let ancestor_path = (self.to_path)(config_file_name.clone());
                        if let Some(ancestor) = self.find_or_create_project(
                            &config_file_name,
                            ancestor_path,
                            ProjectLoadKind::Find,
                            Some(logger),
                        ) {
                            to_remove_projects
                                .delete(&ancestor.lock().unwrap().value().config_file_path);
                        }
                    }
                } else {
                    inferred_project_files.push(open_file.to_string());
                }
            }

            for project_path in to_remove_projects.keys().cloned().unwrap_or_default() {
                if open_file_result.retain.has(&project_path)
                    || self.api_opened_projects.contains_key(&project_path)
                {
                    continue;
                }
                if let Some(project) = self.configured_projects.load(project_path) {
                    self.delete_configured_project(project, Some(logger));
                }
            }
            self.update_inferred_project_roots(inferred_project_files, Some(logger));
            self.config_file_registry_builder.cleanup();
        }
    }

    pub fn cleanup_inferred_project(&mut self, logger: &LogTree) {
        let mut inferred_project_files = Vec::new();
        for (path, overlay) in self.fs.overlays.clone() {
            if self
                .find_default_configured_project(&overlay.file_name(), path)
                .is_none()
            {
                inferred_project_files.push(overlay.file_name());
            }
        }
        self.update_inferred_project_roots(inferred_project_files, Some(logger));
    }

    pub fn ensure_inferred_project_includes_closed_file(
        &mut self,
        file_name: &str,
        logger: &LogTree,
    ) {
        let mut inferred_project_files = Vec::new();
        for (path, overlay) in self.fs.overlays.clone() {
            if self
                .find_default_configured_project(&overlay.file_name(), path)
                .is_none()
            {
                inferred_project_files.push(overlay.file_name());
            }
        }
        inferred_project_files.push(file_name.to_string());
        self.update_inferred_project_roots(inferred_project_files, Some(logger));
        self.update_inferred_project(logger);
    }

    // DidRequestFile ensures projects are loaded for the given URI.
    // If configuredProjectsOnly is true, only configured projects are loaded; no inferred project is created
    // and it is not guaranteed that there will be any project containing the file in the resulting snapshot.
    pub fn did_request_file(
        &mut self,
        uri: lsproto::DocumentUri,
        configured_projects_only: bool,
        logger: &LogTree,
    ) {
        let start_time = Instant::now();
        let file_name = lsproto::DocumentUriExt::file_name(&uri);
        let path = (self.to_path)(file_name.clone());
        if self.default_projects_invalidated {
            self.ensure_configured_project_and_ancestors_for_file(&file_name, path.clone(), logger);
            if !self.fs.is_open_file(&path) {
                return;
            }
        }

        if self.fs.is_open_file(&path) {
            let mut has_changes = self.program_structure_changed;
            if let Some(result) = self.find_default_project(&file_name, path.clone()) {
                has_changes = if result.lock().unwrap().value().kind == Kind::Inferred {
                    self.update_inferred_project(logger)
                } else {
                    self.update_program(result.clone(), logger)
                } || has_changes;
                if !result.lock().unwrap().value().config_file_name.is_empty() {
                    if has_changes {
                        self.cleanup_inferred_project(logger);
                        self.update_inferred_project(logger);
                    }
                    return;
                }
            }

            let mut current_projects = Vec::new();
            self.configured_projects.range_(|_, entry| {
                current_projects.push(entry.clone());
                true
            });
            for entry in current_projects {
                has_changes = self.update_program(entry, logger) || has_changes;
            }
            if has_changes {
                self.cleanup_inferred_project(logger);
            }

            self.update_inferred_project(logger);
        } else {
            let result =
                self.ensure_configured_project_and_ancestors_for_file(&file_name, path, logger);
            if result.project.is_none() && !configured_projects_only {
                self.ensure_inferred_project_includes_closed_file(&file_name, logger);
            }
        }

        logger.logf(format!(
            "Completed file request for {} in {:?}",
            file_name,
            start_time.elapsed()
        ));
    }

    pub fn did_request_project(&mut self, project_id: tspath::Path, logger: &LogTree) {
        let start_time = Instant::now();
        if project_id == INFERRED_PROJECT_NAME {
            self.update_inferred_project(logger);
        } else if let Some(entry) = self.configured_projects.load(project_id.clone()) {
            self.update_program(entry, logger);
        }

        logger.logf(format!(
            "Completed project update request for {} in {:?}",
            project_id,
            start_time.elapsed()
        ));
    }

    pub fn did_request_project_trees(
        &mut self,
        project_tree_request: &ProjectTreeRequest,
        logger: &LogTree,
    ) {
        let start_time = Instant::now();
        let mut current_projects = Vec::new();
        self.configured_projects.range_(|project_id, _| {
            current_projects.push(project_id);
            true
        });

        let seen_projects = collections::SyncSet::default();
        let wg = core::new_work_group(false);
        for project_id in current_projects {
            if let Some(entry) = self.configured_projects.load(project_id) {
                let project = entry.lock().unwrap().value();
                if project_tree_request.is_all_projects()
                    || project.has_potential_project_reference(project_tree_request)
                {
                    self.update_program(entry.clone(), logger);
                }
                self.ensure_project_tree(&*wg, entry, project_tree_request, &seen_projects, logger);
            }
        }
        wg.run_and_wait();
        logger.logf(format!(
            "Completed project tree request for {:?} in {:?}",
            project_tree_request.projects(),
            start_time.elapsed()
        ));
    }

    pub fn ensure_project_tree(
        &mut self,
        wg: &dyn core::WorkGroup,
        entry: ProjectEntry,
        project_tree_request: &ProjectTreeRequest,
        seen_projects: &collections::SyncSet<tspath::Path>,
        logger: &LogTree,
    ) {
        if !seen_projects.add_if_absent(entry.lock().unwrap().key()) {
            return;
        }

        let project = entry.lock().unwrap().value();
        let Some(program) = project.get_program() else {
            return;
        };
        if program
            .command_line()
            .compiler_options()
            .disable_referenced_project_load
            .is_true()
        {
            return;
        }

        let children = program.get_resolved_project_references();
        for child_config in children {
            if !project_tree_request.is_all_projects()
                && program.range_resolved_project_reference_in_child_config(
                    &child_config,
                    |reference_path, _, _, _| {
                        !project_tree_request.is_project_referenced(&reference_path)
                    },
                )
            {
                continue;
            }
            let child_config_name = child_config.config_name();
            let child_path = child_config
                .config_file
                .as_ref()
                .map(|config_file| config_file.source_file.path())
                .unwrap_or_else(|| (self.to_path)(child_config_name.clone()));
            if let Some(child_project_entry) = self.find_or_create_project(
                &child_config_name,
                child_path,
                ProjectLoadKind::Create,
                Some(logger),
            ) {
                self.update_program(child_project_entry.clone(), logger);
                self.ensure_project_tree(
                    wg,
                    child_project_entry,
                    project_tree_request,
                    seen_projects,
                    logger,
                );
            }
        }
    }

    pub fn did_update_ata_state(
        &mut self,
        ata_changes: HashMap<tspath::Path, AtaStateChange>,
        logger: &LogTree,
    ) {
        for (project_path, ata_change) in ata_changes {
            let update_project = |project: &mut Project| {
                if ata_change.typings_info.is_none() {
                    return;
                }
                project.typings_files = ata_change.typings_files.clone();
                let typings_watch_globs = crate::get_typings_locations_globs(
                    &ata_change.typings_files_to_watch,
                    &self.session_options.typings_location,
                    &self.session_options.current_directory,
                    &project.current_directory,
                    self.fs.fs().use_case_sensitive_file_names(),
                );
                project.typings_watch = project
                    .typings_watch
                    .clone()
                    .map(|watch| watch.clone_with(typings_watch_globs));
                project.dirty = true;
                project.dirty_file_path = tspath::Path::default();
            };

            if project_path == INFERRED_PROJECT_NAME {
                let mut project = self.inferred_project.value();
                update_project(&mut project);
                self.inferred_project.set(project);
            } else if let Some(project) = self.configured_projects.load(project_path.clone()) {
                project.lock().unwrap().change(update_project);
            }
            logger.logf(format!("Updated ATA state for project {project_path}"));
        }
    }

    // if customConfigFileName changes, invalidate default projects.
    pub fn did_change_custom_config_file_name(&mut self, logger: &LogTree) {
        if !self
            .config_file_registry_builder
            .did_change_custom_config_file_name(logger)
        {
            return;
        }

        self.file_default_projects.clear();
        self.default_projects_invalidated = true;
        self.program_structure_changed = true;
    }

    pub fn mark_projects_affected_by_config_changes(
        &mut self,
        config_change_result: ChangeFileResult,
        logger: &LogTree,
    ) -> bool {
        for project_path in config_change_result.affected_projects.keys() {
            let project = self
                .configured_projects
                .load(project_path.clone())
                .unwrap_or_else(|| {
                    panic!(
                        "project {} affected by config change not found",
                        project_path
                    )
                });
            project.lock().unwrap().change_if(
                |p| !p.dirty || !p.dirty_file_path.is_empty(),
                |p| {
                    p.dirty = true;
                    p.dirty_file_path = tspath::Path::default();
                    logger.logf(format!(
                        "Marking project {} as dirty due to change affecting config",
                        project_path
                    ));
                },
            );
        }

        // Recompute default projects for open files that now have different config file presence.
        let mut has_changes = false;
        for path in config_change_result.affected_files.keys() {
            if let Some(overlay) = self.fs.overlays.get(path) {
                let file_name = overlay.file_name();
                let _ = self.ensure_configured_project_and_ancestors_for_file(
                    &file_name,
                    path.clone(),
                    logger,
                );
            }
            has_changes = true;
        }

        has_changes
    }

    pub fn find_default_project(
        &mut self,
        file_name: &str,
        path: tspath::Path,
    ) -> Option<ProjectEntry> {
        if let Some(configured_project) =
            self.find_default_configured_project(file_name, path.clone())
        {
            return Some(configured_project);
        }
        if self
            .file_default_projects
            .get(&path)
            .is_some_and(|key| key == INFERRED_PROJECT_NAME)
        {
            return Some(self.configured_projects.new_entry(
                INFERRED_PROJECT_NAME.to_string(),
                self.inferred_project.value(),
            ));
        }
        let inferred_project = self.inferred_project.value();
        if !inferred_project.config_file_name.is_empty()
            && inferred_project.contains_file(path.clone())
        {
            self.file_default_projects
                .insert(path, INFERRED_PROJECT_NAME.to_string());
            return Some(
                self.configured_projects
                    .new_entry(INFERRED_PROJECT_NAME.to_string(), inferred_project),
            );
        }
        None
    }

    pub fn find_default_configured_project(
        &mut self,
        file_name: &str,
        path: tspath::Path,
    ) -> Option<ProjectEntry> {
        if let Some(key) = self.file_default_projects.get(&path) {
            if key != INFERRED_PROJECT_NAME {
                if let Some(entry) = self.configured_projects.load(key.clone()) {
                    return Some(entry);
                }
            }
        }
        // Sort configured projects so we can use a deterministic "first" as a last resort.
        let mut configured_project_paths = Vec::new();
        let mut configured_projects = HashMap::new();
        self.configured_projects.range_(|project_path, entry| {
            configured_project_paths.push(project_path.clone());
            configured_projects.insert(project_path, entry.clone());
            true
        });
        configured_project_paths.sort();

        let (project, multiple_candidates) = find_default_configured_project_from_program_inclusion(
            file_name,
            path.clone(),
            configured_project_paths,
            |path| {
                configured_projects
                    .get(&path)
                    .map(|entry| entry.lock().unwrap().value())
            },
        );

        if multiple_candidates {
            let result = self.find_or_create_default_configured_project_for_file(
                file_name,
                path,
                ProjectLoadKind::Find,
                None,
            );
            if let Some(project) = result.project {
                return Some(project);
            }
        }

        configured_projects.get(&project).cloned()
    }

    pub fn ensure_configured_project_and_ancestors_for_file(
        &mut self,
        file_name: &str,
        path: tspath::Path,
        logger: &LogTree,
    ) -> SearchResult {
        let mut result = self.find_or_create_default_configured_project_for_file(
            file_name,
            path.clone(),
            ProjectLoadKind::Create,
            Some(logger),
        );
        if result.project.is_some() && self.fs.is_open_file(&path) {
            self.create_ancestor_tree(file_name, path, &mut result, logger);
        }
        result
    }

    pub fn create_ancestor_tree(
        &mut self,
        file_name: &str,
        path: tspath::Path,
        open_result: &mut SearchResult,
        logger: &LogTree,
    ) {
        let Some(mut project_entry) = open_result.project.clone() else {
            return;
        };
        loop {
            let project = project_entry.lock().unwrap().value();
            if let Some(command_line) = &project.command_line {
                let compiler_options = command_line.compiler_options();
                if !compiler_options.composite.is_true()
                    || compiler_options.disable_solution_searching.is_true()
                {
                    return;
                }
            }

            let ancestor_config_name = self
                .config_file_registry_builder
                .get_ancestor_config_file_name(
                    file_name,
                    path.clone(),
                    &project.config_file_name,
                    logger,
                );
            if ancestor_config_name.is_empty() {
                return;
            }

            let ancestor_path = (self.to_path)(ancestor_config_name.clone());
            let Some(ancestor) = self.find_or_create_project(
                &ancestor_config_name,
                ancestor_path.clone(),
                ProjectLoadKind::Create,
                Some(logger),
            ) else {
                return;
            };
            open_result.retain.add(ancestor_path);

            let ancestor_value = ancestor.lock().unwrap().value();
            if ancestor_value.command_line.is_none()
                && project
                    .command_line
                    .as_ref()
                    .is_none_or(|command_line| command_line.compiler_options().composite.is_true())
            {
                ancestor.lock().unwrap().change(|ancestor_project| {
                    ancestor_project
                        .set_potential_project_reference(project.config_file_path.clone());
                });
            }

            project_entry = ancestor;
        }
    }

    pub fn find_or_create_default_configured_project_worker(
        &mut self,
        file_name: &str,
        path: tspath::Path,
        config_file_name: String,
        load_kind: ProjectLoadKind,
        visited: Option<collections::SyncSet<SearchNodeKey>>,
        mut fallback: Option<SearchResult>,
        logger: Option<&LogTree>,
    ) -> SearchResult {
        let configs = collections::SyncMap::<tspath::Path, tsoptions::ParsedCommandLine>::default();
        let visited = visited.unwrap_or_default();
        let to_path = self.to_path.clone();
        let search = core::breadth_first_search_parallel_ex(
            SearchNode {
                config_file_name: config_file_name.clone(),
                load_kind,
                logger: logger.map(|logger| {
                    logger.fork(format!(
                        "Searching for default configured project for {file_name}"
                    ))
                }),
            },
            |node| {
                let config_path = (to_path)(node.config_file_name.clone());
                let (config, ok) = configs.load(&config_path);
                if ok
                    && config
                        .as_ref()
                        .is_some_and(|config| !config.project_references().is_empty())
                {
                    let mut reference_load_kind = node.load_kind;
                    if config
                        .as_ref()
                        .unwrap()
                        .compiler_options()
                        .disable_referenced_project_load
                        .is_true()
                    {
                        reference_load_kind = ProjectLoadKind::Find;
                    }
                    let mut config = config.unwrap();
                    let references = config.resolved_project_reference_paths().to_vec();
                    let ref_logger = node.logger.as_ref().map(|logger| {
                        logger.fork(format!(
                            "Searching {} project references of {}",
                            references.len(),
                            node.config_file_name
                        ))
                    });
                    return references
                        .into_iter()
                        .map(|config_file_name| SearchNode {
                            config_file_name: config_file_name.clone(),
                            load_kind: reference_load_kind,
                            logger: ref_logger.as_ref().map(|logger| {
                                logger
                                    .fork(format!("Searching project reference {config_file_name}"))
                            }),
                        })
                        .collect();
                }
                Vec::new()
            },
            |node| {
                let config_file_path = (to_path)(node.config_file_name.clone());
                let default_logger = crate::logging::new_log_tree(String::new());
                let logger = node.logger.as_deref().unwrap_or(default_logger.as_ref());
                let acquire_logger = logger.fork("Acquiring config for open file");
                let config = self
                    .config_file_registry_builder
                    .find_or_acquire_config_for_file(
                        &node.config_file_name,
                        config_file_path.clone(),
                        path.clone(),
                        node.load_kind,
                        &acquire_logger,
                    );
                let Some(config) = config else {
                    logger.logf("Config file for project does not already exist".to_string());
                    return (false, false);
                };
                configs.store(config_file_path.clone(), Some(config.clone()));
                if config.file_names().is_empty() {
                    logger.logf("Project does not contain file (no root files)".to_string());
                    return (false, false);
                }
                if config.compiler_options().composite.is_true()
                    && !config.file_names_by_path().contains_key(&path)
                {
                    logger.logf(
                        "Project does not contain file (by composite config inclusion)".to_string(),
                    );
                    return (false, false);
                }
                let Some(project) = self.find_or_create_project(
                    &node.config_file_name,
                    config_file_path,
                    node.load_kind,
                    node.logger.as_deref(),
                ) else {
                    logger.logf("Project does not already exist".to_string());
                    return (false, false);
                };
                if node.load_kind == ProjectLoadKind::Create {
                    self.update_program(project.clone(), logger);
                }
                let project_value = project.lock().unwrap().value();
                if project_value.contains_file(path.clone()) {
                    let is_direct_inclusion =
                        !project_value.is_source_from_project_reference(path.clone());
                    logger.logf(format!(
                        "Project contains file {}",
                        if is_direct_inclusion {
                            "directly"
                        } else {
                            "as a source of a referenced project"
                        }
                    ));
                    return (true, is_direct_inclusion);
                }
                logger.logf("Project does not contain file".to_string());
                (false, false)
            },
            core::BreadthFirstSearchOptions {
                visited: Some(visited.clone()),
                preprocess_level: Some(Box::new(|level| {
                    let mut to_delete = Vec::new();
                    level.range(|node| {
                        if node.load_kind == ProjectLoadKind::Find
                            && level.has(&SearchNodeKey {
                                config_file_name: node.config_file_name.clone(),
                                load_kind: ProjectLoadKind::Create,
                            })
                        {
                            to_delete.push(SearchNodeKey {
                                config_file_name: node.config_file_name.clone(),
                                load_kind: node.load_kind,
                            });
                        }
                        true
                    });
                    for key in to_delete {
                        level.delete(&key);
                    }
                })),
            },
            |node| SearchNodeKey {
                config_file_name: node.config_file_name,
                load_kind: node.load_kind,
            },
        );

        let mut retain = collections::Set::default();
        let mut project = None;
        if let Some(first) = search.path.first() {
            project = self
                .configured_projects
                .load((self.to_path)(first.config_file_name.clone()));
            for node in &search.path {
                retain.add((self.to_path)(node.config_file_name.clone()));
            }
        }
        if search.stopped {
            return SearchResult { project, retain };
        }
        if project.is_some() {
            fallback = Some(SearchResult {
                project,
                retain: retain.clone(),
            });
        }
        let (config, ok) = configs.load(&(self.to_path)(config_file_name.clone()));
        if ok
            && config.is_some_and(|config| {
                config
                    .compiler_options()
                    .disable_solution_searching
                    .is_true()
            })
        {
            return fallback.unwrap_or_default();
        }
        if let Some(logger) = logger {
            let ancestor_config_name = self
                .config_file_registry_builder
                .get_ancestor_config_file_name(file_name, path.clone(), &config_file_name, logger);
            if !ancestor_config_name.is_empty() {
                return self.find_or_create_default_configured_project_worker(
                    file_name,
                    path,
                    ancestor_config_name.clone(),
                    load_kind,
                    Some(visited),
                    fallback,
                    Some(&logger.fork(format!(
                        "Searching ancestor config file at {ancestor_config_name}"
                    ))),
                );
            }
        }
        if let Some(fallback) = fallback {
            return fallback;
        }
        visited.range(|node| {
            retain.add((self.to_path)(node.config_file_name.clone()));
            true
        });
        SearchResult {
            project: None,
            retain,
        }
    }

    pub fn find_or_create_default_configured_project_for_file(
        &mut self,
        file_name: &str,
        path: tspath::Path,
        load_kind: ProjectLoadKind,
        logger: Option<&LogTree>,
    ) -> SearchResult {
        if let Some(key) = self.file_default_projects.get(&path) {
            if key == INFERRED_PROJECT_NAME {
                // The file belongs to the inferred project
                return SearchResult::default();
            }
            let entry = self.configured_projects.load(key.clone());
            return SearchResult {
                project: entry,
                retain: collections::Set::default(),
            };
        }
        let default_log_tree = crate::logging::new_log_tree(String::new());
        let config_file_name = self
            .config_file_registry_builder
            .get_config_file_name_for_file(
                file_name,
                path.clone(),
                logger.unwrap_or_else(|| default_log_tree.as_ref()),
            );
        if !config_file_name.is_empty() {
            let start_time = Instant::now();
            let result = self.find_or_create_default_configured_project_worker(
                file_name,
                path.clone(),
                config_file_name,
                load_kind,
                None,
                None,
                logger,
            );
            if let Some(project) = &result.project {
                self.file_default_projects.insert(
                    path,
                    project.lock().unwrap().value().config_file_path.clone(),
                );
            }
            if let Some(logger) = logger {
                if let Some(project) = &result.project {
                    let project = project.lock().unwrap().value();
                    logger.logf(format!(
                        "Found default configured project for {}: {} (in {:?})",
                        file_name,
                        project.config_file_name,
                        start_time.elapsed()
                    ));
                } else {
                    logger.logf(format!(
                        "No default configured project found for {} (searched in {:?})",
                        file_name,
                        start_time.elapsed()
                    ));
                }
            }
            return result;
        }
        SearchResult::default()
    }

    pub fn find_or_create_project(
        &mut self,
        config_file_name: &str,
        config_file_path: tspath::Path,
        load_kind: ProjectLoadKind,
        logger: Option<&LogTree>,
    ) -> Option<ProjectEntry> {
        if load_kind == ProjectLoadKind::Find {
            return self.configured_projects.load(config_file_path);
        }
        let project = new_configured_project(
            config_file_name.to_string(),
            config_file_path.clone(),
            self.clone(),
            logger,
        );
        let (entry, _) = self.configured_projects.load_or_store(
            config_file_path.clone(),
            self.configured_projects
                .new_entry(config_file_path, project),
        );
        Some(entry)
    }

    pub fn update_inferred_project_roots(
        &mut self,
        mut root_file_names: Vec<String>,
        logger: Option<&LogTree>,
    ) -> bool {
        if root_file_names.is_empty() {
            if !self.inferred_project.value().config_file_name.is_empty() {
                if let Some(logger) = logger {
                    logger.logf("Deleting inferred project".to_string());
                }
                self.inferred_project.delete();
                return true;
            }
            return false;
        }

        root_file_names.sort();
        if self.inferred_project.value().config_file_name.is_empty() {
            self.inferred_project.set(new_inferred_project(
                self.session_options.current_directory.clone(),
                self.compiler_options_for_inferred_projects.clone(),
                root_file_names,
                self.clone(),
                logger,
            ));
        } else {
            let mut project = self.inferred_project.value();
            let mut new_compiler_options =
                project.command_line.as_ref().unwrap().compiler_options();
            if let Some(options) = &self.compiler_options_for_inferred_projects {
                new_compiler_options = options.clone();
            }
            let new_command_line = tsoptions::new_parsed_command_line(
                new_compiler_options,
                root_file_names.clone(),
                tspath::ComparePathsOptions {
                    use_case_sensitive_file_names: self.fs.fs().use_case_sensitive_file_names(),
                    current_directory: self.session_options.current_directory.clone(),
                    ..Default::default()
                },
            );
            if project.command_line.as_ref().is_some_and(|command_line| {
                command_line.file_names_by_path() == new_command_line.file_names_by_path()
            }) {
                return false;
            }
            if let Some(logger) = logger {
                logger.logf(format!(
                    "Updating inferred project config with {} root files",
                    root_file_names.len()
                ));
            }
            project.command_line = Some(new_command_line);
            project.command_line_with_typings_files = None;
            project.dirty = true;
            project.dirty_file_path = tspath::Path::default();
            self.inferred_project.set(project);
        }
        true
    }

    pub fn update_inferred_project(&mut self, logger: &LogTree) -> bool {
        if self.inferred_project.value().config_file_name.is_empty() {
            return false;
        }

        let inferred = self.configured_projects.new_entry(
            INFERRED_PROJECT_NAME.to_string(),
            self.inferred_project.value(),
        );
        let files_changed = self.update_program(inferred.clone(), logger);
        self.inferred_project.set(inferred.lock().unwrap().value());
        files_changed
    }

    // updateProgram updates the program for the given project entry if necessary. It returns
    // a boolean indicating whether the update could have caused any structure-affecting changes.
    pub fn update_program(&mut self, entry: ProjectEntry, logger: &LogTree) -> bool {
        let mut update_program = false;
        let mut delete_project = false;
        let mut files_changed = false;
        let config_file_name = entry.lock().unwrap().value().config_file_name;
        let start_time = Instant::now();
        let mut notified_loading = false;
        let mut display_name = String::new();

        {
            let mut entry = entry.lock().unwrap();
            let value = entry.value();
            if value.kind == Kind::Configured {
                let acquire_logger = logger.fork("Acquiring config for project");
                let command_line = self
                    .config_file_registry_builder
                    .acquire_config_for_project(
                        &value.config_file_name,
                        value.config_file_path.clone(),
                        &value.config_file_path,
                        &acquire_logger,
                    );
                if command_line.is_none() {
                    delete_project = true;
                    files_changed = true;
                } else if value.command_line != command_line {
                    update_program = true;
                    entry.change(|p| {
                        p.command_line = command_line;
                        p.command_line_with_typings_files = None;
                        p.potential_project_references = None;
                    });
                }
            }
            if !update_program {
                update_program = entry.value().dirty;
            }
            if update_program && self.client.is_some() {
                display_name = entry
                    .value()
                    .display_name(&self.session_options.current_directory);
                notified_loading = true;
            }
        }

        if notified_loading {
            if let Some(client) = self.client.as_ref() {
                client.progress_start(&diagnostics::Project_0, &[display_name.clone()]);
            }
        }
        if delete_project {
            self.delete_configured_project(entry.clone(), Some(logger));
        }
        if update_program {
            let host_logger = logger.fork("CompilerHost");
            entry.lock().unwrap().change(|project| {
                let old_host = project.host.clone();
                project.host = Some(std::sync::Arc::new(new_compiler_host(
                    project.current_directory.clone(),
                    project.config_file_path.clone(),
                    self,
                    host_logger,
                )));
                let result = project.create_program();
                project.program = Some(result.program);
                project.checker_pool = result.checker_pool;
                project.program_update_kind = result.update_kind;
                project.program_last_update = self.new_snapshot_id;
                if result.update_kind == ProgramUpdateKind::Cloned {
                    if let (Some(host), Some(old_host)) = (&project.host, old_host) {
                        host.set_seen_files(old_host.source_fs.seen_files());
                    }
                }
                if result.update_kind == ProgramUpdateKind::NewFiles {
                    files_changed = true;
                    project.program_files_watch = Some(project.clone_watchers());
                }
                project.dirty = false;
                project.dirty_file_path = tspath::Path::default();
            });
        }
        if notified_loading {
            if let Some(client) = self.client.as_ref() {
                client.progress_finish(&diagnostics::Project_0, &[display_name]);
            }
        }
        if update_program {
            logger.logf(format!(
                "Program update for {} completed in {:?}",
                config_file_name,
                start_time.elapsed()
            ));
        }
        files_changed
    }

    pub fn mark_files_changed(
        &mut self,
        entry: ProjectEntry,
        paths: Vec<tspath::Path>,
        change_type: lsproto::FileChangeType,
        logger: &LogTree,
    ) -> bool {
        let dirty = std::cell::Cell::new(false);
        let dirty_file_path = std::cell::RefCell::new(tspath::Path::default());
        entry.lock().unwrap().change_if(
            |p| {
                if p.program.is_none() || p.dirty && p.dirty_file_path.is_empty() {
                    return false;
                }
                *dirty_file_path.borrow_mut() = p.dirty_file_path.clone();
                for path in &paths {
                    if p.contains_file(path.clone()) {
                        dirty.set(true);
                        if change_type == lsproto::FileChangeType::DELETED
                            || tspath::get_base_file_name(path) == "package.json"
                        {
                            *dirty_file_path.borrow_mut() = tspath::Path::default();
                            break;
                        }
                        if dirty_file_path.borrow().is_empty() {
                            *dirty_file_path.borrow_mut() = path.clone();
                        } else if *dirty_file_path.borrow() != *path {
                            *dirty_file_path.borrow_mut() = tspath::Path::default();
                            break;
                        }
                    } else if p.host.as_ref().is_some_and(|host| {
                        (change_type == lsproto::FileChangeType::CREATED
                            && host
                                .source_fs
                                .seen_file_or_missing_parent_directory(path.clone()))
                            || (change_type != lsproto::FileChangeType::CREATED
                                && host.source_fs.seen_file(path))
                    }) {
                        dirty.set(true);
                        *dirty_file_path.borrow_mut() = tspath::Path::default();
                        break;
                    }
                }
                dirty.get() || p.dirty_file_path != *dirty_file_path.borrow()
            },
            |p| {
                let dirty_file_path = dirty_file_path.borrow().clone();
                p.dirty = true;
                p.dirty_file_path = dirty_file_path.clone();
                if !dirty_file_path.is_empty() {
                    logger.logf(format!(
                        "Marking project {} as dirty due to changes in {}",
                        p.config_file_name, dirty_file_path
                    ));
                } else {
                    logger.logf(format!("Marking project {} as dirty", p.config_file_name));
                }
            },
        )
    }

    pub fn delete_configured_project(&mut self, project: ProjectEntry, logger: Option<&LogTree>) {
        let project_value = project.lock().unwrap().value();
        let project_path = project_value.config_file_path.clone();
        if let Some(logger) = logger {
            logger.logf(format!(
                "Deleting configured project: {}",
                project_value.config_file_name
            ));
        }
        if let Some(program) = project_value.program.as_ref() {
            program.range_resolved_project_reference(|reference_path, _, _, _| {
                self.config_file_registry_builder
                    .release_config_for_project(reference_path, project_path.clone());
                true
            });
        }
        self.config_file_registry_builder
            .release_config_for_project(project_path.clone(), project_path.clone());
        project.lock().unwrap().delete();
    }
}

pub fn log_change_file_result(result: &ChangeFileResult, logger: &LogTree) {
    if !result.affected_projects.is_empty() {
        logger.logf(format!(
            "Config file change affected projects: {:?}",
            result.affected_projects.keys().collect::<Vec<_>>()
        ));
    }
    if !result.affected_files.is_empty() {
        logger.logf(format!(
            "Config file change affected config file lookups for {} files",
            result.affected_files.len()
        ));
    }
}
