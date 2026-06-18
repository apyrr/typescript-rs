use std::collections::HashMap;
use std::sync::{Arc, Mutex, atomic::AtomicI32};

use ts_ast as ast;
use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_module as module;
use ts_packagejson as packagejson;
use ts_tracing as tracing;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::file_include::{AutomaticTypeDirectiveFileData, ReferencedFileData};
use crate::filesparser::{FilesParser, ParseTask, ParseTaskRef, ResolvedRef, new_parse_task};
use crate::projectreferenceparser::{ProjectReferenceParser, create_project_reference_parse_tasks};
use crate::{
    CompilerHost, FILE_INCLUDE_KIND_AUTOMATIC_TYPE_DIRECTIVE_FILE, FILE_INCLUDE_KIND_IMPORT,
    FILE_INCLUDE_KIND_LIB_FILE, FILE_INCLUDE_KIND_REFERENCE_FILE, FILE_INCLUDE_KIND_ROOT_FILE,
    FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE, FileIncludeReason, IncludeExplainingDiagnostic,
    IncludeProcessor, ProcessingDiagnostic, ProcessingDiagnosticData, ProcessingDiagnosticKind,
    ProgramOptions, ProjectReferenceFileMapper,
};

struct CompilerHostResolutionHost {
    host: Arc<dyn CompilerHost>,
}

impl CompilerHostResolutionHost {
    fn new(host: Arc<dyn CompilerHost>) -> Self {
        Self { host }
    }
}

impl module::ResolutionHost for CompilerHostResolutionHost {
    fn get_current_directory(&self) -> String {
        self.host.get_current_directory()
    }

    fn fs(&self) -> &dyn ts_vfs::Fs {
        self.host.fs()
    }
}

#[derive(Clone)]
pub(crate) struct LibResolution {
    pub(crate) library_name: String,
    pub(crate) resolution: module::ResolvedModule,
    pub(crate) trace: Vec<module::DiagAndArgs>,
}

#[derive(Clone)]
pub struct LibFile {
    pub name: String,
    pub(crate) path: String,
    pub replaced: bool,
}

struct SourceFileFromReferenceDiagnostic {
    message: &'static diagnostics::Message,
    args: Vec<String>,
}

pub(crate) struct FileLoader {
    pub(crate) opts: ProgramOptions,
    pub(crate) resolver: module::Resolver,
    pub(crate) default_library_path: String,
    pub(crate) compare_paths_options: tspath::ComparePathsOptions,
    pub(crate) supported_extensions: Vec<Vec<String>>,
    pub(crate) supported_extensions_with_json_if_resolve_json_module: Vec<Vec<String>>,

    pub(crate) files_parser: FilesParser,
    pub(crate) root_tasks: Vec<ParseTaskRef>,

    pub(crate) total_file_count: AtomicI32,
    pub(crate) lib_file_count: AtomicI32,

    pub(crate) factory_mu: Mutex<()>,
    pub(crate) factory: ast::NodeFactory,

    pub(crate) project_reference_file_mapper: ProjectReferenceFileMapper,
    pub(crate) project_reference_resolution_host: Option<module::ResolutionHostBox>,
    pub(crate) dts_directories: collections::Set<tspath::Path>,

    pub(crate) path_for_lib_file_cache: collections::SyncMap<String, LibFile>,
    pub(crate) path_for_lib_file_resolutions: collections::SyncMap<tspath::Path, LibResolution>,
}

#[derive(Clone)]
pub(crate) struct RedirectsFile {
    // Index of file at which this redirect file needs to be iterated
    pub(crate) index: usize,
    pub(crate) file_name: String,
    pub(crate) path: tspath::Path,
    pub(crate) target: tspath::Path,
}

#[derive(Clone)]
pub struct DuplicateSourceFile {
    pub parse_options: ast::SourceFileParseOptions,
    pub hash: u128,
    pub script_kind: core::ScriptKind,
}

impl ast::HasFileName for RedirectsFile {
    fn file_name(&self) -> String {
        self.file_name.clone()
    }

    fn path(&self) -> tspath::Path {
        self.path.clone()
    }
}

impl RedirectsFile {
    fn file_name(&self) -> String {
        self.file_name.clone()
    }

    fn path(&self) -> tspath::Path {
        self.path.clone()
    }
}

#[derive(Default)]
pub(crate) struct ProcessedFiles {
    pub(crate) resolver: Arc<Mutex<module::Resolver>>,
    // duplicateSourceFiles tracks parsed files loaded during program construction
    // that were later dropped from the final program, such as losing filename
    // casing variants for the same path or files hidden behind package redirect
    // deduplication. Their parse-cache acquires still need to be balanced when
    // the program is disposed.
    pub(crate) duplicate_source_files: Vec<DuplicateSourceFile>,
    pub(crate) files_by_path: HashMap<tspath::Path, usize>,
    pub(crate) project_reference_file_mapper: ProjectReferenceFileMapper,
    pub(crate) missing_files: Vec<String>,
    pub(crate) resolved_modules:
        HashMap<tspath::Path, module::ModeAwareCache<module::ResolvedModule>>,
    pub(crate) type_resolutions_in_file:
        HashMap<tspath::Path, module::ModeAwareCache<module::ResolvedTypeReferenceDirective>>,
    pub(crate) source_file_meta_datas: HashMap<tspath::Path, ast::SourceFileMetaData>,
    pub(crate) jsx_runtime_import_specifiers: HashMap<tspath::Path, JsxRuntimeImportSpecifier>,
    pub(crate) import_helpers_import_specifiers: HashMap<tspath::Path, ast::StringLiteralNode>,
    pub(crate) lib_files: HashMap<tspath::Path, LibFile>,
    // List of present unsupported extensions
    pub(crate) source_files_found_searching_node_modules: collections::Set<tspath::Path>,
    pub(crate) include_processor: IncludeProcessor,
    // if file was included using source file and its output is actually part of program
    // this contains mapping from output to source file
    pub(crate) output_file_to_project_reference_source: HashMap<tspath::Path, String>,
    // Key is a file path. Value is the list of files that redirect to it (same package, different install location)
    pub(crate) redirect_targets_map: HashMap<tspath::Path, Vec<String>>,
    // filesByPath for redirect files
    pub(crate) redirect_files_by_path: HashMap<tspath::Path, RedirectsFile>,
    pub(crate) finished_processing: bool,
}

impl Clone for ProcessedFiles {
    fn clone(&self) -> Self {
        Self {
            resolver: self.resolver.clone(),
            duplicate_source_files: self.duplicate_source_files.clone(),
            files_by_path: self.files_by_path.clone(),
            project_reference_file_mapper: self.project_reference_file_mapper.clone(),
            missing_files: self.missing_files.clone(),
            resolved_modules: self.resolved_modules.clone(),
            type_resolutions_in_file: self.type_resolutions_in_file.clone(),
            source_file_meta_datas: self.source_file_meta_datas.clone(),
            jsx_runtime_import_specifiers: self.jsx_runtime_import_specifiers.clone(),
            import_helpers_import_specifiers: self.import_helpers_import_specifiers.clone(),
            lib_files: self.lib_files.clone(),
            source_files_found_searching_node_modules: self
                .source_files_found_searching_node_modules
                .clone(),
            include_processor: self.include_processor.clone(),
            output_file_to_project_reference_source: self
                .output_file_to_project_reference_source
                .clone(),
            redirect_targets_map: self.redirect_targets_map.clone(),
            redirect_files_by_path: self.redirect_files_by_path.clone(),
            finished_processing: self.finished_processing,
        }
    }
}

pub(crate) struct ProcessedProgramFiles {
    pub(crate) processed_files: ProcessedFiles,
    pub(crate) source_files: Vec<ast::SourceFile>,
}

#[derive(Clone)]
pub(crate) struct JsxRuntimeImportSpecifier {
    pub(crate) module_reference: String,
    pub(crate) specifier: ast::StringLiteralNode,
}

pub(crate) fn process_all_program_files(
    opts: ProgramOptions,
    single_threaded: bool,
) -> ProcessedProgramFiles {
    let compiler_options = opts.config.compiler_options();
    let root_files = opts.config.file_names().to_vec();
    let supported_extensions = tsoptions::get_supported_extensions(&compiler_options, &[]);
    let supported_extensions_with_json_if_resolve_json_module =
        tsoptions::get_supported_extensions_with_json_if_resolve_json_module(
            Some(&compiler_options),
            supported_extensions.clone(),
        );
    let current_directory = opts.host.get_current_directory();
    let default_library_path =
        tspath::get_normalized_absolute_path(&opts.host.default_library_path(), &current_directory);
    let use_case_sensitive_file_names = opts.host.fs().use_case_sensitive_file_names();
    let mut max_node_module_js_depth = 0;
    if let Some(p) = compiler_options.max_node_module_js_depth {
        max_node_module_js_depth = p;
    }
    let mut loader = FileLoader {
        opts,
        default_library_path,
        compare_paths_options: tspath::ComparePathsOptions {
            use_case_sensitive_file_names,
            current_directory,
        },
        files_parser: FilesParser::new(
            core::new_work_group(single_threaded),
            max_node_module_js_depth as i32,
        ),
        root_tasks: Vec::with_capacity(root_files.len() + compiler_options.lib.len()),
        supported_extensions,
        supported_extensions_with_json_if_resolve_json_module,
        resolver: module::Resolver::default(),
        total_file_count: AtomicI32::new(0),
        lib_file_count: AtomicI32::new(0),
        factory_mu: Mutex::new(()),
        factory: ast::NodeFactory::default(),
        project_reference_file_mapper: ProjectReferenceFileMapper::default(),
        project_reference_resolution_host: None,
        dts_directories: collections::Set::new(),
        path_for_lib_file_cache: collections::SyncMap::default(),
        path_for_lib_file_resolutions: collections::SyncMap::default(),
    };
    loader.add_project_reference_tasks(single_threaded);
    let resolver_host = loader
        .project_reference_resolution_host
        .take()
        .unwrap_or_else(|| Box::new(CompilerHostResolutionHost::new(loader.opts.host.clone())));
    loader.resolver = module::new_resolver(
        resolver_host,
        compiler_options.clone(),
        loader.opts.typings_location.clone(),
        loader.opts.project_name.clone(),
    );
    let pop_root_files_trace = loader.opts.tracing.as_mut().map(|tracing| {
        tracing.push(
            tracing::PHASE_PROGRAM,
            "processRootFiles",
            hashmap! {"count" => root_files.len()},
            false,
        )
    });
    process_root_files(&mut loader, &root_files, &compiler_options);

    if !root_files.is_empty() {
        loader.add_automatic_type_directive_tasks();
    }

    // PORT NOTE: Go stores filesParser inside FileLoader and passes the loader
    // receiver back through it. Move the parser out temporarily so Rust can
    // borrow the parser and loader mutably at the same time.
    let mut files_parser = std::mem::replace(
        &mut loader.files_parser,
        FilesParser::new(
            core::new_work_group(single_threaded),
            max_node_module_js_depth as i32,
        ),
    );
    let root_tasks = loader.root_tasks.clone();
    files_parser.parse(&mut loader, root_tasks);

    // Clear out host to ensure its not used post program creation
    loader.project_reference_file_mapper.host = None;

    let processed_files = files_parser.get_processed_files(&mut loader);
    loader.files_parser = files_parser;
    if let Some(pop_trace) = pop_root_files_trace {
        if let Some(tracing) = loader.opts.tracing.as_mut() {
            pop_trace(tracing);
        }
    }
    processed_files
}

fn process_root_files(
    loader: &mut FileLoader,
    root_files: &[String],
    compiler_options: &core::CompilerOptions,
) {
    for (index, root_file) in root_files.iter().enumerate() {
        loader.add_root_task(
            root_file,
            None,
            FileIncludeReason::new(FILE_INCLUDE_KIND_ROOT_FILE, index),
        );
    }
    if !root_files.is_empty() && compiler_options.no_lib.is_false_or_unknown() {
        if !loader.opts.config.options.contains_key("lib") {
            let name = tsoptions::get_default_lib_file_name(compiler_options);
            let lib_file = loader.path_for_lib_file(&name);
            let lib_path = lib_file.path.clone();
            loader.add_root_task(
                &lib_path,
                Some(lib_file),
                FileIncludeReason::new(FILE_INCLUDE_KIND_LIB_FILE, ()),
            );
        } else {
            for (index, lib) in compiler_options.lib.iter().enumerate() {
                if let Some(name) = tsoptions::get_lib_file_name(lib) {
                    let lib_file = loader.path_for_lib_file(&name);
                    let lib_path = lib_file.path.clone();
                    loader.add_root_task(
                        &lib_path,
                        Some(lib_file),
                        FileIncludeReason::new(FILE_INCLUDE_KIND_LIB_FILE, index),
                    );
                }
                // !!! error on unknown name
            }
        }
    }
}

impl FileLoader {
    pub(crate) fn to_path(&self, file: &str) -> tspath::Path {
        tspath::to_path(
            file,
            &self.opts.host.get_current_directory(),
            self.opts.host.fs().use_case_sensitive_file_names(),
        )
    }

    fn add_root_task(
        &mut self,
        file_name: &str,
        lib_file: Option<LibFile>,
        include_reason: FileIncludeReason,
    ) {
        let abs_path = tspath::get_normalized_absolute_path(
            file_name,
            &self.opts.host.get_current_directory(),
        );
        if self
            .opts
            .config
            .compiler_options()
            .allow_non_ts_extensions
            .is_true()
            || tspath::has_extension(&abs_path)
        {
            self.root_tasks.push(new_parse_task(ParseTask {
                normalized_file_path: abs_path,
                lib_file,
                include_reason: Some(include_reason),
                ..ParseTask::default()
            }));
        }
    }

    fn add_automatic_type_directive_tasks(&mut self) {
        let compiler_options = self.opts.config.compiler_options();
        let containing_directory = if !compiler_options.config_file_path.is_empty() {
            tspath::get_directory_path(&compiler_options.config_file_path)
        } else {
            self.opts.host.get_current_directory()
        };
        let containing_file_name = tspath::combine_paths(
            &containing_directory,
            &[module::INFERRED_TYPES_CONTAINING_FILE],
        );
        self.root_tasks.push(new_parse_task(ParseTask {
            normalized_file_path: containing_file_name,
            is_for_automatic_type_directive: true,
            ..ParseTask::default()
        }));
    }

    pub(crate) fn resolve_automatic_type_directives(
        &mut self,
        containing_file_name: &str,
    ) -> (
        Vec<ResolvedRef>,
        module::ModeAwareCache<module::ResolvedTypeReferenceDirective>,
        Vec<module::DiagAndArgs>,
        Vec<ProcessingDiagnostic>,
    ) {
        let automatic_type_directive_names = module::get_automatic_type_directive_names(
            &self.opts.config.compiler_options(),
            &CompilerHostResolutionHost::new(self.opts.host.clone()),
        );
        let mut to_parse = Vec::new();
        let mut type_resolutions_in_file = module::ModeAwareCache::new();
        let mut type_resolutions_trace = Vec::new();
        let mut p_diagnostics = Vec::new();
        if !automatic_type_directive_names.is_empty() {
            to_parse = Vec::with_capacity(automatic_type_directive_names.len());
            type_resolutions_in_file =
                module::ModeAwareCache::with_capacity(automatic_type_directive_names.len());
            for name in automatic_type_directive_names {
                // Under node16/nodenext module resolution, load `types`/ata include names as cjs resolution results by passing an `undefined` mode.
                // Under bundler module resolution, this also triggers the "import" condition to be used.
                let resolution_mode = core::ResolutionMode::None;
                let (resolved, trace) = self.resolver.resolve_type_reference_directive(
                    &name,
                    containing_file_name,
                    resolution_mode,
                    None,
                    true,
                );
                let trace_done = self.opts.tracing.as_mut().map(|tracing| {
                    tracing.push(
                        tracing::PHASE_PROGRAM,
                        "processTypeReferenceDirective",
                        hashmap! {
                            "directive" => name.clone(),
                            "hasResolved" => resolved.is_resolved(),
                            "refKind" => FILE_INCLUDE_KIND_AUTOMATIC_TYPE_DIRECTIVE_FILE as i32,
                        },
                        false,
                    )
                });
                type_resolutions_in_file.insert(
                    module::ModeAwareCacheKey {
                        name: name.clone(),
                        mode: resolution_mode,
                    },
                    resolved.clone(),
                );
                type_resolutions_trace.extend(trace);
                if resolved.is_resolved() {
                    to_parse.push(ResolvedRef {
                        file_name: resolved.resolved_file_name.clone(),
                        increase_depth: resolved.is_external_library_import,
                        elide_on_depth: false,
                        include_reason: Some(FileIncludeReason::new(
                            FILE_INCLUDE_KIND_AUTOMATIC_TYPE_DIRECTIVE_FILE,
                            AutomaticTypeDirectiveFileData {
                                type_reference: name,
                                package_id: resolved.package_id.clone(),
                            },
                        )),
                        package_id: resolved.package_id,
                    });
                } else {
                    p_diagnostics.push(ProcessingDiagnostic {
                        kind: ProcessingDiagnosticKind::ExplainingFileInclude,
                        data: ProcessingDiagnosticData::IncludeExplaining(
                            IncludeExplainingDiagnostic {
                                file: None,
                                diagnostic_reason: Some(FileIncludeReason::new(
                                    FILE_INCLUDE_KIND_AUTOMATIC_TYPE_DIRECTIVE_FILE,
                                    AutomaticTypeDirectiveFileData {
                                        type_reference: name.clone(),
                                        package_id: Default::default(),
                                    },
                                )),
                                message: &diagnostics::Cannot_find_type_definition_file_for_0,
                                args: vec![name],
                            },
                        ),
                    });
                }
                if let Some(trace_done) = trace_done {
                    if let Some(tracing) = self.opts.tracing.as_mut() {
                        trace_done(tracing);
                    }
                }
            }
        }
        (
            to_parse,
            type_resolutions_in_file,
            type_resolutions_trace,
            p_diagnostics,
        )
    }

    fn add_project_reference_tasks(&mut self, single_threaded: bool) {
        self.project_reference_file_mapper = ProjectReferenceFileMapper {
            opts: Some(self.opts.clone()),
            host: Some(Box::new(CompilerHostResolutionHost::new(
                self.opts.host.clone(),
            ))),
            ..ProjectReferenceFileMapper::default()
        };
        let project_references = self.opts.config.resolved_project_reference_paths().to_vec();
        if project_references.is_empty() {
            return;
        }

        let mut parser = ProjectReferenceParser {
            loader: self,
            wg: core::new_work_group(single_threaded),
            tasks_by_file_name: collections::SyncMap::default(),
        };
        let root_tasks = create_project_reference_parse_tasks(&project_references);
        parser.parse(root_tasks);
    }

    pub(crate) fn sort_libs(&self, lib_files: &mut [ast::SourceFile]) {
        lib_files.sort_by(|f1, f2| {
            self.get_default_lib_file_priority(f1)
                .cmp(&self.get_default_lib_file_priority(f2))
        });
    }

    fn get_default_lib_file_priority(&self, a: &ast::SourceFile) -> usize {
        // defaultLibraryPath and a.FileName() are absolute and normalized; a prefix check should suffice.
        let default_library_path =
            tspath::remove_trailing_directory_separator(&self.default_library_path);
        let a_file_name = a.file_name();

        if a_file_name.starts_with(&default_library_path)
            && a_file_name.len() > default_library_path.len()
            && a_file_name.as_bytes()[default_library_path.len()]
                == tspath::DIRECTORY_SEPARATOR as u8
        {
            // avoid tspath.GetBaseFileName; we know these paths are already absolute and normalized.
            let basename =
                &a_file_name[a_file_name.rfind(tspath::DIRECTORY_SEPARATOR).unwrap() + 1..];
            if basename == "lib.d.ts" || basename == "lib.es6.d.ts" {
                return 0;
            }
            let name = basename.strip_prefix("lib.").unwrap_or(basename);
            let name = name.strip_suffix(".d.ts").unwrap_or(name);
            if let Some(index) = tsoptions::LIBS.iter().position(|lib| *lib == name) {
                return index + 1;
            }
        }
        tsoptions::LIBS.len() + 2
    }

    pub(crate) fn load_source_file_meta_data(&self, file_name: &str) -> ast::SourceFileMetaData {
        let package_json_scope = self
            .resolver
            .get_package_scope_for_path(&tspath::get_directory_path(file_name));
        let module_resolution_kind = self
            .opts
            .config
            .compiler_options()
            .get_module_resolution_kind();

        let mut package_json_type = String::new();
        let mut package_json_directory = String::new();
        if let Some(package_directory) = package_json_scope {
            package_json_directory = package_directory;
            let package_json_path =
                tspath::combine_paths(&package_json_directory, &["package.json"]);
            let (text, ok) = self.opts.host.fs().read_file(&package_json_path);
            let package_json_type_field = ok
                .then(|| packagejson::parse(text.as_bytes()).ok())
                .flatten()
                .and_then(|fields| {
                    let (value, valid) = fields.header_fields.type_.get_value();
                    valid.then_some(value)
                });
            if let Some(value) = package_json_type_field {
                if !tspath::file_extension_is_one_of(
                    file_name,
                    &[
                        tspath::EXTENSION_MTS,
                        tspath::EXTENSION_CTS,
                        tspath::EXTENSION_MJS,
                        tspath::EXTENSION_CJS,
                    ],
                ) && core::MODULE_RESOLUTION_KIND_NODE16 <= module_resolution_kind
                    && module_resolution_kind <= core::MODULE_RESOLUTION_KIND_NODE_NEXT
                    || file_name.contains("/node_modules/")
                {
                    package_json_type = value;
                }
            }
        }

        let implied_node_format =
            ast::get_implied_node_format_for_file(file_name, &package_json_type);
        ast::SourceFileMetaData {
            package_json_type,
            package_json_directory,
            implied_node_format,
        }
    }

    pub(crate) fn parse_source_file(&mut self, t: &ParseTask) -> Option<ast::ParsedSourceFile> {
        let pop_trace = self.opts.tracing.as_mut().map(|tracing| {
            tracing.push(
                tracing::PHASE_PARSE,
                "createSourceFile",
                hashmap! {"path" => t.normalized_file_path.clone()},
                true,
            )
        });
        let path = self.to_path(&t.normalized_file_path);
        let options = self
            .project_reference_file_mapper
            .get_compiler_options_for_file(t);
        let source_file =
            self.opts
                .host
                .as_ref()
                .get_parsed_source_file(ast::SourceFileParseOptions {
                    file_name: t.normalized_file_path.clone(),
                    path,
                    external_module_indicator_options: ast::get_external_module_indicator_options(
                        &t.normalized_file_path,
                        &options,
                        t.metadata.clone(),
                    ),
                });
        if let Some(pop_trace) = pop_trace {
            if let Some(tracing) = self.opts.tracing.as_mut() {
                pop_trace(tracing);
            }
        }
        source_file
    }

    pub(crate) fn is_supported_extension(&self, canonical_file_name: &str) -> bool {
        for group in &self.supported_extensions_with_json_if_resolve_json_module {
            let group: Vec<&str> = group.iter().map(String::as_str).collect();
            if tspath::file_extension_is_one_of(canonical_file_name, &group) {
                return true;
            }
        }
        false
    }

    fn get_source_file_from_reference(
        &self,
        file_name: &str,
        reference_text: &str,
        containing_file: &str,
    ) -> (String, Option<SourceFileFromReferenceDiagnostic>) {
        let options = self.opts.config.compiler_options();
        let allow_non_ts_extensions = options.allow_non_ts_extensions.is_true();
        let diagnostic_file_name = tspath::normalize_slashes(reference_text);

        if tspath::has_extension(file_name) {
            let canonical_file_name = tspath::get_canonical_file_name(
                file_name,
                self.opts.host.fs().use_case_sensitive_file_names(),
            );
            if !allow_non_ts_extensions && !self.is_supported_extension(&canonical_file_name) {
                if tspath::has_js_file_extension(&canonical_file_name) {
                    return (
                        String::new(),
                        Some(SourceFileFromReferenceDiagnostic {
                            message: &diagnostics::File_0_is_a_JavaScript_file_Did_you_mean_to_enable_the_allowJs_option,
                            args: vec![diagnostic_file_name],
                        }),
                    );
                }
                return (
                    String::new(),
                    Some(SourceFileFromReferenceDiagnostic {
                        message: &diagnostics::File_0_has_an_unsupported_extension_The_only_supported_extensions_are_1,
                        args: vec![
                            diagnostic_file_name,
                            format!("'{}'", core::flatten(&self.supported_extensions).join("', '")),
                        ],
                    }),
                );
            }

            if !self.opts.host.fs().file_exists(file_name) {
                return (
                    String::new(),
                    Some(SourceFileFromReferenceDiagnostic {
                        message: &diagnostics::File_0_not_found,
                        args: vec![diagnostic_file_name],
                    }),
                );
            }

            if tspath::get_canonical_file_name(
                containing_file,
                self.opts.host.fs().use_case_sensitive_file_names(),
            ) == canonical_file_name
            {
                return (
                    String::new(),
                    Some(SourceFileFromReferenceDiagnostic {
                        message: &diagnostics::A_file_cannot_have_a_reference_to_itself,
                        args: Vec::new(),
                    }),
                );
            }
            return (file_name.to_string(), None);
        }

        if allow_non_ts_extensions && self.opts.host.fs().file_exists(file_name) {
            return (file_name.to_string(), None);
        }

        if allow_non_ts_extensions {
            return (
                String::new(),
                Some(SourceFileFromReferenceDiagnostic {
                    message: &diagnostics::File_0_not_found,
                    args: vec![diagnostic_file_name],
                }),
            );
        }

        for ext in &self.supported_extensions[0] {
            let candidate = format!("{file_name}{ext}");
            if self.opts.host.fs().file_exists(&candidate) {
                return (candidate, None);
            }
        }

        (
            String::new(),
            Some(SourceFileFromReferenceDiagnostic {
                message: &diagnostics::Could_not_resolve_the_path_0_with_the_extensions_Colon_1,
                args: vec![
                    diagnostic_file_name,
                    format!(
                        "'{}'",
                        core::flatten(&self.supported_extensions).join("', '")
                    ),
                ],
            }),
        )
    }

    pub(crate) fn resolve_tripleslash_path_reference(
        &self,
        module_name: &str,
        containing_file: &str,
        index: usize,
    ) -> (Option<ResolvedRef>, Option<ProcessingDiagnostic>) {
        let base_path = tspath::get_directory_path(containing_file);
        let mut referenced_file_name = module_name.to_string();

        if !tspath::is_rooted_disk_path(module_name) {
            referenced_file_name = tspath::combine_paths(&base_path, &[module_name]);
        }
        let normalized_file_name = tspath::normalize_path(&referenced_file_name);
        let include_reason = FileIncludeReason::new(
            FILE_INCLUDE_KIND_REFERENCE_FILE,
            ReferencedFileData {
                file: self.to_path(containing_file),
                index: index as isize,
                synthetic: None,
            },
        );

        let (resolved_file_name, diagnostic) = self.get_source_file_from_reference(
            &normalized_file_name,
            module_name,
            containing_file,
        );
        if let Some(diagnostic) = diagnostic {
            return (
                None,
                Some(ProcessingDiagnostic {
                    kind: ProcessingDiagnosticKind::ExplainingFileInclude,
                    data: ProcessingDiagnosticData::IncludeExplaining(
                        IncludeExplainingDiagnostic {
                            file: None,
                            diagnostic_reason: Some(include_reason),
                            message: diagnostic.message,
                            args: diagnostic.args,
                        },
                    ),
                }),
            );
        }

        (
            Some(ResolvedRef {
                file_name: resolved_file_name,
                include_reason: Some(include_reason),
                ..ResolvedRef::default()
            }),
            None,
        )
    }

    pub(crate) fn resolve_type_reference_directives(&mut self, t: &mut ParseTask) {
        let file = t
            .file
            .clone()
            .expect("resolve_type_reference_directives requires a loaded source file");
        let type_reference_directives = file.type_reference_directives().to_vec();
        if type_reference_directives.is_empty() {
            return;
        }
        let containing_file_name = file.file_name();
        let pop_trace = self.opts.tracing.as_mut().map(|tracing| {
            tracing.push(
                tracing::PHASE_PROGRAM,
                "resolveTypeReferenceDirectiveNamesWorker",
                hashmap! {"containingFileName" => containing_file_name},
                false,
            )
        });
        let meta = t.metadata.clone();

        let mut type_resolutions_in_file =
            module::ModeAwareCache::with_capacity(type_reference_directives.len());
        let mut type_resolutions_trace = Vec::new();
        for (index, r#ref) in type_reference_directives.iter().enumerate() {
            let (redirect, file_name) = self
                .project_reference_file_mapper
                .get_redirect_for_resolution(&file);
            let redirected_reference = redirect
                .as_ref()
                .map(|redirect| redirect as &dyn module::ResolvedProjectReference);
            let options_for_file = module::get_compiler_options_with_redirect(
                &self.opts.config.compiler_options(),
                redirected_reference,
            );
            let resolution_mode = get_mode_for_type_reference_directive_in_file(
                r#ref,
                &file.file_name(),
                &meta,
                &options_for_file,
            );
            let (resolved, trace) = self.resolver.resolve_type_reference_directive(
                &r#ref.file_name,
                &file_name,
                resolution_mode,
                redirected_reference,
                false,
            );
            let trace_done = self.opts.tracing.as_mut().map(|tracing| {
                tracing.push(
                    tracing::PHASE_PROGRAM,
                    "processTypeReferenceDirective",
                    hashmap! {
                        "directive" => r#ref.file_name.clone(),
                        "hasResolved" => resolved.is_resolved(),
                        "refKind" => FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE as i32,
                        "refPath" => t.path.to_string(),
                    },
                    false,
                )
            });
            type_resolutions_in_file.insert(
                module::ModeAwareCacheKey {
                    name: r#ref.file_name.clone(),
                    mode: resolution_mode,
                },
                resolved.clone(),
            );
            let include_reason = FileIncludeReason::new(
                FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE,
                ReferencedFileData {
                    file: t.path.clone(),
                    index: index as isize,
                    synthetic: None,
                },
            );
            type_resolutions_trace.extend(trace);

            if resolved.is_resolved() {
                t.add_sub_task(
                    ResolvedRef {
                        file_name: resolved.resolved_file_name.clone(),
                        increase_depth: resolved.is_external_library_import,
                        elide_on_depth: false,
                        include_reason: Some(include_reason),
                        package_id: resolved.package_id,
                    },
                    None,
                );
            } else {
                t.processing_diagnostics.push(ProcessingDiagnostic {
                    kind: ProcessingDiagnosticKind::UnknownReference,
                    data: ProcessingDiagnosticData::FileIncludeReason(include_reason),
                });
            }
            if let Some(trace_done) = trace_done {
                if let Some(tracing) = self.opts.tracing.as_mut() {
                    trace_done(tracing);
                }
            }
        }

        t.type_resolutions_in_file = type_resolutions_in_file;
        t.type_resolutions_trace = type_resolutions_trace;
        if let Some(pop_trace) = pop_trace {
            if let Some(tracing) = self.opts.tracing.as_mut() {
                pop_trace(tracing);
            }
        }
    }
}

const EXTERNAL_HELPERS_MODULE_NAME_TEXT: &str = "tslib"; // TODO(jakebailey): dedupe

impl FileLoader {
    pub(crate) fn resolve_imports_and_module_augmentations(&mut self, t: &mut ParseTask) {
        let file = t
            .file
            .clone()
            .expect("resolve_imports_and_module_augmentations requires a loaded source file");
        let containing_file_name = file.file_name();
        let pop_trace = self.opts.tracing.as_mut().map(|tracing| {
            tracing.push(
                tracing::PHASE_PROGRAM,
                "resolveModuleNamesWorker",
                hashmap! {"containingFileName" => containing_file_name},
                false,
            )
        });
        let meta = t.metadata.clone();

        let imports = file.imports().to_vec();
        let module_augmentations = file.module_augmentations().to_vec();
        let is_java_script_file = matches!(
            file.script_kind(),
            core::ScriptKind::JS | core::ScriptKind::JSX
        );
        let is_external_module_file = ast::is_external_module(&file);
        let mut module_names: Vec<ast::Node> =
            Vec::with_capacity(imports.len() + module_augmentations.len() + 2);

        let (redirect, file_name) = self
            .project_reference_file_mapper
            .get_redirect_for_resolution(&file);
        let redirected_reference = redirect
            .as_ref()
            .map(|redirect| redirect as &dyn module::ResolvedProjectReference);
        let options_for_file = module::get_compiler_options_with_redirect(
            &self.opts.config.compiler_options(),
            redirected_reference,
        );
        let is_declaration_file = file.is_declaration_file();
        if is_java_script_file
            || (!is_declaration_file
                && (options_for_file.get_isolated_modules() || is_external_module_file))
        {
            if options_for_file.import_helpers.is_true() {
                let specifier =
                    self.create_synthetic_import(EXTERNAL_HELPERS_MODULE_NAME_TEXT, &file);
                module_names.push(specifier);
                t.import_helpers_import_specifier = Some(specifier);
            }
        }

        let script_kind = file.script_kind();
        if script_kind == core::ScriptKind::Jsx || script_kind == core::ScriptKind::Tsx {
            let program_file = file.share_readonly().into_source_file();
            let jsx_import = {
                ast::get_jsx_runtime_import(
                    &ast::get_jsx_implicit_import_base(&options_for_file, &program_file),
                    &options_for_file,
                )
            };
            if !jsx_import.is_empty() {
                let specifier = self.create_synthetic_import(&jsx_import, &file);
                module_names.push(specifier);
                t.jsx_runtime_import_specifier = Some(JsxRuntimeImportSpecifier {
                    module_reference: jsx_import.to_string(),
                    specifier,
                });
            }
        }

        let imports_start = module_names.len();

        module_names.extend(imports.iter().copied());
        for imp in &module_augmentations {
            if file.store().kind(*imp) == ast::Kind::StringLiteral {
                module_names.push(*imp);
            }
            // Do nothing if it's an Identifier; we don't need to do module resolution for `declare global`.
        }

        if !module_names.is_empty() {
            let mut resolutions_in_file = module::ModeAwareCache::with_capacity(module_names.len());
            let mut resolutions_trace = Vec::new();

            for (index, entry) in module_names.iter().enumerate() {
                let entry_store = if entry.store_id() == file.store().store_id() {
                    file.store()
                } else {
                    self.factory.store()
                };
                let module_name = entry_store.text(*entry);
                if module_name.is_empty() {
                    continue;
                }

                let mode = get_mode_for_usage_location(
                    entry_store,
                    &file.file_name(),
                    &meta,
                    entry,
                    &options_for_file,
                );
                let (resolved_module, trace) = self.resolver.resolve_module_name(
                    &module_name,
                    &file_name,
                    mode,
                    redirected_reference,
                );
                resolutions_in_file.insert(
                    module::ModeAwareCacheKey {
                        name: module_name.clone(),
                        mode,
                    },
                    resolved_module.clone(),
                );
                resolutions_trace.extend(trace);

                if !resolved_module.is_resolved() {
                    continue;
                }

                let resolved_file_name = resolved_module.resolved_file_name.clone();
                let is_from_node_modules_search = resolved_module.is_external_library_import;
                // Don't treat redirected files as JS files.
                let is_js_file = !tspath::file_extension_is_one_of(
                    &resolved_file_name,
                    tspath::SUPPORTED_TS_EXTENSIONS_WITH_JSON_FLAT,
                ) && self
                    .project_reference_file_mapper
                    .get_redirect_parsed_command_line_for_resolution(&ast::new_has_file_name(
                        resolved_file_name.clone(),
                        self.to_path(&resolved_file_name),
                    ))
                    .is_none();
                let is_js_file_from_node_modules = is_from_node_modules_search
                    && is_js_file
                    && resolved_file_name.contains("/node_modules/");

                // add file to program only if:
                // - resolution was successful
                // - noResolve is falsy
                // - module name comes from the list of imports
                // - it's not a top level JavaScript module that exceeded the search max

                let import_index = index as isize - imports_start as isize;

                let should_add_file = !module_name.is_empty()
                    && module::get_resolution_diagnostic(
                        &options_for_file,
                        &resolved_module,
                        is_declaration_file,
                    )
                    .is_none()
                    && !options_for_file.no_resolve.is_true()
                    && !(is_js_file && !options_for_file.get_allow_js())
                    && (import_index < 0 || (import_index as usize) < imports.len());

                if should_add_file {
                    t.add_sub_task(
                        ResolvedRef {
                            file_name: resolved_file_name,
                            increase_depth: resolved_module.is_external_library_import,
                            elide_on_depth: is_js_file_from_node_modules,
                            include_reason: Some(FileIncludeReason::new(
                                FILE_INCLUDE_KIND_IMPORT,
                                ReferencedFileData {
                                    file: t.path.clone(),
                                    index: import_index,
                                    synthetic: if import_index < 0 {
                                        Some(entry.clone())
                                    } else {
                                        None
                                    },
                                },
                            )),
                            package_id: resolved_module.package_id,
                        },
                        None,
                    );
                }
            }

            t.resolutions_in_file = resolutions_in_file;
            t.resolutions_trace = resolutions_trace;
        }
        if let Some(pop_trace) = pop_trace {
            if let Some(tracing) = self.opts.tracing.as_mut() {
                pop_trace(tracing);
            }
        }
    }

    fn create_synthetic_import(
        &mut self,
        text: &str,
        file: &(impl ast::SourceFileStoreLike + ?Sized),
    ) -> ast::StringLiteralNode {
        let external_helpers_module_reference = self
            .factory
            .new_string_literal(text.to_string(), ast::TOKEN_FLAGS_NONE);
        let import_decl = self.factory.new_import_declaration(
            None,
            None,
            external_helpers_module_reference.clone(),
            None,
        );
        self.factory
            .link_external_helper_parent(import_decl, Some(file.as_node()));
        self.factory
            .link_external_helper_parent(external_helpers_module_reference, Some(import_decl));
        external_helpers_module_reference
    }

    pub(crate) fn path_for_lib_file(&mut self, name: &str) -> LibFile {
        if let (Some(cached), true) = self.path_for_lib_file_cache.load(&name.to_string()) {
            return cached;
        }

        let mut path = tspath::combine_paths(&self.default_library_path, &[name]);
        let mut replaced = false;
        if self
            .opts
            .config
            .compiler_options()
            .lib_replacement
            .is_true()
            && name != "lib.d.ts"
        {
            let library_name = get_library_name_from_lib_file_name(name);
            let resolve_from = get_inferred_library_name_resolve_from(
                &self.opts.config.compiler_options(),
                &self.opts.host.get_current_directory(),
                name,
            );
            let (resolution, trace) = self.resolve_library(&library_name, &resolve_from);
            if resolution.is_resolved() {
                path = resolution.resolved_file_name.clone();
                replaced = true;
            }
            self.path_for_lib_file_resolutions.load_or_store(
                self.to_path(&resolve_from),
                Some(LibResolution {
                    library_name,
                    resolution,
                    trace,
                }),
            );
        }

        self.path_for_lib_file_cache
            .load_or_store(
                name.to_string(),
                Some(LibFile {
                    name: name.to_string(),
                    path,
                    replaced,
                }),
            )
            .0
            .expect("path_for_lib_file stores a lib file")
    }

    fn resolve_library(
        &mut self,
        library_name: &str,
        resolve_from: &str,
    ) -> (module::ResolvedModule, Vec<module::DiagAndArgs>) {
        let pop_trace = self.opts.tracing.as_mut().map(|tr| {
            tr.push(
                tracing::PHASE_PROGRAM,
                "resolveLibrary",
                hashmap! {"resolveFrom" => resolve_from.to_string()},
                false,
            )
        });
        let result = self.resolver.resolve_module_name(
            library_name,
            resolve_from,
            core::ResolutionMode::CommonJs,
            None,
        );
        if let Some(pop_trace) = pop_trace {
            if let Some(tracing) = self.opts.tracing.as_mut() {
                pop_trace(tracing);
            }
        }
        result
    }
}

fn get_library_name_from_lib_file_name(lib_file_name: &str) -> String {
    // Support resolving to lib.dom.d.ts -> @typescript/lib-dom, and
    //                      lib.dom.iterable.d.ts -> @typescript/lib-dom/iterable
    //                      lib.es2015.symbol.wellknown.d.ts -> @typescript/lib-es2015/symbol-wellknown
    let components: Vec<&str> = lib_file_name.split('.').collect();
    let mut path = String::new();
    path.push_str("@typescript/lib-");
    if components.len() > 1 {
        path.push_str(components[1]);
    }
    let mut i = 2;
    while i < components.len() && !components[i].is_empty() && components[i] != "d" {
        if i == 2 {
            path.push('/');
        } else {
            path.push('-');
        }
        path.push_str(components[i]);
        i += 1;
    }
    path
}

fn get_inferred_library_name_resolve_from(
    options: &core::CompilerOptions,
    current_directory: &str,
    lib_file_name: &str,
) -> String {
    let containing_directory = if !options.config_file_path.is_empty() {
        tspath::get_directory_path(&options.config_file_path)
    } else {
        current_directory.to_string()
    };
    tspath::combine_paths(
        &containing_directory,
        &[&format!("__lib_node_modules_lookup_{lib_file_name}__.ts")],
    )
}

pub(crate) fn get_mode_for_type_reference_directive_in_file(
    r#ref: &ast::FileReference,
    file_name: &str,
    meta: &ast::SourceFileMetaData,
    options: &core::CompilerOptions,
) -> core::ResolutionMode {
    if r#ref.resolution_mode != core::ResolutionMode::None {
        r#ref.resolution_mode
    } else {
        get_default_resolution_mode_for_file(file_name, meta, options)
    }
}

pub(crate) fn get_default_resolution_mode_for_file(
    file_name: &str,
    meta: &ast::SourceFileMetaData,
    options: &core::CompilerOptions,
) -> core::ResolutionMode {
    if import_syntax_affects_module_resolution(options) {
        ast::get_implied_node_format_for_emit_worker(
            file_name,
            options.get_emit_module_kind(),
            meta.clone(),
        )
    } else {
        core::ResolutionMode::None
    }
}

pub(crate) fn get_mode_for_usage_location(
    store: &ast::AstStore,
    file_name: &str,
    meta: &ast::SourceFileMetaData,
    usage: &ast::StringLiteralLike,
    options: &core::CompilerOptions,
) -> core::ResolutionMode {
    if let Some(parent) = store.parent(*usage) {
        if ast::is_import_declaration(store, parent)
            || store.kind(parent) == ast::Kind::JSImportDeclaration
            || ast::is_export_declaration(store, parent)
        {
            let is_type_only = ast::is_exclusively_type_only_import_or_export(store, parent);
            if is_type_only {
                let attributes = match store.kind(parent) {
                    ast::Kind::ImportDeclaration
                    | ast::Kind::JSImportDeclaration
                    | ast::Kind::ExportDeclaration => store.attributes(parent),
                    _ => None,
                };
                if let Some(attributes) = attributes {
                    let (override_mode, ok) = ast::get_resolution_mode_override(store, attributes);
                    if ok {
                        return override_mode;
                    }
                }
            }
        }
        if ast::is_literal_type_node(store, parent)
            && store
                .parent(parent)
                .as_ref()
                .is_some_and(|parent| ast::is_import_type_node(store, *parent))
        {
            let parent_parent = store.parent(parent).unwrap();
            if let Some(attributes) = store.attributes(parent_parent) {
                let (override_mode, ok) = ast::get_resolution_mode_override(store, attributes);
                if ok {
                    return override_mode;
                }
            }
        }
    }

    if import_syntax_affects_module_resolution(options) {
        return get_emit_syntax_for_usage_location_worker(store, file_name, meta, usage, options);
    }

    core::ResolutionMode::None
}

fn import_syntax_affects_module_resolution(options: &core::CompilerOptions) -> bool {
    let module_resolution = options.get_module_resolution_kind();
    core::MODULE_RESOLUTION_KIND_NODE16 <= module_resolution
        && module_resolution <= core::MODULE_RESOLUTION_KIND_NODE_NEXT
        || options.get_resolve_package_json_exports()
        || options.get_resolve_package_json_imports()
}

pub(crate) fn get_emit_syntax_for_usage_location_worker(
    store: &ast::AstStore,
    file_name: &str,
    meta: &ast::SourceFileMetaData,
    usage: &ast::Node,
    options: &core::CompilerOptions,
) -> core::ResolutionMode {
    if let Some(parent) = store.parent(*usage) {
        if ast::is_require_call(store, parent, false)
            || ast::is_external_module_reference(store, parent)
                && store
                    .parent(parent)
                    .as_ref()
                    .is_some_and(|parent| ast::is_import_equals_declaration(store, *parent))
        {
            return core::ModuleKind::CommonJs;
        }
    }
    let file_emit_mode =
        ast::get_emit_module_format_of_file_worker(file_name, options, meta.clone());
    if let Some(parent) = store.parent(*usage)
        && ast::walk_up_parenthesized_expressions(store, Some(parent))
            .is_some_and(|node| ast::is_import_call(store, node))
    {
        if ast::should_transform_import_call(file_name, options, file_emit_mode) {
            return core::ModuleKind::CommonJs;
        } else {
            return core::ModuleKind::EsNext;
        }
    }
    // If we're in --module preserve on an input file, we know that an import
    // is an import. But if this is a declaration file, we'd prefer to use the
    // impliedNodeFormat. Since we want things to be consistent between the two,
    // we need to issue errors when the user writes ESM syntax in a definitely-CJS
    // file, until/unless declaration emit can indicate a true ESM import. On the
    // other hand, writing CJS syntax in a definitely-ESM file is fine, since declaration
    // emit preserves the CJS syntax.
    if file_emit_mode == core::ModuleKind::CommonJs {
        core::ModuleKind::CommonJs
    } else {
        if file_emit_mode.is_non_node_esm() || file_emit_mode == core::ModuleKind::Preserve {
            return core::ModuleKind::EsNext;
        }
        core::ModuleKind::None
    }
}
