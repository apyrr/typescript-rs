use std::sync::{Arc, Mutex, Once};

use ts_ast as ast;
use ts_collections as collections;
use ts_compiler as compiler;
use ts_core as core;
use ts_ls::Project as CrossProject;
use ts_lsproto as lsproto;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::ata;
use crate::dirty::Cloneable;
use crate::logging::{LogTree, Logger};
use crate::{
    CheckerPoolHandle, CompilerHost, PatternsAndIgnored, ProjectCollectionBuilder,
    ProjectTreeRequest, WatchedFiles, create_resolution_lookup_glob_mapper, new_checker_pool,
    new_compiler_host_handle, new_parse_cache_key, new_watched_files,
};

pub const INFERRED_PROJECT_NAME: &str = "/dev/null/inferred"; // lowercase so toPath is a no-op regardless of settings
pub const HR: &str = "-----------------------------------------------";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum Kind {
    Inferred = 0,
    Configured = 1,
}

pub const KIND_INFERRED: Kind = Kind::Inferred;
pub const KIND_CONFIGURED: Kind = Kind::Configured;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum ProgramUpdateKind {
    None = 0,
    Cloned = 1,
    SameFileNames = 2,
    NewFiles = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PendingReload {
    None = 0,
    FileNames = 1,
    Full = 2,
}

// Project represents a TypeScript project.
// If changing struct fields, also update the Clone method.
pub struct Project {
    pub(crate) kind: Kind,
    pub(crate) current_directory: String,
    pub(crate) config_file_name: String,
    pub(crate) config_file_path: tspath::Path,

    pub(crate) dirty: bool,
    pub(crate) dirty_file_path: tspath::Path,

    pub(crate) host: Option<Arc<CompilerHost>>,
    pub(crate) command_line: Option<tsoptions::ParsedCommandLine>,
    pub(crate) command_line_with_typings_files: Option<tsoptions::ParsedCommandLine>,
    pub(crate) command_line_with_typings_files_once: Once,
    pub(crate) program: Option<Arc<compiler::Program>>,
    // The kind of update that was performed on the program last time it was updated.
    pub(crate) program_update_kind: ProgramUpdateKind,
    // The ID of the snapshot that created the program stored in this project.
    pub(crate) program_last_update: u64,
    // Set of projects that this project could be referencing.
    // Only set before actually loading config file to get actual project references
    pub(crate) potential_project_references: Option<collections::Set<tspath::Path>>,

    pub(crate) program_files_watch: Option<WatchedFiles<collections::SyncSet<tspath::Path>>>,
    pub(crate) typings_watch: Option<WatchedFiles<PatternsAndIgnored>>,

    pub(crate) checker_pool: Option<CheckerPoolHandle>,

    // installedTypingsInfo is the value of `project.ComputeTypingsInfo()` that was
    // used during the most recently completed typings installation.
    pub(crate) installed_typings_info: Option<ata::TypingsInfo>,
    // typingsFiles are the root files added by the typings installer.
    pub(crate) typings_files: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ProjectInfo {
    pub id: tspath::Path,
    pub kind: Kind,
    pub name: String,
    pub root_files: Vec<String>,
    pub compiler_options: Option<core::CompilerOptions>,
    pub program_id: Option<u64>,
}

impl PartialEq for Project {
    fn eq(&self, other: &Self) -> bool {
        self.config_file_path == other.config_file_path
            && self.program_last_update == other.program_last_update
            && self.program_update_kind == other.program_update_kind
    }
}

impl Default for Project {
    fn default() -> Self {
        Self {
            kind: Kind::Inferred,
            current_directory: String::new(),
            config_file_name: String::new(),
            config_file_path: tspath::Path::default(),
            dirty: false,
            dirty_file_path: tspath::Path::default(),
            host: None,
            command_line: None,
            command_line_with_typings_files: None,
            command_line_with_typings_files_once: Once::new(),
            program: None,
            program_update_kind: ProgramUpdateKind::None,
            program_last_update: 0,
            potential_project_references: None,
            program_files_watch: None,
            typings_watch: None,
            checker_pool: None,
            installed_typings_info: None,
            typings_files: Vec::new(),
        }
    }
}

impl Clone for Project {
    fn clone(&self) -> Self {
        self.clone_project_preserving_update_kind()
    }
}

impl Cloneable<Project> for Project {
    fn clone_value(&self) -> Project {
        self.clone_project()
    }
}

pub struct CreateProgramResult {
    pub program: Arc<compiler::Program>,
    pub update_kind: ProgramUpdateKind,
    pub checker_pool: Option<CheckerPoolHandle>,
}

pub fn new_configured_project(
    config_file_name: String,
    _config_file_path: tspath::Path,
    builder: ProjectCollectionBuilder,
    logger: Option<&LogTree>,
) -> Project {
    new_project(
        config_file_name.clone(),
        Kind::Configured,
        tspath::get_directory_path(&config_file_name),
        builder,
        logger,
    )
}

pub fn new_inferred_project(
    current_directory: String,
    compiler_options: Option<core::CompilerOptions>,
    root_file_names: Vec<String>,
    builder: ProjectCollectionBuilder,
    logger: Option<&LogTree>,
) -> Project {
    let mut p = new_project(
        INFERRED_PROJECT_NAME.to_string(),
        Kind::Inferred,
        current_directory.clone(),
        builder.clone(),
        logger,
    );
    let compiler_options =
        compiler_options.unwrap_or_else(default_inferred_project_compiler_options);
    p.command_line = Some(tsoptions::new_parsed_command_line(
        compiler_options,
        root_file_names,
        tspath::ComparePathsOptions {
            use_case_sensitive_file_names: builder.fs.fs().use_case_sensitive_file_names(),
            current_directory,
            ..Default::default()
        },
    ));
    p
}

fn default_inferred_project_compiler_options() -> core::CompilerOptions {
    core::CompilerOptions {
        allow_js: core::Tristate::True,
        module: core::ModuleKind::ESNext,
        module_resolution: core::ModuleResolutionKind::Bundler,
        target: core::SCRIPT_TARGET_LATEST_STANDARD,
        jsx: core::JsxEmit::ReactJSX,
        allow_importing_ts_extensions: core::Tristate::True,
        strict_null_checks: core::Tristate::True,
        strict_function_types: core::Tristate::True,
        source_map: core::Tristate::True,
        allow_non_ts_extensions: core::Tristate::True,
        resolve_json_module: core::Tristate::True,
        ..Default::default()
    }
}

pub fn new_project(
    config_file_name: String,
    kind: Kind,
    current_directory: String,
    builder: ProjectCollectionBuilder,
    logger: Option<&LogTree>,
) -> Project {
    if let Some(logger) = logger {
        logger.logf(format!(
            "Creating {}Project: {}, currentDirectory: {}",
            kind, config_file_name, current_directory
        ));
    }

    let config_file_path = tspath::to_path(
        &config_file_name,
        &current_directory,
        builder.fs.fs().use_case_sensitive_file_names(),
    );

    let program_files_watch = Some(new_watched_files(
        format!("program files for {}", config_file_name),
        lsproto::WatchKind::Create | lsproto::WatchKind::Change | lsproto::WatchKind::Delete,
        lsproto::get_client_capabilities(&builder.ctx)
            .workspace
            .did_change_watched_files
            .relative_pattern_support,
        create_resolution_lookup_glob_mapper(
            builder.session_options.current_directory.clone(),
            builder.session_options.default_library_path.clone(),
            current_directory.clone(),
            builder.fs.fs().use_case_sensitive_file_names(),
        ),
    ));

    let typings_watch = if builder.session_options.typings_location.is_empty() {
        None
    } else {
        Some(new_watched_files(
            "typings installer files".to_string(),
            lsproto::WatchKind::Create | lsproto::WatchKind::Change | lsproto::WatchKind::Delete,
            lsproto::get_client_capabilities(&builder.ctx)
                .workspace
                .did_change_watched_files
                .relative_pattern_support,
            core::identity,
        ))
    };

    Project {
        kind,
        current_directory,
        config_file_name,
        config_file_path,
        dirty: true,
        dirty_file_path: tspath::Path::default(),
        host: None,
        command_line: None,
        command_line_with_typings_files: None,
        command_line_with_typings_files_once: Once::new(),
        program: None,
        program_update_kind: ProgramUpdateKind::None,
        program_last_update: 0,
        potential_project_references: None,
        program_files_watch,
        typings_watch,
        checker_pool: None,
        installed_typings_info: None,
        typings_files: Vec::new(),
    }
}

impl Project {
    pub(crate) fn info(&self) -> ProjectInfo {
        ProjectInfo {
            id: self.id(),
            kind: self.kind(),
            name: self.name(),
            root_files: self
                .command_line()
                .map(|command_line| command_line.file_names().to_vec())
                .unwrap_or_default(),
            compiler_options: self
                .command_line()
                .map(|command_line| command_line.compiler_options()),
            program_id: self.get_program().map(|program| program.id()),
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn name(&self) -> String {
        self.config_file_name.clone()
    }

    // DisplayName returns a short, human-readable name for the project,
    // relative to the given workspace root directory.
    // For configured projects, this is the config file path made relative.
    // For inferred projects, this is the last component of the current directory.
    pub fn display_name(&self, cwd: &str) -> String {
        if self.kind == Kind::Inferred {
            return tspath::get_base_file_name(&self.current_directory);
        }
        tspath::convert_to_relative_path(
            &self.config_file_name,
            &tspath::ComparePathsOptions {
                current_directory: cwd.to_string(),
                ..Default::default()
            },
        )
    }

    pub fn id(&self) -> tspath::Path {
        self.config_file_path.clone()
    }

    // ConfigFileName panics if Kind() is not KindConfigured.
    pub fn config_file_name(&self) -> String {
        if self.kind != Kind::Configured {
            panic!("ConfigFileName called on non-configured project");
        }
        self.config_file_name.clone()
    }

    // ConfigFilePath panics if Kind() is not KindConfigured.
    pub fn config_file_path(&self) -> tspath::Path {
        if self.kind != Kind::Configured {
            panic!("ConfigFilePath called on non-configured project");
        }
        self.config_file_path.clone()
    }

    pub fn get_program(&self) -> Option<&compiler::Program> {
        self.program.as_deref()
    }

    pub fn command_line(&self) -> Option<&tsoptions::ParsedCommandLine> {
        self.command_line.as_ref()
    }

    // GetProjectDiagnostics returns program diagnostics combined with any global
    // diagnostics discovered during checking. These are the diagnostics reported on
    // the tsconfig.json file.
    pub fn get_project_diagnostics(&self, _ctx: &core::Context) -> Vec<ast::Diagnostic> {
        let global_diags = self
            .checker_pool
            .as_ref()
            .map(|checker_pool| checker_pool.get_global_diagnostics())
            .unwrap_or_default();
        let program_diags = self
            .program
            .as_ref()
            .map(|program| program.get_program_diagnostics())
            .unwrap_or_default();
        compiler::sort_and_deduplicate_diagnostics(core::concatenate(&program_diags, &global_diags))
    }

    pub fn has_file(&self, file_name: &str) -> bool {
        self.contains_file(self.to_path(file_name))
    }

    pub fn contains_file(&self, path: tspath::Path) -> bool {
        self.program
            .as_ref()
            .and_then(|program| program.get_source_file_by_path(path))
            .is_some()
    }

    pub fn is_source_from_project_reference(&self, path: tspath::Path) -> bool {
        self.program
            .as_ref()
            .is_some_and(|program| program.is_source_from_project_reference(path))
    }

    pub fn clone_project(&self) -> Project {
        Project {
            kind: self.kind,
            current_directory: self.current_directory.clone(),
            config_file_name: self.config_file_name.clone(),
            config_file_path: self.config_file_path.clone(),
            dirty: self.dirty,
            dirty_file_path: self.dirty_file_path.clone(),
            host: self.host.clone(),
            command_line: self.command_line.clone(),
            command_line_with_typings_files: self.command_line_with_typings_files.clone(),
            command_line_with_typings_files_once: Once::new(),
            program: self.program.clone(),
            program_update_kind: ProgramUpdateKind::None,
            program_last_update: self.program_last_update,
            potential_project_references: self.potential_project_references.clone(),
            program_files_watch: self.program_files_watch.clone(),
            typings_watch: self.typings_watch.clone(),
            checker_pool: self.checker_pool.clone(),
            installed_typings_info: self.installed_typings_info.clone(),
            typings_files: self.typings_files.clone(),
        }
    }

    pub fn clone_project_preserving_update_kind(&self) -> Project {
        let mut project = self.clone_project();
        project.program_update_kind = self.program_update_kind;
        project
    }

    // getCommandLineWithTypingsFiles returns the command line augmented with typing files if ATA is enabled.
    pub fn get_command_line_with_typings_files(&mut self) -> Option<tsoptions::ParsedCommandLine> {
        if self.typings_files.is_empty() {
            return self.command_line.clone();
        }

        // Check if ATA is enabled for this project
        let type_acquisition = self.get_type_acquisition();
        if type_acquisition.is_none() || !type_acquisition.as_ref().unwrap().enable.is_true() {
            return self.command_line.clone();
        }

        if self.command_line_with_typings_files.is_none() {
            // Create an augmented command line that includes typing files
            let original_root_names = self.command_line.as_ref().unwrap().file_names();
            let mut new_root_names =
                Vec::with_capacity(original_root_names.len() + self.typings_files.len());
            new_root_names.extend(original_root_names.iter().cloned());
            new_root_names.extend(self.typings_files.clone());

            // Create a new ParsedCommandLine with the augmented root file names
            self.command_line_with_typings_files = Some(tsoptions::new_parsed_command_line(
                self.command_line.as_ref().unwrap().compiler_options(),
                new_root_names,
                tspath::ComparePathsOptions {
                    use_case_sensitive_file_names: self
                        .host
                        .as_ref()
                        .unwrap()
                        .source_fs
                        .fs()
                        .use_case_sensitive_file_names(),
                    current_directory: self.current_directory.clone(),
                    ..Default::default()
                },
            ));
        }
        self.command_line_with_typings_files.clone()
    }

    pub fn set_potential_project_reference(&mut self, config_file_path: tspath::Path) {
        let mut refs = self
            .potential_project_references
            .as_ref()
            .cloned()
            .unwrap_or_default();
        refs.add(config_file_path);
        self.potential_project_references = Some(refs);
    }

    pub fn has_potential_project_reference(
        &self,
        project_tree_request: &ProjectTreeRequest,
    ) -> bool {
        if let Some(command_line) = &self.command_line {
            let mut command_line = command_line.clone();
            for path in command_line.resolved_project_reference_paths() {
                if project_tree_request.is_project_referenced(&self.to_path(path)) {
                    return true;
                }
            }
        } else if let Some(potential_project_references) = &self.potential_project_references {
            if let Some(paths) = potential_project_references.keys() {
                for path in paths {
                    if project_tree_request.is_project_referenced(path) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn create_program(&mut self) -> CreateProgramResult {
        let mut update_kind = ProgramUpdateKind::NewFiles;
        let mut program_cloned = false;
        let mut new_program = None;

        let checker_pool_slot: Arc<Mutex<Option<CheckerPoolHandle>>> = Arc::new(Mutex::new(None));
        let create_checker_pool: compiler::CreateCheckerPool = {
            let checker_pool_slot = checker_pool_slot.clone();
            Arc::new(move |_program| {
                let pool = new_checker_pool(4, Option::<fn(String)>::None);
                let handle = pool.handle();
                *checker_pool_slot
                    .lock()
                    .unwrap_or_else(|err| err.into_inner()) = Some(handle);
                Box::new(pool) as Box<dyn compiler::CheckerPool>
            })
        };

        let command_line = self.get_command_line_with_typings_files().unwrap();

        if !self.dirty_file_path.is_empty()
            && self
                .program
                .as_ref()
                .is_some_and(|program| program.command_line() == &command_line)
        {
            let update_host = new_compiler_host_handle(self.host.as_ref().unwrap());
            let (updated_program, cloned) = self.program.as_ref().unwrap().update_program(
                self.dirty_file_path.clone(),
                update_host,
                Some(create_checker_pool.clone()),
            );
            program_cloned = cloned;
            if program_cloned {
                update_kind = ProgramUpdateKind::Cloned;
                let parse_cache = self.host.as_ref().unwrap().parse_cache();
                for file in updated_program.source_files() {
                    if file.path() != self.dirty_file_path {
                        // UpdateProgram acquired the changed file only, so we need to ref everything else
                        parse_cache.r#ref(new_parse_cache_key(
                            file.parse_options(),
                            file.hash(),
                            file.script_kind(),
                        ));
                    }
                }
                for file in updated_program.duplicate_source_files() {
                    parse_cache.r#ref(new_parse_cache_key(
                        file.parse_options,
                        file.hash,
                        file.script_kind,
                    ));
                }
            } else if let Some(new_file) =
                updated_program.get_source_file_by_path(self.dirty_file_path.clone())
            {
                // UpdateProgram always acquires the dirty file before deciding whether it can
                // reuse the old program. If it falls back to a full rebuild, release that
                // speculative acquire so the rebuilt program is the only remaining owner.
                self.host
                    .as_ref()
                    .unwrap()
                    .parse_cache()
                    .deref(&new_parse_cache_key(
                        new_file.parse_options(),
                        new_file.hash(),
                        new_file.script_kind(),
                    ));
            }
            new_program = Some(updated_program);
        }

        if new_program.is_none() {
            let mut typings_location = String::new();
            if self.get_type_acquisition().unwrap().enable.is_true() {
                typings_location = self
                    .host
                    .as_ref()
                    .unwrap()
                    .session_options
                    .typings_location
                    .clone();
            }
            let program_host = new_compiler_host_handle(self.host.as_ref().unwrap());
            new_program = Some(compiler::new_program(compiler::ProgramOptions {
                host: program_host,
                config: Box::new(command_line),
                use_source_of_project_reference: true,
                typings_location,
                create_checker_pool: Some(create_checker_pool),
                single_threaded: core::Tristate::Unknown,
                project_name: self.config_file_name.clone(),
                type_script_version: String::new(),
                tracing: None,
            }));
        }

        let program = new_program.unwrap();

        if !program_cloned
            && self.program.is_some()
            && self.program.as_ref().unwrap().has_same_file_names(&program)
        {
            update_kind = ProgramUpdateKind::SameFileNames;
        }

        program.bind_source_files();
        let program = Arc::new(program);
        let checker_pool = checker_pool_slot
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();

        CreateProgramResult {
            program,
            update_kind,
            checker_pool,
        }
    }

    pub fn clone_watchers(&self) -> WatchedFiles<collections::SyncSet<tspath::Path>> {
        self.program_files_watch.as_ref().unwrap().clone_with(
            self.host
                .as_ref()
                .unwrap()
                .source_fs
                .seen_files()
                .unwrap_or_else(collections::SyncSet::new),
        )
    }

    pub fn log(&self, _msg: &str) {}

    pub fn to_path(&self, file_name: &str) -> tspath::Path {
        tspath::to_path(
            file_name,
            &self.current_directory,
            self.host
                .as_ref()
                .unwrap()
                .source_fs
                .fs()
                .use_case_sensitive_file_names(),
        )
    }

    pub fn print(
        &self,
        write_file_names: bool,
        _write_file_explanation: bool,
        builder: &mut String,
    ) -> String {
        builder.push_str(&format!("\nProject '{}'\n", self.name()));
        if self.program.is_none() {
            builder.push_str("\tFiles (0) NoProgram\n");
        } else {
            let source_files = self.program.as_ref().unwrap().get_source_files();
            builder.push_str(&format!("\tFiles ({})\n", source_files.len()));
            if write_file_names {
                for source_file in source_files {
                    builder.push_str("\t\t");
                    builder.push_str(&source_file.file_name());
                    builder.push('\n');
                }
                // if writeFileExplanation {}
            }
        }
        builder.push_str(HR);
        builder.clone()
    }

    // GetTypeAcquisition returns the type acquisition settings for this project.
    pub fn get_type_acquisition(&self) -> Option<core::TypeAcquisition> {
        if self.kind == Kind::Inferred {
            // For inferred projects, use default settings
            return Some(core::TypeAcquisition {
                enable: core::Tristate::True,
                include: Vec::new(),
                exclude: Vec::new(),
                disable_filename_based_type_acquisition: core::Tristate::False,
            });
        }

        self.command_line
            .as_ref()
            .and_then(|command_line| command_line.type_acquisition())
    }

    // GetUnresolvedImports extracts unresolved imports from this project's program.
    pub fn get_unresolved_imports(&self) -> Option<collections::Set<String>> {
        self.program
            .as_ref()
            .map(|program| program.get_unresolved_imports())
    }

    // ShouldTriggerATA determines if ATA should be triggered for this project.
    pub fn should_trigger_ata(&self, snapshot_id: u64) -> bool {
        if self.program.is_none() || self.command_line.is_none() {
            return false;
        }

        let type_acquisition = self.get_type_acquisition();
        if type_acquisition.is_none() || !type_acquisition.as_ref().unwrap().enable.is_true() {
            return false;
        }

        if self.installed_typings_info.is_none()
            || self.program_last_update == snapshot_id
                && self.program_update_kind == ProgramUpdateKind::NewFiles
        {
            return true;
        }

        !self
            .installed_typings_info
            .as_ref()
            .unwrap()
            .equals(self.compute_typings_info())
    }

    pub fn compute_typings_info(&self) -> ata::TypingsInfo {
        ata::TypingsInfo {
            compiler_options: self.command_line.as_ref().unwrap().compiler_options(),
            type_acquisition: self.get_type_acquisition(),
            unresolved_imports: self.get_unresolved_imports(),
        }
    }
}

impl CrossProject for Project {
    fn id(&self) -> tspath::Path {
        self.id()
    }

    fn get_program(&self) -> Option<&compiler::Program> {
        self.get_program()
    }

    fn has_file(&self, file_name: &str) -> bool {
        self.has_file(file_name)
    }
}
