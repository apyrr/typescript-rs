use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use ts_collections as collections;
use ts_core as core;
use ts_ls as lsconv;
use ts_ls as lsutil;
use ts_ls::{AutoImportRegistry, AutoImportRegistryChange, Host, RegistryCloneHost};
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_sourcemap as sourcemap;
use ts_tspath as tspath;
use ts_vfs::vfsmatch;

use crate::client::ClientArcExt;
use crate::logging::{self, LogTree, Logger};
use crate::overlayfs::{FileHandle, Overlay};
use crate::{
    ConfigFileRegistry, FileChangeSummary, ProgramUpdateKind, Project, ProjectCollection, Session,
    SessionOptions, SnapshotFs, UpdateReason, WatchedFiles, new_auto_import_registry_clone_host,
    new_parse_cache_key, new_project_collection_builder, new_snapshot_fs_builder,
};

pub(crate) struct Snapshot {
    id: u64,
    parent_id: u64,
    ref_count: AtomicI32,

    // Session options are immutable for the server lifetime.
    pub(crate) session_options: SessionOptions,
    // toPath func(fileName string) tspath.Path
    pub(crate) to_path_current_directory: String,
    pub(crate) converters: Option<lsconv::Converters>,

    // Immutable state, cloned between snapshots.
    pub(crate) fs: Arc<SnapshotFs>,
    pub(crate) project_collection: ProjectCollection,
    pub(crate) config_file_registry: Option<ConfigFileRegistry>,
    pub(crate) auto_imports: Option<AutoImportRegistry>,
    pub(crate) auto_imports_watch: Option<WatchedFiles<HashMap<tspath::Path, String>>>,
    pub(crate) compiler_options_for_inferred_projects: Option<core::CompilerOptions>,
    pub(crate) user_preferences: lsutil::UserPreferences,
    pub(crate) builder_logs: Option<Arc<LogTree>>,
    pub(crate) api_error: Option<Box<dyn Error + Send + Sync>>,
}

pub struct SnapshotHandle {
    snapshot: Snapshot,
}

impl SnapshotHandle {
    pub(crate) fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub(crate) fn same_snapshot_identity(&self, other: &Self) -> bool {
        self.snapshot.id == other.snapshot.id
    }

    pub(crate) fn take(self) -> Snapshot {
        self.snapshot
    }

    pub fn id(&self) -> u64 {
        self.snapshot.id()
    }

    pub fn parent_id(&self) -> u64 {
        self.snapshot.parent_id()
    }

    pub(crate) fn get_default_project(&self, uri: lsproto::DocumentUri) -> Option<&Project> {
        self.snapshot.get_default_project(uri)
    }

    pub(crate) fn get_project_by_path(&self, project_path: tspath::Path) -> Option<&Project> {
        self.snapshot
            .project_collection
            .get_project_by_path(project_path)
    }

    pub(crate) fn projects(&self) -> Vec<&Project> {
        self.snapshot.project_collection.projects()
    }

    pub(crate) fn projects_by_path(&self) -> collections::OrderedMap<tspath::Path, &Project> {
        self.snapshot.project_collection.projects_by_path()
    }

    pub(crate) fn config_file_registry(&self) -> Option<&ConfigFileRegistry> {
        self.snapshot.project_collection.config_file_registry()
    }

    pub(crate) fn builder_logs(&self) -> Option<&Arc<LogTree>> {
        self.snapshot.builder_logs()
    }

    pub(crate) fn clone_host(&self) -> SnapshotHandle {
        self.snapshot.clone_host()
    }

    pub(crate) fn r#ref(&self) {
        self.snapshot.r#ref();
    }

    pub(crate) fn deref(&self, session: &mut Session) {
        self.snapshot.deref(session);
    }

    pub(crate) fn project_collection(&self) -> &ProjectCollection {
        &self.snapshot.project_collection
    }

    pub(crate) fn converters(&self) -> Option<&lsconv::Converters> {
        self.snapshot.converters()
    }
}

impl Snapshot {
    pub(crate) fn project_collection(&self) -> &ProjectCollection {
        &self.project_collection
    }

    pub(crate) fn clone_handle(&self) -> SnapshotHandle {
        self.r#ref();
        SnapshotHandle {
            snapshot: self.clone_snapshot_value(),
        }
    }

    pub(crate) fn clone_host(&self) -> SnapshotHandle {
        SnapshotHandle {
            snapshot: self.clone_snapshot_value(),
        }
    }

    pub(crate) fn clone_snapshot_value(&self) -> Snapshot {
        Snapshot {
            id: self.id,
            parent_id: self.parent_id,
            ref_count: AtomicI32::new(self.ref_count.load(Ordering::SeqCst)),
            session_options: self.session_options.clone(),
            to_path_current_directory: self.to_path_current_directory.clone(),
            converters: self.converters.clone(),
            fs: self.fs.clone(),
            project_collection: self.project_collection.clone_collection(),
            config_file_registry: self.config_file_registry.clone(),
            auto_imports: self.auto_imports.clone(),
            auto_imports_watch: self.auto_imports_watch.clone(),
            compiler_options_for_inferred_projects: self
                .compiler_options_for_inferred_projects
                .clone(),
            user_preferences: self.user_preferences.clone(),
            builder_logs: self.builder_logs.clone(),
            api_error: None,
        }
    }

    pub fn parent_id(&self) -> u64 {
        self.parent_id
    }

    pub fn builder_logs(&self) -> Option<&Arc<LogTree>> {
        self.builder_logs.as_ref()
    }
}

impl Host for SnapshotHandle {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.snapshot.use_case_sensitive_file_names()
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.snapshot
            .get_file(path)
            .map(|file| (file.content(), true))
            .unwrap_or_else(|| (String::new(), false))
    }

    fn converters(&self) -> lsconv::Converters {
        self.snapshot.converters.as_ref().unwrap().clone()
    }

    fn get_preferences(&self, active_file: &str) -> lsutil::UserPreferences {
        self.snapshot.get_preferences(active_file)
    }

    fn get_ecma_line_info(&self, file_name: &str) -> Option<sourcemap::ECMALineInfo> {
        self.snapshot.get_ecma_line_info(file_name)
    }

    fn auto_import_registry(&self) -> Option<AutoImportRegistry> {
        self.snapshot.auto_imports.clone()
    }

    fn read_directory(
        &self,
        current_dir: &str,
        path: &str,
        extensions: &[String],
        excludes: &[String],
        includes: &[String],
        depth: i32,
    ) -> Vec<String> {
        vfsmatch::read_directory(
            self.snapshot.fs.fs().as_ref(),
            current_dir,
            path,
            extensions,
            excludes,
            includes,
            depth,
        )
    }

    fn get_directories(&self, path: &str) -> Vec<String> {
        self.snapshot.fs.get_accessible_entries(path).directories
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.snapshot.fs.fs().directory_exists(path)
    }

    fn file_exists(&self, path: &str) -> bool {
        let path_key = (self.snapshot.fs.to_path)(path);
        self.snapshot.fs.file_exists(path, &path_key)
    }
}

pub(crate) fn new_snapshot(
    id: u64,
    fs: Arc<SnapshotFs>,
    session_options: SessionOptions,
    config_file_registry: Option<ConfigFileRegistry>,
    compiler_options_for_inferred_projects: Option<core::CompilerOptions>,
    user_preferences: lsutil::UserPreferences,
    auto_imports: Option<AutoImportRegistry>,
    auto_imports_watch: Option<WatchedFiles<HashMap<tspath::Path, String>>>,
    // toPath func(fileName string) tspath.Path
    to_path_current_directory: String,
) -> Snapshot {
    let converters_fs = fs.clone();
    let converters = lsconv::new_converters(session_options.position_encoding, move |file_name| {
        converters_fs
            .get_file(file_name)
            .map(|file| file.lsp_line_map().clone())
            .unwrap_or_else(|| panic!("missing LSP line map for file: {file_name}"))
    });

    Snapshot {
        id,
        parent_id: 0,
        ref_count: AtomicI32::new(1),
        session_options,
        to_path_current_directory,
        converters: Some(converters),
        fs,
        project_collection: ProjectCollection::default(),
        config_file_registry,
        auto_imports,
        auto_imports_watch,
        compiler_options_for_inferred_projects,
        user_preferences,
        builder_logs: None,
        api_error: None,
    }
}

impl Snapshot {
    pub fn get_default_project(&self, uri: lsproto::DocumentUri) -> Option<&Project> {
        self.project_collection
            .get_default_project(uri.path(self.use_case_sensitive_file_names()))
    }

    pub fn get_projects_containing_file(
        &self,
        uri: lsproto::DocumentUri,
    ) -> Vec<&dyn ts_ls::Project> {
        let file_name = uri.file_name();
        let path = (self.project_collection.to_path)(file_name);
        self.project_collection.get_projects_containing_file(path)
    }

    pub fn get_file(&self, file_name: &str) -> Option<Arc<dyn FileHandle + Send + Sync>> {
        self.fs.get_file(file_name)
    }

    pub fn lsp_line_map(&self, file_name: &str) -> Option<lsconv::LspLineMap> {
        self.get_file(file_name)
            .map(|file| file.lsp_line_map().clone())
    }

    pub fn get_ecma_line_info(&self, file_name: &str) -> Option<sourcemap::ECMALineInfo> {
        self.get_file(file_name).map(|file| {
            sourcemap::create_ecma_line_info(
                file.ecma_line_info().text().to_string(),
                file.ecma_line_info().line_starts().to_vec(),
            )
        })
    }

    pub fn get_preferences(&self, _active_file: &str) -> lsutil::UserPreferences {
        self.user_preferences()
    }

    pub fn user_preferences(&self) -> lsutil::UserPreferences {
        self.user_preferences.clone()
    }

    pub fn converters(&self) -> Option<&lsconv::Converters> {
        self.converters.as_ref()
    }

    pub fn auto_import_registry(&self) -> Option<&AutoImportRegistry> {
        self.auto_imports.as_ref()
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.fs().use_case_sensitive_file_names()
    }

    pub fn read_file(&self, file_name: &str) -> Option<String> {
        self.get_file(file_name).map(|file| file.content())
    }

    pub fn directory_exists(&self, path: &str) -> bool {
        self.fs.fs().directory_exists(path)
    }

    pub fn file_exists(&self, path: &str) -> bool {
        self.fs.fs().file_exists(path)
    }

    pub fn get_directories(&self, path: &str) -> Vec<String> {
        self.fs.fs().get_accessible_entries(path).directories
    }

    pub fn read_directory(
        &self,
        current_dir: &str,
        path: &str,
        extensions: &[String],
        excludes: &[String],
        includes: &[String],
        depth: i32,
    ) -> Vec<String> {
        vfsmatch::read_directory(
            self.fs.fs().as_ref(),
            current_dir,
            path,
            extensions,
            excludes,
            includes,
            depth,
        )
    }

    pub(crate) fn clone_snapshot(
        &self,
        ctx: core::Context,
        mut change: SnapshotChange,
        overlays: HashMap<tspath::Path, Arc<Overlay>>,
        session: &mut Session,
    ) -> Snapshot {
        let mut logger: Option<Arc<LogTree>> = None;

        // PORT NOTE: Go logs in-progress clone details from defer/recover. The
        // Rust panic boundary belongs to Session/logger integration; the clone
        // body below preserves the ordered work and records the same log tree.
        if session.options.logging_enabled {
            logger = Some(logging::new_log_tree(format!(
                "Cloning snapshot {}",
                self.id
            )));
            let get_details = || {
                let mut details = String::new();
                if !change.resource_request.documents.is_empty() {
                    details.push_str(&format!(
                        " Documents: {:?}",
                        change.resource_request.documents
                    ));
                }
                if !change
                    .resource_request
                    .configured_project_documents
                    .is_empty()
                {
                    details.push_str(&format!(
                        " ConfiguredProjectDocuments: {:?}",
                        change.resource_request.configured_project_documents
                    ));
                }
                if !change.resource_request.projects.is_empty() {
                    details.push_str(&format!(
                        " Projects: {:?}",
                        change.resource_request.projects
                    ));
                }
                if let Some(project_tree) = &change.resource_request.project_tree {
                    details.push_str(&format!(" ProjectTree: {:?}", project_tree.projects()));
                }
                details
            };
            let logger_ref = logger.as_ref().unwrap();
            match change.reason {
                UpdateReason::DidOpenFile => {
                    logger_ref.logf(format!(
                        "Reason: DidOpenFile - {}",
                        change.file_changes.opened
                    ));
                }
                UpdateReason::DidChangeCompilerOptionsForInferredProjects => {
                    logger_ref
                        .logf("Reason: DidChangeCompilerOptionsForInferredProjects".to_string());
                }
                UpdateReason::RequestedLanguageServicePendingChanges => {
                    logger_ref.logf(format!(
                        "Reason: RequestedLanguageService (pending file changes) - {}",
                        get_details()
                    ));
                }
                UpdateReason::RequestedLanguageServiceProjectNotLoaded => {
                    logger_ref.logf(format!(
                        "Reason: RequestedLanguageService (project not loaded) - {}",
                        get_details()
                    ));
                }
                UpdateReason::RequestedLanguageServiceForFileNotOpen => {
                    logger_ref.logf(format!(
                        "Reason: RequestedLanguageService (file not open) - {}",
                        get_details()
                    ));
                }
                UpdateReason::RequestedLanguageServiceProjectDirty => {
                    logger_ref.logf(format!(
                        "Reason: RequestedLanguageService (project dirty) - {}",
                        get_details()
                    ));
                }
                UpdateReason::RequestedLoadProjectTree => {
                    logger_ref.logf(format!(
                        "Reason: RequestedLoadProjectTree - {}",
                        get_details()
                    ));
                }
                UpdateReason::IdleCleanDiskCache => {
                    logger_ref.logf("Reason: IdleCleanDiskCache".to_string());
                }
                _ => {}
            }
        }

        let builder_logs = logger.clone();
        let logger = logger.unwrap_or_else(|| logging::new_log_tree(String::new()));

        let start = Instant::now();
        let mut fs = new_snapshot_fs_builder(
            session.fs.fs.clone(),
            self.fs.overlays.clone(),
            overlays.clone(),
            self.fs.disk_files.clone(),
            self.fs.disk_directories.clone(),
            self.fs.node_modules_realpath_aliases.clone(),
            session.options.position_encoding,
            self.fs.to_path.clone(),
        );
        if change.file_changes.has_excessive_watch_events() {
            let invalidate_start = Instant::now();
            if change.file_changes.invalidate_all {
                fs.invalidate_cache();
                logger.logf(format!(
                    "InvalidateAll: invalidated file cache in {:?}",
                    invalidate_start.elapsed()
                ));
            } else if !fs.watch_changes_overlap_cache(&change.file_changes) {
                change.file_changes.changed = Default::default();
                change.file_changes.deleted = Default::default();
            } else if change
                .file_changes
                .includes_watch_change_outside_node_modules
            {
                fs.invalidate_cache();
                logger.logf(format!(
                    "Excessive watch changes detected, invalidated file cache in {:?}",
                    invalidate_start.elapsed()
                ));
            } else {
                fs.invalidate_node_modules_cache();
                logger.logf(format!(
                    "npm install detected, invalidated node_modules cache in {:?}",
                    invalidate_start.elapsed()
                ));
            }
        } else {
            change.file_changes = fs.expand_and_filter_watch_events(change.file_changes);
            change.file_changes = self.fs.expand_realpath_aliases(change.file_changes);
            fs.mark_dirty_files(&change.file_changes);
            change.file_changes = fs.convert_open_and_close_to_changes(change.file_changes);
        }

        let mut compiler_options_for_inferred_projects =
            self.compiler_options_for_inferred_projects.clone();
        if change.compiler_options_for_inferred_projects.is_some() {
            compiler_options_for_inferred_projects =
                change.compiler_options_for_inferred_projects.clone();
        }

        let mut custom_config_file_name = self
            .config_file_registry
            .as_ref()
            .map(|registry| registry.custom_config_file_name.clone())
            .unwrap_or_default();
        if let Some(new_config) = &change.new_config {
            custom_config_file_name = new_config.custom_config_file_name.clone();
        }

        let new_snapshot_id = session.snapshot_id.fetch_add(1, Ordering::SeqCst) + 1;
        let old_config_file_registry = self.config_file_registry.clone().unwrap_or_default();
        let mut project_collection_builder = new_project_collection_builder(
            ctx.clone(),
            new_snapshot_id,
            fs,
            self.project_collection.clone_collection(),
            old_config_file_registry,
            self.project_collection.api_opened_projects.clone(),
            compiler_options_for_inferred_projects.clone(),
            self.session_options.clone(),
            custom_config_file_name,
            session.parse_cache.clone(),
            session.extended_config_cache.clone(),
            session.client.as_ref().map(|client| client.clone_handle()),
        );

        if !change.ata_changes.is_empty() {
            project_collection_builder
                .did_update_ata_state(change.ata_changes, &logger.fork("DidUpdateATAState"));
        }

        project_collection_builder
            .did_change_custom_config_file_name(&logger.fork("DidChangeCustomConfigFileName"));

        if !change.file_changes.is_empty() {
            project_collection_builder
                .did_change_files(change.file_changes.clone(), &logger.fork("DidChangeFiles"));
        }

        let mut api_error = None;
        if let Some(api_request) = &change.api_request {
            api_error = project_collection_builder
                .handle_api_request(api_request, &logger.fork("HandleAPIRequest"))
                .err();
        }

        for uri in change.resource_request.documents {
            project_collection_builder.did_request_file(uri, false, &logger.fork("DidRequestFile"));
        }

        for uri in change.resource_request.configured_project_documents {
            project_collection_builder.did_request_file(
                uri,
                true,
                &logger.fork("DidRequestFile (optional)"),
            );
        }

        for project_id in change.resource_request.projects {
            project_collection_builder
                .did_request_project(project_id, &logger.fork("DidRequestProject"));
        }

        if let Some(project_tree) = &change.resource_request.project_tree {
            project_collection_builder
                .did_request_project_trees(project_tree, &logger.fork("DidRequestProjectTrees"));
        }

        let (project_collection, config_file_registry) =
            project_collection_builder.finalize(Some(&logger));

        let mut projects_with_new_program_structure = HashMap::new();
        for project in project_collection.projects() {
            if project.program_last_update == new_snapshot_id
                && project.program_update_kind != ProgramUpdateKind::Cloned
            {
                projects_with_new_program_structure.insert(
                    project.config_file_path.clone(),
                    project.program_update_kind == ProgramUpdateKind::NewFiles,
                );
            }
        }

        let should_clean_disk_cache = change.clean_disk_cache
            || !change.file_changes.opened.is_empty()
            || !change.file_changes.reopened.is_empty()
            || !change.file_changes.closed.is_empty()
            || !change.file_changes.deleted.is_empty();
        if should_clean_disk_cache
            && (!projects_with_new_program_structure.is_empty() || change.clean_disk_cache)
        {
            let clean_files_start = Instant::now();
            let mut removed_files = 0;
            project_collection_builder.fs.disk_files.range(|entry| {
                for project in project_collection.projects() {
                    if project
                        .host
                        .as_ref()
                        .is_some_and(|host| host.source_fs.seen_file(entry.key()))
                    {
                        return true;
                    }
                }
                entry.delete();
                removed_files += 1;
                true
            });
            if session.options.logging_enabled {
                logger.logf(format!(
                    "Removed {} cached file(s) in {:?}",
                    removed_files,
                    clean_files_start.elapsed()
                ));
            }
        }

        let mut config = self.user_preferences.clone();
        if let Some(new_config) = change.new_config {
            config = new_config;
        }

        let auto_import_host = new_auto_import_registry_clone_host(
            project_collection.clone_collection(),
            session.parse_cache.clone(),
            project_collection_builder.fs.clone(),
            self.session_options.current_directory.clone(),
            self.fs.to_path.clone(),
        );
        let registry_to_path = self.fs.to_path.clone();
        let mut open_files = HashMap::with_capacity(overlays.len());
        for (path, overlay) in overlays {
            open_files.insert(path, overlay.file_name());
        }
        let mut prepare_auto_imports = tspath::Path::default();
        if !change.resource_request.auto_imports.is_empty() {
            prepare_auto_imports = change
                .resource_request
                .auto_imports
                .path(self.use_case_sensitive_file_names());
            open_files
                .entry(prepare_auto_imports.clone())
                .or_insert_with(|| change.resource_request.auto_imports.file_name());
        }
        let mut old_auto_imports = self.auto_imports.clone();
        if old_auto_imports.is_none() {
            old_auto_imports = Some(ts_ls::new_auto_import_registry(
                {
                    let registry_to_path = registry_to_path.clone();
                    move |file_name: String| registry_to_path(&file_name)
                },
                self.user_preferences.clone(),
            ));
        }
        let mut auto_imports_watch = None;
        let auto_import_result = old_auto_imports.unwrap().clone_registry(
            ctx,
            AutoImportRegistryChange::new(
                prepare_auto_imports,
                open_files,
                collections::new_set_from_items(change.file_changes.changed),
                collections::new_set_from_items(change.file_changes.created),
                collections::new_set_from_items(change.file_changes.deleted),
                projects_with_new_program_structure,
                Some(config.clone()),
            ),
            &auto_import_host,
            Some(&logger.fork("UpdateAutoImports")),
        );
        let auto_imports = match auto_import_result {
            Ok(auto_imports) => auto_imports,
            Err(err) => {
                api_error = Some(err.into());
                ts_ls::new_auto_import_registry(
                    {
                        let registry_to_path = registry_to_path.clone();
                        move |file_name: String| registry_to_path(&file_name)
                    },
                    self.user_preferences.clone(),
                )
            }
        };
        if api_error.is_none() {
            auto_imports_watch = self
                .auto_imports_watch
                .as_ref()
                .map(|watch| watch.clone_with(auto_imports.node_modules_directories()));
        }

        let (snapshot_fs, _) = project_collection_builder.fs.finalize();
        let mut new_snapshot = new_snapshot(
            new_snapshot_id,
            Arc::new(snapshot_fs),
            self.session_options.clone(),
            None,
            compiler_options_for_inferred_projects,
            config,
            Some(auto_imports),
            auto_imports_watch,
            self.to_path_current_directory.clone(),
        );
        new_snapshot.parent_id = self.id;
        new_snapshot.project_collection = project_collection;
        new_snapshot.config_file_registry = Some(config_file_registry);
        new_snapshot.builder_logs = builder_logs;
        new_snapshot.api_error = api_error;

        for project in new_snapshot.project_collection.projects() {
            if let Some(program) = project.program.as_ref() {
                session.program_counter.r#ref(Arc::as_ptr(program));
                if project.program_last_update == new_snapshot_id {
                    project.host.as_ref().unwrap().freeze(
                        (*new_snapshot.fs).clone(),
                        new_snapshot.config_file_registry.as_ref().unwrap().clone(),
                    );
                }
            }
        }
        for config in new_snapshot
            .config_file_registry
            .as_ref()
            .unwrap()
            .configs
            .values()
        {
            if let Some(command_line) = &config.command_line {
                for file in command_line.extended_source_files() {
                    session
                        .extended_config_cache
                        .add_owner(&(new_snapshot.fs.to_path)(file), new_snapshot.id);
                }
            }
        }

        auto_import_host.dispose();

        if session.options.logging_enabled {
            logger.logf(format!(
                "Finished cloning snapshot {} into snapshot {} in {:?}",
                self.id,
                new_snapshot.id,
                start.elapsed()
            ));
        }
        new_snapshot
    }

    pub(crate) fn r#ref(&self) {
        let rc = self.ref_count.fetch_add(1, Ordering::SeqCst) + 1;
        if rc <= 1 {
            panic!(
                "snapshot {}: ref on disposed snapshot, parentId={}",
                self.id, self.parent_id
            );
        }
    }

    pub(crate) fn try_ref(&self) -> bool {
        loop {
            let rc = self.ref_count.load(Ordering::SeqCst);
            if rc <= 0 {
                return false;
            }
            if self
                .ref_count
                .compare_exchange(rc, rc + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return true;
            }
        }
    }

    pub(crate) fn deref(&self, session: &mut Session) {
        let rc = self.ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
        if rc < 0 {
            panic!(
                "snapshot {}: ref count below zero, parentId={}",
                self.id, self.parent_id
            );
        }
        if rc == 0 {
            self.dispose(session);
        }
    }

    fn dispose(&self, session: &mut Session) {
        for project in self.project_collection.projects() {
            if let Some(program) = project.program.as_ref() {
                if session.program_counter.deref(Arc::as_ptr(program)) {
                    for file in program.source_files() {
                        session.parse_cache.deref(&new_parse_cache_key(
                            file.parse_options(),
                            file.hash(),
                            file.script_kind(),
                        ));
                    }
                    for file in program.duplicate_source_files() {
                        session.parse_cache.deref(&new_parse_cache_key(
                            file.parse_options,
                            file.hash,
                            file.script_kind,
                        ));
                    }
                }
            }
        }
        if let Some(config_file_registry) = &self.config_file_registry {
            for config in config_file_registry.configs.values() {
                if let Some(command_line) = &config.command_line {
                    for file in command_line.extended_source_files() {
                        session
                            .extended_config_cache
                            .release(&session.to_path(file), self.id);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct ApiSnapshotRequest {
    pub open_projects: Option<collections::Set<String>>,
    pub close_projects: Option<collections::Set<tspath::Path>>,
}

#[derive(Clone, Default)]
pub struct ProjectTreeRequest {
    // If None, all project trees need to be loaded; otherwise only those
    // referenced by this set need to be loaded.
    pub referenced_projects: Option<collections::Set<tspath::Path>>,
}

impl ProjectTreeRequest {
    pub fn is_all_projects(&self) -> bool {
        self.referenced_projects.is_none()
    }

    pub fn is_project_referenced(&self, project_id: &tspath::Path) -> bool {
        self.referenced_projects
            .as_ref()
            .is_some_and(|projects| projects.has(project_id))
    }

    pub fn projects(&self) -> Vec<tspath::Path> {
        self.referenced_projects
            .as_ref()
            .and_then(|projects| projects.keys().map(|keys| keys.iter().cloned().collect()))
            .unwrap_or_default()
    }
}

#[derive(Clone, Default)]
pub struct ResourceRequest {
    pub documents: Vec<lsproto::DocumentUri>,
    pub configured_project_documents: Vec<lsproto::DocumentUri>,
    pub projects: Vec<tspath::Path>,
    pub project_tree: Option<ProjectTreeRequest>,
    pub auto_imports: lsproto::DocumentUri,
}

#[derive(Clone, Default)]
pub struct SnapshotChange {
    pub resource_request: ResourceRequest,
    pub reason: UpdateReason,
    pub file_changes: FileChangeSummary,
    pub compiler_options_for_inferred_projects: Option<core::CompilerOptions>,
    pub new_config: Option<lsutil::UserPreferences>,
    // ataChanges map[tspath.Path]*ATAStateChange
    pub ata_changes: HashMap<tspath::Path, AtaStateChange>,
    pub api_request: Option<ApiSnapshotRequest>,
    pub clean_disk_cache: bool,
}

#[derive(Clone, Default)]
pub struct AtaStateChange {
    pub project_id: tspath::Path,
    pub typings_info: Option<crate::ata::TypingsInfo>,
    pub typings_files: Vec<String>,
    pub typings_files_to_watch: Vec<String>,
    // Logs *logging.LogTree
    pub logs: Option<()>,
}
