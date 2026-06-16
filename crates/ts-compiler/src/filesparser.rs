use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ts_ast as ast;
use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_module as module;
use ts_tracing as tracing;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::file_include::ReferencedFileData;
use crate::{
    DuplicateSourceFile, FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE, FileIncludeReason, FileLoader,
    IncludeExplainingDiagnostic, IncludeProcessor, JsxRuntimeImportSpecifier, LibFile,
    ProcessedFiles, ProcessedProgramFiles, ProcessingDiagnostic, ProcessingDiagnosticData,
    ProcessingDiagnosticKind, RedirectsFile,
};

fn duplicate_source_file(file: &ast::ParsedSourceFile) -> DuplicateSourceFile {
    DuplicateSourceFile {
        parse_options: file.parse_options(),
        hash: file.hash(),
        script_kind: file.script_kind(),
    }
}

fn finish_trace(
    pop_trace: Option<Box<dyn FnOnce(&mut tracing::Tracing)>>,
    loader: &mut FileLoader,
) {
    if let Some(pop_trace) = pop_trace {
        if let Some(tracing) = loader.opts.tracing.as_mut() {
            pop_trace(tracing);
        }
    }
}

fn trace_resolution(loader: &FileLoader, trace: &module::DiagAndArgs) {
    loader.opts.host.trace_text(&trace.message);
}

pub(crate) type ParseTaskRef = Arc<Mutex<ParseTask>>;

pub(crate) fn new_parse_task(task: ParseTask) -> ParseTaskRef {
    Arc::new(Mutex::new(task))
}

#[derive(Default)]
pub(crate) struct ParseTask {
    pub(crate) normalized_file_path: String,
    pub(crate) path: tspath::Path,
    pub(crate) file: Option<ast::ParsedSourceFile>,
    pub(crate) lib_file: Option<LibFile>,
    pub(crate) redirected_parse_task: Option<ParseTaskRef>,
    pub(crate) sub_tasks: Vec<ParseTaskRef>,
    pub(crate) loaded: bool,
    pub(crate) started_sub_tasks: bool,
    pub(crate) is_for_automatic_type_directive: bool,
    pub(crate) include_reason: Option<FileIncludeReason>,
    pub(crate) package_id: module::PackageId,

    pub(crate) metadata: ast::SourceFileMetaData,
    pub(crate) resolutions_in_file: module::ModeAwareCache<module::ResolvedModule>,
    pub(crate) resolutions_trace: Vec<module::DiagAndArgs>,
    pub(crate) type_resolutions_in_file:
        module::ModeAwareCache<module::ResolvedTypeReferenceDirective>,
    pub(crate) type_resolutions_trace: Vec<module::DiagAndArgs>,
    pub(crate) resolution_diagnostics: Vec<ast::Diagnostic>,
    pub(crate) processing_diagnostics: Vec<ProcessingDiagnostic>,
    pub(crate) import_helpers_import_specifier: Option<ast::StringLiteralNode>,
    pub(crate) jsx_runtime_import_specifier: Option<JsxRuntimeImportSpecifier>,

    pub(crate) increase_depth: bool,
    pub(crate) elide_on_depth: bool,

    pub(crate) loaded_task: Option<ParseTaskRef>,
    pub(crate) all_include_reasons: Vec<FileIncludeReason>,
}

impl ParseTask {
    fn file_name(&self) -> String {
        self.normalized_file_path.clone()
    }

    fn path(&self) -> tspath::Path {
        self.path.clone()
    }

    fn load(&mut self, loader: &mut FileLoader) {
        self.loaded = true;
        if self.is_for_automatic_type_directive {
            self.load_automatic_type_directives(loader);
            return;
        }
        let pop_trace = loader.opts.tracing.as_mut().map(|tracing| {
            tracing.push(
                tracing::PHASE_PROGRAM,
                "findSourceFile",
                hashmap! {"fileName" => self.normalized_file_path.clone()},
                false,
            )
        });
        let redirect = loader
            .project_reference_file_mapper
            .get_parse_file_redirect(self);
        if !redirect.is_empty() {
            self.redirect(loader, &redirect);
            finish_trace(pop_trace, loader);
            return;
        }

        if tspath::has_extension(&self.normalized_file_path) {
            let compiler_options = loader.opts.config.compiler_options();
            let allow_non_ts_extensions = compiler_options.allow_non_ts_extensions.is_true();
            if !allow_non_ts_extensions {
                let canonical_file_name = tspath::get_canonical_file_name(
                    &self.normalized_file_path,
                    loader.opts.host.fs().use_case_sensitive_file_names(),
                );
                if !loader.is_supported_extension(&canonical_file_name) {
                    if tspath::has_js_file_extension(&canonical_file_name) {
                        self.processing_diagnostics.push(ProcessingDiagnostic {
                            kind: ProcessingDiagnosticKind::ExplainingFileInclude,
                            data: ProcessingDiagnosticData::IncludeExplaining(IncludeExplainingDiagnostic {
                                file: None,
                                diagnostic_reason: self.include_reason.clone(),
                                message: &diagnostics::File_0_is_a_JavaScript_file_Did_you_mean_to_enable_the_allowJs_option,
                                args: vec![self.normalized_file_path.clone()],
                            }),
                        });
                    } else {
                        self.processing_diagnostics.push(ProcessingDiagnostic {
                            kind: ProcessingDiagnosticKind::ExplainingFileInclude,
                            data: ProcessingDiagnosticData::IncludeExplaining(IncludeExplainingDiagnostic {
                                file: None,
                                diagnostic_reason: self.include_reason.clone(),
                                message: &diagnostics::File_0_has_an_unsupported_extension_The_only_supported_extensions_are_1,
                                args: vec![
                                    self.normalized_file_path.clone(),
                                    format!("'{}'", core::flatten(&loader.supported_extensions).join("', '")),
                                ],
                            }),
                        });
                    }
                    finish_trace(pop_trace, loader);
                    return;
                }
            }
        }

        loader
            .total_file_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.lib_file.is_some() {
            loader
                .lib_file_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Default lib files are all scripts; we can safely skip looking up their package.json
            // to avoid adding spurious lookups to file watcher tracking.
            self.metadata = ast::SourceFileMetaData {
                implied_node_format: core::ResolutionMode::CommonJs,
                ..Default::default()
            };
        } else {
            self.metadata = loader.load_source_file_meta_data(&self.normalized_file_path);
        }

        let file = loader.parse_source_file(self);
        if file.is_none() {
            finish_trace(pop_trace, loader);
            return;
        }

        let file = file.unwrap();
        let file_name = file.file_name();
        let referenced_files = file.referenced_files().to_vec();
        let lib_reference_directives = file.lib_reference_directives().to_vec();
        self.sub_tasks = Vec::with_capacity(
            referenced_files.len() + file.imports().len() + file.module_augmentations().len(),
        );
        self.file = Some(file);

        let compiler_options = loader.opts.config.compiler_options();
        if !compiler_options.no_resolve.is_true() {
            for (index, r#ref) in referenced_files.iter().enumerate() {
                let (resolved_ref, processing_diagnostic) =
                    loader.resolve_tripleslash_path_reference(&r#ref.file_name, &file_name, index);
                if let Some(processing_diagnostic) = processing_diagnostic {
                    self.processing_diagnostics.push(processing_diagnostic);
                    continue;
                }
                self.add_sub_task(resolved_ref.unwrap(), None);
            }

            loader.resolve_type_reference_directives(self);
        }

        if compiler_options.no_lib != core::TSTrue {
            for (index, lib) in lib_reference_directives.iter().enumerate() {
                let include_reason = FileIncludeReason::new(
                    FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE,
                    ReferencedFileData {
                        file: self.path.clone(),
                        index: index as isize,
                        synthetic: None,
                    },
                );
                if let Some(name) = tsoptions::get_lib_file_name(&lib.file_name) {
                    let lib_file = loader.path_for_lib_file(&name);
                    self.add_sub_task(
                        ResolvedRef {
                            file_name: lib_file.path.clone(),
                            include_reason: Some(include_reason),
                            ..ResolvedRef::default()
                        },
                        Some(lib_file),
                    );
                } else {
                    self.processing_diagnostics.push(ProcessingDiagnostic {
                        kind: ProcessingDiagnosticKind::UnknownReference,
                        data: ProcessingDiagnosticData::FileIncludeReason(include_reason),
                    });
                }
            }
        }

        loader.resolve_imports_and_module_augmentations(self);
        finish_trace(pop_trace, loader);
    }

    fn redirect(&mut self, _loader: &FileLoader, file_name: &str) {
        let redirected_parse_task = new_parse_task(ParseTask {
            normalized_file_path: tspath::normalize_path(file_name),
            lib_file: self.lib_file.clone(),
            include_reason: self.include_reason.clone(),
            ..ParseTask::default()
        });
        // increaseDepth and elideOnDepth are not copied to redirects, otherwise their depth would be double counted.
        self.sub_tasks = vec![Arc::clone(&redirected_parse_task)];
        self.redirected_parse_task = Some(redirected_parse_task);
    }

    fn load_automatic_type_directives(&mut self, loader: &mut FileLoader) {
        let pop_trace = loader.opts.tracing.as_mut().map(|tracing| {
            tracing.push(
                tracing::PHASE_PROGRAM,
                "processTypeReferences",
                HashMap::<String, tracing::Any>::new(),
                false,
            )
        });
        let (to_parse_type_refs, type_resolutions_in_file, type_resolutions_trace, p_diagnostics) =
            loader.resolve_automatic_type_directives(&self.normalized_file_path);
        self.type_resolutions_in_file = type_resolutions_in_file;
        self.type_resolutions_trace = type_resolutions_trace;
        self.processing_diagnostics.extend(p_diagnostics);
        for type_resolution in to_parse_type_refs {
            self.add_sub_task(type_resolution, None);
        }
        finish_trace(pop_trace, loader);
    }

    pub(crate) fn add_sub_task(&mut self, r#ref: ResolvedRef, lib_file: Option<LibFile>) {
        let normalized_file_path = tspath::normalize_path(&r#ref.file_name);
        let sub_task = ParseTask {
            normalized_file_path,
            lib_file,
            increase_depth: r#ref.increase_depth,
            elide_on_depth: r#ref.elide_on_depth,
            include_reason: r#ref.include_reason,
            package_id: r#ref.package_id,
            ..ParseTask::default()
        };
        self.sub_tasks.push(new_parse_task(sub_task));
    }
}

impl ast::HasFileName for ParseTask {
    fn file_name(&self) -> String {
        self.file_name()
    }

    fn path(&self) -> tspath::Path {
        self.path()
    }
}

pub(crate) struct ResolvedRef {
    pub(crate) file_name: String,
    pub(crate) increase_depth: bool,
    pub(crate) elide_on_depth: bool,
    pub(crate) include_reason: Option<FileIncludeReason>,
    pub(crate) package_id: module::PackageId,
}

impl Default for ResolvedRef {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            increase_depth: false,
            elide_on_depth: false,
            include_reason: None,
            package_id: Default::default(),
        }
    }
}

pub(crate) struct FilesParser {
    wg: Box<dyn core::WorkGroup>,
    task_data_by_path: collections::SyncMap<tspath::Path, Arc<Mutex<ParseTaskData>>>,
    max_depth: i32,
}

fn get_parse_task_data(
    task: &ParseTaskRef,
    normalized_file_path: String,
) -> Arc<Mutex<ParseTaskData>> {
    let mut td = ParseTaskData {
        tasks: HashMap::with_capacity(1),
        lowest_depth: i32::MAX,
        started_sub_tasks: false,
        package_id: Default::default(),
    };
    td.tasks.insert(normalized_file_path, Arc::clone(task));
    td.lowest_depth = i32::MAX;
    Arc::new(Mutex::new(td))
}

fn put_parse_task_data(td: Arc<Mutex<ParseTaskData>>) {
    td.lock()
        .unwrap_or_else(|err| err.into_inner())
        .tasks
        .clear();
}

struct ParseTaskData {
    // map of tasks by file casing
    tasks: HashMap<String, ParseTaskRef>,
    lowest_depth: i32,
    started_sub_tasks: bool,
    package_id: module::PackageId,
}

struct CollectFilesContext<'a> {
    include_processor: &'a mut IncludeProcessor,
    duplicate_source_files: &'a mut Vec<DuplicateSourceFile>,
    tasks_seen_by_name_ignore_case: &'a mut Option<HashMap<String, ParseTaskRef>>,
    files: &'a mut Vec<ast::SourceFile>,
    lib_files: &'a mut Vec<ast::SourceFile>,
    files_by_path_targets: &'a mut Vec<(tspath::Path, tspath::Path)>,
    output_file_to_project_reference_source: &'a mut HashMap<tspath::Path, String>,
    resolved_modules: &'a mut HashMap<tspath::Path, module::ModeAwareCache<module::ResolvedModule>>,
    type_resolutions_in_file: &'a mut HashMap<
        tspath::Path,
        module::ModeAwareCache<module::ResolvedTypeReferenceDirective>,
    >,
    source_file_meta_datas: &'a mut HashMap<tspath::Path, ast::SourceFileMetaData>,
    jsx_runtime_import_specifiers: &'a mut HashMap<tspath::Path, JsxRuntimeImportSpecifier>,
    import_helpers_import_specifiers: &'a mut HashMap<tspath::Path, ast::StringLiteralNode>,
    source_files_found_searching_node_modules: &'a mut collections::Set<tspath::Path>,
    lib_files_map: &'a mut HashMap<tspath::Path, LibFile>,
    redirect_targets_map: &'a mut HashMap<tspath::Path, Vec<String>>,
    redirect_files_by_path: &'a mut HashMap<tspath::Path, RedirectsFile>,
    package_id_to_source_file: &'a mut Vec<(module::PackageId, tspath::Path)>,
    package_deduplication_enabled: bool,
    missing_files: &'a mut Vec<String>,
}

impl FilesParser {
    pub(crate) fn new(wg: Box<dyn core::WorkGroup>, max_depth: i32) -> Self {
        Self {
            wg,
            task_data_by_path: collections::SyncMap::default(),
            max_depth,
        }
    }

    pub(crate) fn parse(&mut self, loader: &mut FileLoader, mut tasks: Vec<ParseTaskRef>) {
        self.start(loader, &mut tasks, 0);
        self.wg.run_and_wait();
    }

    fn start(&mut self, loader: &mut FileLoader, tasks: &mut [ParseTaskRef], depth: i32) {
        for i in 0..tasks.len() {
            let (normalized_file_path, path) = {
                let mut task = tasks[i].lock().unwrap_or_else(|err| err.into_inner());
                let path = loader.to_path(&task.normalized_file_path);
                task.path = path.clone();
                (task.normalized_file_path.clone(), path)
            };
            let candidate = get_parse_task_data(&tasks[i], normalized_file_path.clone());
            let (data, loaded) = self
                .task_data_by_path
                .load_or_store(path, Some(candidate.clone()));
            let data = data.unwrap();
            if loaded {
                put_parse_task_data(candidate);
            }

            let mut pending_subtasks = Vec::new();
            {
                let mut data = data.lock().unwrap_or_else(|err| err.into_inner());

                let mut start_subtasks = false;
                if loaded {
                    if let Some(existing_task) = data.tasks.get(&normalized_file_path) {
                        tasks[i]
                            .lock()
                            .unwrap_or_else(|err| err.into_inner())
                            .loaded_task = Some(Arc::clone(existing_task));
                    } else {
                        data.tasks
                            .insert(normalized_file_path.clone(), Arc::clone(&tasks[i]));
                        // This is new task for file name - so load subtasks if there was loading for any other casing
                        start_subtasks = data.started_sub_tasks;
                    }
                }

                // Propagate packageId to data if we have one and data doesn't yet
                let (package_id, increase_depth, elide_on_depth) = {
                    let task = tasks[i].lock().unwrap_or_else(|err| err.into_inner());
                    (
                        task.package_id.clone(),
                        task.increase_depth,
                        task.elide_on_depth,
                    )
                };
                if data.package_id.name.is_empty() && !package_id.name.is_empty() {
                    data.package_id = package_id;
                }

                let current_depth = if increase_depth { depth + 1 } else { depth };
                if current_depth < data.lowest_depth {
                    // If we're seeing this task at a lower depth than before,
                    // reprocess its subtasks to ensure they are loaded.
                    data.lowest_depth = current_depth;
                    start_subtasks = true;
                    data.started_sub_tasks = true;
                }

                if !(elide_on_depth && current_depth > self.max_depth) {
                    // PORT NOTE: queued closure body is executed inline so parsed task state can be
                    // written back through the shared task handles used by this Rust port.
                    let task_refs: Vec<_> = data.tasks.values().cloned().collect();
                    for task_ref in task_refs {
                        let mut task_by_file_name =
                            task_ref.lock().unwrap_or_else(|err| err.into_inner());
                        let mut load_sub_tasks = start_subtasks;
                        if !task_by_file_name.loaded {
                            task_by_file_name.load(loader);
                            if task_by_file_name.redirected_parse_task.is_some() {
                                // Always load redirected task
                                load_sub_tasks = true;
                                data.started_sub_tasks = true;
                            }
                        }
                        if !task_by_file_name.started_sub_tasks && load_sub_tasks {
                            task_by_file_name.started_sub_tasks = true;
                            pending_subtasks.push((
                                Arc::clone(&task_ref),
                                task_by_file_name.sub_tasks.clone(),
                                data.lowest_depth,
                                task_by_file_name.redirected_parse_task.is_some(),
                            ));
                        }
                    }
                }
            }

            for (task_ref, mut sub_tasks, lowest_depth, is_redirect) in pending_subtasks {
                self.start(loader, &mut sub_tasks, lowest_depth);
                let mut task_by_file_name = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                if is_redirect {
                    task_by_file_name.redirected_parse_task = sub_tasks.first().cloned();
                }
                task_by_file_name.sub_tasks = sub_tasks;
            }
        }
    }

    pub(crate) fn get_processed_files(&mut self, loader: &mut FileLoader) -> ProcessedProgramFiles {
        let total_file_count = loader
            .total_file_count
            .load(std::sync::atomic::Ordering::SeqCst) as usize;
        let lib_file_count = loader
            .lib_file_count
            .load(std::sync::atomic::Ordering::SeqCst) as usize;

        let mut missing_files = Vec::new();
        let mut duplicate_source_files = Vec::new();
        let mut files = Vec::with_capacity(total_file_count - lib_file_count);
        let mut lib_files = Vec::with_capacity(total_file_count); // totalFileCount here since we append files to it later to construct the final list

        let mut files_by_path_targets = Vec::with_capacity(total_file_count);
        // stores 'filename -> file association' ignoring case
        // used to track cases when two file names differ only in casing
        let mut tasks_seen_by_name_ignore_case: Option<HashMap<String, ParseTaskRef>> = None;
        if loader.compare_paths_options.use_case_sensitive_file_names {
            tasks_seen_by_name_ignore_case = Some(HashMap::with_capacity(total_file_count));
        }

        let mut include_processor = IncludeProcessor {
            file_include_reasons: HashMap::with_capacity(total_file_count),
            ..Default::default()
        };
        let mut output_file_to_project_reference_source = HashMap::new();
        if !loader.opts.can_use_project_reference_source() {
            output_file_to_project_reference_source = HashMap::with_capacity(total_file_count);
        }
        let mut resolved_modules = HashMap::with_capacity(total_file_count + 1);
        let mut type_resolutions_in_file = HashMap::with_capacity(total_file_count);
        let mut source_file_meta_datas = HashMap::with_capacity(total_file_count);
        let mut jsx_runtime_import_specifiers: HashMap<tspath::Path, JsxRuntimeImportSpecifier> =
            HashMap::new();
        let mut import_helpers_import_specifiers: HashMap<tspath::Path, ast::StringLiteralNode> =
            HashMap::new();
        let mut source_files_found_searching_node_modules = collections::Set::new();
        let mut lib_files_map = HashMap::with_capacity(lib_file_count);

        let mut redirect_targets_map = HashMap::new();
        let mut redirect_files_by_path = HashMap::new();
        let mut package_id_to_source_file = Vec::new();
        let package_deduplication_enabled = !loader
            .opts
            .config
            .compiler_options()
            .deduplicate_packages
            .is_false();
        if package_deduplication_enabled {
            redirect_targets_map = HashMap::new();
            package_id_to_source_file = Vec::new();
        }

        let mut collect_context = CollectFilesContext {
            include_processor: &mut include_processor,
            duplicate_source_files: &mut duplicate_source_files,
            tasks_seen_by_name_ignore_case: &mut tasks_seen_by_name_ignore_case,
            files: &mut files,
            lib_files: &mut lib_files,
            files_by_path_targets: &mut files_by_path_targets,
            output_file_to_project_reference_source: &mut output_file_to_project_reference_source,
            resolved_modules: &mut resolved_modules,
            type_resolutions_in_file: &mut type_resolutions_in_file,
            source_file_meta_datas: &mut source_file_meta_datas,
            jsx_runtime_import_specifiers: &mut jsx_runtime_import_specifiers,
            import_helpers_import_specifiers: &mut import_helpers_import_specifiers,
            source_files_found_searching_node_modules:
                &mut source_files_found_searching_node_modules,
            lib_files_map: &mut lib_files_map,
            redirect_targets_map: &mut redirect_targets_map,
            redirect_files_by_path: &mut redirect_files_by_path,
            package_id_to_source_file: &mut package_id_to_source_file,
            package_deduplication_enabled,
            missing_files: &mut missing_files,
        };
        let root_tasks = loader.root_tasks.clone();
        self.collect_files(
            &root_tasks,
            &mut HashMap::with_capacity(total_file_count),
            loader,
            &mut collect_context,
        );
        drop(collect_context);
        loader.sort_libs(&mut lib_files);

        let lib_files_len = lib_files.len();
        let mut all_files = lib_files;
        all_files.extend(files);
        let mut file_indices_by_path = HashMap::with_capacity(all_files.len());
        for (index, file) in all_files.iter().enumerate() {
            file_indices_by_path.insert(file.path(), index);
        }
        let mut files_by_path = HashMap::with_capacity(total_file_count);
        for (index, file) in all_files.iter().enumerate() {
            files_by_path.insert(file.path(), index);
        }
        for (path, target) in files_by_path_targets {
            if let Some(index) = file_indices_by_path.get(&target) {
                files_by_path.insert(path, *index);
            }
        }
        for redirect_file in redirect_files_by_path.values_mut() {
            redirect_file.index += lib_files_len;
        }

        let mut keys = loader.path_for_lib_file_resolutions.keys();
        keys.sort();
        for key in keys {
            let (value, _) = loader.path_for_lib_file_resolutions.load(&key);
            let value = value.unwrap();
            resolved_modules.insert(
                key,
                module::ModeAwareCache::from([(
                    module::ModeAwareCacheKey {
                        name: value.library_name.clone(),
                        mode: core::ModuleKind::CommonJs,
                    },
                    value.resolution.clone(),
                )]),
            );
            for trace in &value.trace {
                trace_resolution(loader, trace);
            }
        }

        ProcessedProgramFiles {
            processed_files: ProcessedFiles {
                finished_processing: true,
                resolver: Arc::new(Mutex::new(std::mem::take(&mut loader.resolver))),
                duplicate_source_files,
                files_by_path,
                project_reference_file_mapper: std::mem::take(
                    &mut loader.project_reference_file_mapper,
                ),
                resolved_modules,
                type_resolutions_in_file,
                source_file_meta_datas,
                jsx_runtime_import_specifiers,
                import_helpers_import_specifiers,
                source_files_found_searching_node_modules,
                lib_files: lib_files_map,
                missing_files,
                include_processor,
                output_file_to_project_reference_source,
                redirect_targets_map,
                redirect_files_by_path,
            },
            source_files: all_files,
        }
    }

    fn collect_files(
        &self,
        tasks: &[ParseTaskRef],
        seen: &mut HashMap<usize, String>,
        loader: &mut FileLoader,
        context: &mut CollectFilesContext<'_>,
    ) {
        for task_ref in tasks {
            let mut task_ref = Arc::clone(task_ref);
            let (
                include_reason,
                redirected_parse_task,
                is_for_automatic_type_directive,
                loaded_task,
            ) = {
                let task = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                (
                    task.include_reason.clone(),
                    task.redirected_parse_task.clone(),
                    task.is_for_automatic_type_directive,
                    task.loaded_task.clone(),
                )
            };
            // Exclude automatic type directive tasks from include reason processing,
            // as these are internal implementation details and should not contribute
            // to the reasons for including files.
            if redirected_parse_task.is_none() && !is_for_automatic_type_directive {
                if let Some(loaded_task) = loaded_task {
                    task_ref = loaded_task;
                }
                self.add_include_reason(
                    context.include_processor,
                    &task_ref,
                    include_reason.clone(),
                );
            }
            let (path, loaded) = {
                let task = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                (task.path.clone(), task.loaded)
            };
            let (data, ok) = self.task_data_by_path.load(&path);
            if !ok {
                continue;
            }
            let data = data.unwrap();
            if !loaded {
                continue;
            }
            let data_key = Arc::as_ptr(&data) as usize;
            let (package_id, lowest_depth) = {
                let data = data.lock().unwrap_or_else(|err| err.into_inner());
                (data.package_id.clone(), data.lowest_depth)
            };

            // ensure we only walk each task once
            if let Some(checked_name) = seen.get(&data_key).cloned() {
                let (normalized_file_path, duplicate_source_file) = {
                    let task = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                    (
                        task.normalized_file_path.clone(),
                        task.file.as_ref().map(duplicate_source_file),
                    )
                };
                if let Some(duplicate_source_file) = duplicate_source_file {
                    if checked_name != normalized_file_path {
                        context.duplicate_source_files.push(duplicate_source_file);
                    }
                }
                if !loader
                    .opts
                    .config
                    .compiler_options()
                    .force_consistent_casing_in_file_names
                    .is_false()
                {
                    // Check if it differs only in drive letters its ok to ignore that error:
                    let checked_absolute_path = tspath::get_normalized_absolute_path_without_root(
                        &checked_name,
                        &loader.compare_paths_options.current_directory,
                    );
                    let input_absolute_path = tspath::get_normalized_absolute_path_without_root(
                        &normalized_file_path,
                        &loader.compare_paths_options.current_directory,
                    );
                    if checked_absolute_path != input_absolute_path {
                        context
                            .include_processor
                            .add_processing_diagnostics_for_file_casing(
                                &path,
                                &checked_name,
                                &normalized_file_path,
                                include_reason.clone(),
                            );
                    }
                }
                continue;
            } else {
                let normalized_file_path = task_ref
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .normalized_file_path
                    .clone();
                seen.insert(data_key, normalized_file_path);
            }

            if let Some(tasks_seen_by_name_ignore_case) = context.tasks_seen_by_name_ignore_case {
                let path_lower_case = tspath::to_file_name_lower_case(&path.to_string());
                if let Some(task_by_ignore_case) =
                    tasks_seen_by_name_ignore_case.get(&path_lower_case)
                {
                    let (existing_path, existing_file_name) = {
                        let task_by_ignore_case = task_by_ignore_case
                            .lock()
                            .unwrap_or_else(|err| err.into_inner());
                        (
                            task_by_ignore_case.path.clone(),
                            task_by_ignore_case.normalized_file_path.clone(),
                        )
                    };
                    let normalized_file_path = task_ref
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .normalized_file_path
                        .clone();
                    context
                        .include_processor
                        .add_processing_diagnostics_for_file_casing(
                            &existing_path,
                            &existing_file_name,
                            &normalized_file_path,
                            include_reason.clone(),
                        );
                } else {
                    tasks_seen_by_name_ignore_case.insert(path_lower_case, Arc::clone(&task_ref));
                }
            }

            {
                let task = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                for trace in &task.type_resolutions_trace {
                    trace_resolution(loader, trace);
                }
                for trace in &task.resolutions_trace {
                    trace_resolution(loader, trace);
                }
            }

            if context.package_deduplication_enabled && !package_id.name.is_empty() {
                if let Some((_, package_id_file)) = context
                    .package_id_to_source_file
                    .iter()
                    .find(|(existing_package_id, _)| existing_package_id == &package_id)
                {
                    let (normalized_file_path, task_path, duplicate_source_file) = {
                        let task = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                        (
                            task.normalized_file_path.clone(),
                            task.path.clone(),
                            task.file.as_ref().map(duplicate_source_file),
                        )
                    };
                    if let Some(duplicate_source_file) = duplicate_source_file {
                        // Package deduplication keeps the first package instance in the
                        // program, but we still parsed this file and acquired it through
                        // the host, so snapshot disposal must release that extra owner.
                        context.duplicate_source_files.push(duplicate_source_file);
                    }
                    context
                        .redirect_targets_map
                        .entry(package_id_file.clone())
                        .or_default()
                        .push(normalized_file_path.clone());
                    context.redirect_files_by_path.insert(
                        task_path.clone(),
                        RedirectsFile {
                            index: context.files.len() + context.redirect_files_by_path.len(),
                            file_name: normalized_file_path,
                            path: task_path.clone(),
                            target: package_id_file.clone(),
                        },
                    );
                    context
                        .files_by_path_targets
                        .push((task_path.clone(), package_id_file.clone()));
                    if lowest_depth > 0 {
                        context
                            .source_files_found_searching_node_modules
                            .add(task_path);
                    }
                    continue;
                } else if let Some(file_path) = task_ref
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .file
                    .as_ref()
                    .map(|file| file.path())
                {
                    context
                        .package_id_to_source_file
                        .push((package_id.clone(), file_path));
                }
            }

            let sub_tasks = {
                let task = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                task.sub_tasks.clone()
            };
            if !sub_tasks.is_empty() {
                self.collect_files(&sub_tasks, seen, loader, context);
            }

            // Exclude automatic type directive tasks from include reason processing,
            // as these are internal implementation details and should not contribute
            // to the reasons for including files.
            let redirected_parse_task = task_ref
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .redirected_parse_task
                .clone();
            if let Some(redirected_parse_task) = redirected_parse_task {
                if !loader.opts.can_use_project_reference_source() {
                    let redirected_path = redirected_parse_task
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .path
                        .clone();
                    let file_name = task_ref
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .file_name();
                    context
                        .output_file_to_project_reference_source
                        .insert(redirected_path, file_name);
                }
                continue;
            }

            let is_for_automatic_type_directive = task_ref
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .is_for_automatic_type_directive;
            if is_for_automatic_type_directive {
                let (path, type_resolutions_in_file, processing_diagnostics) = {
                    let mut task = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                    (
                        task.path.clone(),
                        std::mem::take(&mut task.type_resolutions_in_file),
                        std::mem::take(&mut task.processing_diagnostics),
                    )
                };
                context
                    .type_resolutions_in_file
                    .insert(path, type_resolutions_in_file);
                if !processing_diagnostics.is_empty() {
                    context
                        .include_processor
                        .add_processing_diagnostic(processing_diagnostics);
                }
                continue;
            }

            let (
                path,
                normalized_file_path,
                processing_diagnostics,
                file,
                lib_file,
                resolutions_in_file,
                type_resolutions_in_file,
                metadata,
                jsx_runtime_import_specifier,
                import_helpers_import_specifier,
            ) = {
                let mut task = task_ref.lock().unwrap_or_else(|err| err.into_inner());
                (
                    task.path.clone(),
                    task.normalized_file_path.clone(),
                    std::mem::take(&mut task.processing_diagnostics),
                    task.file.take(),
                    task.lib_file.take(),
                    std::mem::take(&mut task.resolutions_in_file),
                    std::mem::take(&mut task.type_resolutions_in_file),
                    std::mem::take(&mut task.metadata),
                    task.jsx_runtime_import_specifier.take(),
                    task.import_helpers_import_specifier.take(),
                )
            };

            if !processing_diagnostics.is_empty() {
                context
                    .include_processor
                    .add_processing_diagnostic(processing_diagnostics);
            }

            if file.is_none() {
                context.missing_files.push(normalized_file_path);
                continue;
            }

            let file = file.unwrap().share_readonly().into_source_file();
            let file_path = file.path();
            if let Some(lib_file) = lib_file {
                context.lib_files.push(file);
                context.lib_files_map.insert(path.clone(), lib_file);
            } else {
                context.files.push(file);
            }
            context
                .files_by_path_targets
                .push((path.clone(), file_path));
            context
                .resolved_modules
                .insert(path.clone(), resolutions_in_file);
            context
                .type_resolutions_in_file
                .insert(path.clone(), type_resolutions_in_file);
            context
                .source_file_meta_datas
                .insert(path.clone(), metadata);

            if let Some(jsx_runtime_import_specifier) = jsx_runtime_import_specifier {
                context
                    .jsx_runtime_import_specifiers
                    .insert(path.clone(), jsx_runtime_import_specifier);
            }
            if let Some(import_helpers_import_specifier) = import_helpers_import_specifier {
                context
                    .import_helpers_import_specifiers
                    .insert(path.clone(), import_helpers_import_specifier);
            }
            if lowest_depth > 0 {
                context.source_files_found_searching_node_modules.add(path);
            }
        }
    }

    fn add_include_reason(
        &self,
        include_processor: &mut IncludeProcessor,
        task: &ParseTaskRef,
        reason: Option<FileIncludeReason>,
    ) {
        let (redirected_parse_task, loaded, path) = {
            let task = task.lock().unwrap_or_else(|err| err.into_inner());
            (
                task.redirected_parse_task.clone(),
                task.loaded,
                task.path.clone(),
            )
        };
        if let Some(redirected_parse_task) = redirected_parse_task {
            self.add_include_reason(include_processor, &redirected_parse_task, reason);
        } else if loaded {
            if let Some(existing) = include_processor.file_include_reasons.get_mut(&path) {
                if let Some(reason) = reason {
                    existing.push(reason);
                }
            } else {
                include_processor
                    .file_include_reasons
                    .insert(path, reason.map_or_else(Vec::new, |reason| vec![reason]));
            }
        }
    }
}
