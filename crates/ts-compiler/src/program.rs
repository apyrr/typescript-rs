use std::any::Any as StdAny;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;
use ts_ast as ast;
use ts_ast::HasFileName as _;
use ts_binder as binder;
use ts_checker as checker;
use ts_collections as collections;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_json as json;
use ts_locale as locale;
use ts_module as module;
use ts_modulespecifiers as modulespecifiers;
use ts_outputpaths as outputpaths;
use ts_packagejson as packagejson;
use ts_parser as parser;
use ts_scanner as scanner;
use ts_sourcemap as sourcemap;
use ts_symlinks as symlinks;
use ts_tracing as tracing;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::checkerpool::{ActiveChecker, CheckerAccess, new_checker_pool};
use crate::emit_host::new_emit_host_with_checker;
use crate::emitter::{emit_source_file, get_declaration_diagnostics, get_source_files_to_emit};
use crate::{
    CheckerPool, CheckerPoolImpl, CompilerHost, DuplicateSourceFile, EMIT_ALL,
    EMIT_ONLY_FORCED_DTS, EmitOnly, FileIncludeReason, IncludeExplainingDiagnostic, LibFile,
    NullCheckerPool, ProcessedFiles, ProcessingDiagnostic, ProcessingDiagnosticData,
    ProcessingDiagnosticKind, get_default_resolution_mode_for_file,
    get_emit_syntax_for_usage_location_worker, get_mode_for_usage_location,
    process_all_program_files, source_file_may_be_emitted, update_file_include_processor,
};

type Context = core::Context;
type Any = ts_diagnostics::Any;
type Error = String;

pub type CreateCheckerPool = Arc<dyn Fn(&Program) -> Box<dyn CheckerPool> + Send + Sync>;

pub fn config_file_parsing_diagnostic(message: String) -> ast::Diagnostic {
    fn split_args(arguments: &str) -> Vec<String> {
        arguments.split('\u{1f}').map(str::to_owned).collect()
    }

    if let Some(arguments) = message.strip_prefix("Argument_for_0_option_must_be_Colon_1: ") {
        let arguments = split_args(arguments);
        let option = arguments
            .first()
            .expect("encoded option diagnostic name")
            .clone();
        let values = arguments
            .get(1)
            .expect("encoded option diagnostic values")
            .clone();
        return ast::new_compiler_diagnostic(
            &diagnostics::Argument_for_0_option_must_be_Colon_1,
            &[
                Box::new(option) as diagnostics::Argument,
                Box::new(values) as diagnostics::Argument,
            ],
        );
    }
    if let Some(arguments) = message.strip_prefix("Cannot_read_file_0: ") {
        return ast::new_compiler_diagnostic(
            &diagnostics::Cannot_read_file_0,
            &[Box::new(arguments.to_owned()) as diagnostics::Argument],
        );
    }
    if let Some(config_file_name) = message
        .strip_prefix("The root value of a ")
        .and_then(|message| message.strip_suffix(" file must be an object"))
    {
        return ast::new_compiler_diagnostic(
            &diagnostics::The_root_value_of_a_0_file_must_be_an_object,
            &[Box::new(config_file_name.to_owned()) as diagnostics::Argument],
        );
    }
    if message == diagnostics::Property_assignment_expected.string() {
        return ast::new_compiler_diagnostic(
            &diagnostics::Property_assignment_expected,
            &[] as &[diagnostics::Argument],
        );
    }
    if message == diagnostics::String_literal_with_double_quotes_expected.string() {
        return ast::new_compiler_diagnostic(
            &diagnostics::String_literal_with_double_quotes_expected,
            &[] as &[diagnostics::Argument],
        );
    }
    if message
        == diagnostics::Property_value_can_only_be_string_literal_numeric_literal_true_false_null_object_literal_or_array_literal.string()
    {
        return ast::new_compiler_diagnostic(
            &diagnostics::Property_value_can_only_be_string_literal_numeric_literal_true_false_null_object_literal_or_array_literal,
            &[] as &[diagnostics::Argument],
        );
    }
    if let Some(arguments) = message.strip_prefix(
        "No_inputs_were_found_in_config_file_0_Specified_include_paths_were_1_and_exclude_paths_were_2: ",
    ) {
        let arguments = split_args(arguments);
        let config_file_name = arguments
            .first()
            .expect("encoded no-input diagnostic config file name")
            .clone();
        let include_specs = arguments
            .get(1)
            .expect("encoded no-input diagnostic include specs")
            .clone();
        let exclude_specs = arguments
            .get(2)
            .expect("encoded no-input diagnostic exclude specs")
            .clone();
        return ast::new_compiler_diagnostic(
            &diagnostics::No_inputs_were_found_in_config_file_0_Specified_include_paths_were_1_and_exclude_paths_were_2,
            &[
                Box::new(config_file_name) as diagnostics::Argument,
                Box::new(include_specs) as diagnostics::Argument,
                Box::new(exclude_specs) as diagnostics::Argument,
            ],
        );
    }
    if let Some(argument) = message.strip_prefix("Unknown_compiler_option_0: ") {
        return ast::new_compiler_diagnostic(
            &diagnostics::Unknown_compiler_option_0,
            &[Box::new(argument.to_owned()) as diagnostics::Argument],
        );
    }
    if let Some(arguments) = message.strip_prefix("Unknown_compiler_option_0_Did_you_mean_1: ") {
        let arguments = split_args(arguments);
        let option = arguments
            .first()
            .expect("encoded unknown-option diagnostic option")
            .clone();
        let suggestion = arguments
            .get(1)
            .expect("encoded unknown-option diagnostic suggestion")
            .clone();
        return ast::new_compiler_diagnostic(
            &diagnostics::Unknown_compiler_option_0_Did_you_mean_1,
            &[
                Box::new(option) as diagnostics::Argument,
                Box::new(suggestion) as diagnostics::Argument,
            ],
        );
    }
    if message == "Unknown option 'excludes'. Did you mean 'exclude'?" {
        return ast::new_compiler_diagnostic(
            &diagnostics::Unknown_option_excludes_Did_you_mean_exclude,
            &[] as &[diagnostics::Argument],
        );
    }
    if let Some(config_file_name) = message
        .strip_prefix("The files list in config file ")
        .and_then(|message| message.strip_suffix(" is empty"))
    {
        return ast::new_compiler_diagnostic(
            &diagnostics::The_files_list_in_config_file_0_is_empty,
            &[Box::new(config_file_name.to_owned()) as diagnostics::Argument],
        );
    }
    if message == "Circularity detected while resolving configuration" {
        return ast::new_compiler_diagnostic(
            &diagnostics::Circularity_detected_while_resolving_configuration_Colon_0,
            &[] as &[diagnostics::Argument],
        );
    }
    panic!("unhandled config file parsing diagnostic: {message}");
}

fn compare_version_text(a: &str, b: &str) -> std::cmp::Ordering {
    fn parse_part(part: Option<&str>) -> u32 {
        part.unwrap_or_default()
            .split_once('-')
            .map(|(part, _)| part)
            .unwrap_or_else(|| part.unwrap_or_default())
            .parse()
            .unwrap_or(0)
    }

    let mut a_parts = a.split('.');
    let mut b_parts = b.split('.');
    for _ in 0..3 {
        let ordering = parse_part(a_parts.next()).cmp(&parse_part(b_parts.next()));
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    std::cmp::Ordering::Equal
}

#[derive(Clone)]
pub struct ProgramOptions {
    pub host: Arc<dyn CompilerHost>,
    pub config: Box<tsoptions::ParsedCommandLine>,
    pub use_source_of_project_reference: bool,
    pub single_threaded: core::Tristate,
    pub create_checker_pool: Option<CreateCheckerPool>,
    pub typings_location: String,
    pub project_name: String,
    pub type_script_version: String,
    pub tracing: Option<tracing::Tracing>,
}

impl ProgramOptions {
    pub fn can_use_project_reference_source(&self) -> bool {
        self.use_source_of_project_reference
            && !self
                .config
                .compiler_options()
                .disable_source_of_project_reference_redirect
                .is_true()
    }
}

struct LazyValue<T> {
    value: OnceLock<Option<T>>,
    initialized: AtomicBool,
}

struct ObjectPool<T, F>
where
    F: Fn() -> T,
{
    new: F,
    values: Mutex<Vec<T>>,
}

impl<T, F> ObjectPool<T, F>
where
    F: Fn() -> T,
{
    fn new(new: F) -> Self {
        Self {
            new,
            values: Mutex::new(Vec::new()),
        }
    }

    fn get(&self) -> T {
        self.values
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pop()
            .unwrap_or_else(|| (self.new)())
    }

    fn put(&self, value: T) {
        self.values
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(value);
    }
}

pub fn outputpaths_compiler_options(
    options: &core::CompilerOptions,
) -> outputpaths::CompilerOptions {
    options.clone()
}

fn outputpaths_source_file(source_file: &ast::SourceFile) -> outputpaths::SourceFile {
    source_file.share_readonly()
}

fn compare_diagnostics_ordering(a: &ast::Diagnostic, b: &ast::Diagnostic) -> std::cmp::Ordering {
    ast::compare_diagnostics(a, b).cmp(&0)
}

impl<T: Clone> LazyValue<T> {
    fn get_value(&self, compute: impl FnOnce() -> T) -> T {
        self.value.get_or_init(|| Some(compute()));
        self.initialized.store(true, Ordering::SeqCst);
        self.value.get().and_then(Clone::clone).unwrap()
    }

    fn try_reuse(&self, from: &LazyValue<T>) {
        if from.initialized.load(Ordering::SeqCst) {
            if let Some(value) = from.value.get().cloned() {
                let _ = self.value.set(value);
                self.initialized.store(true, Ordering::SeqCst);
            }
        }
    }
}

#[derive(Clone)]
struct PackageNamesInfo {
    resolved: collections::Set<String>,
    unresolved: collections::Set<String>,
    deep_import_packages: collections::Set<String>,
}

#[derive(Clone)]
struct BoundSourceFileBindingState {
    root: ast::Node,
    state: Arc<binder::ProgramBindingState>,
}

struct ProgramBindingState {
    files: Vec<BoundSourceFileBindingState>,
    by_path: HashMap<tspath::Path, usize>,
    by_root: HashMap<ast::Node, usize>,
}

impl ProgramBindingState {
    fn new(parsed_files: &[ProgramSourceFile], previous: Option<&ProgramBindingState>) -> Self {
        let mut files = Vec::with_capacity(parsed_files.len());
        let mut by_path = HashMap::with_capacity(parsed_files.len());
        let mut by_root = HashMap::with_capacity(parsed_files.len());

        for file in parsed_files {
            let file = file.as_source_file();
            let root = file.as_node();
            let path = file.path();
            let state = previous
                .and_then(|previous| previous.get_by_source_file(file))
                .cloned()
                .unwrap_or_else(|| {
                    Arc::new(binder::bind_source_file_view(
                        &ast::SourceFileView::from_source_file(file),
                    ))
                });
            let index = files.len();
            by_path.insert(path.clone(), index);
            by_root.insert(root, index);
            files.push(BoundSourceFileBindingState { root, state });
        }

        Self {
            files,
            by_path,
            by_root,
        }
    }

    fn get_by_source_file(
        &self,
        source_file: &ast::SourceFile,
    ) -> Option<&Arc<binder::ProgramBindingState>> {
        let root = source_file.as_node();
        if let Some(index) = self.by_root.get(&root) {
            return self.files.get(*index).map(|file| &file.state);
        }

        let path = source_file.path();
        self.by_path.get(&path).and_then(|index| {
            self.files
                .get(*index)
                .filter(|file| file.root == root)
                .map(|file| &file.state)
        })
    }

    fn bind_diagnostics(&self, file: &ast::SourceFile) -> &[ast::Diagnostic] {
        &self
            .get_by_source_file(file)
            .unwrap_or_else(|| {
                panic!(
                    "source file `{}` is not part of this program binding state",
                    file.file_name()
                )
            })
            .bind_diagnostics()
    }

    fn bind_suggestion_diagnostics(&self, file: &ast::SourceFile) -> &[ast::Diagnostic] {
        &self
            .get_by_source_file(file)
            .unwrap_or_else(|| {
                panic!(
                    "source file `{}` is not part of this program binding state",
                    file.file_name()
                )
            })
            .bind_suggestion_diagnostics()
    }

    fn symbol_count(&self) -> usize {
        self.files
            .iter()
            .map(|file| file.state.symbol_count() as usize)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ts_parser as parser;
    use ts_vfs::vfstest;

    use super::*;

    #[test]
    fn program_binding_state_should_not_populate_source_file_bind_once_cache() {
        let file_name = "/programBindOnce.ts".to_owned();
        let path = tspath::to_path(&file_name, "/", true);
        let source_file = parser::parse_source_file_as_parsed(
            ast::SourceFileParseOptions {
                file_name,
                path,
                ..Default::default()
            },
            "namespace foo {}\nimport provide = foo;\n".to_owned(),
            core::ScriptKind::TS,
        )
        .into_source_file();
        let program_file = ProgramSourceFile::new(source_file.share_readonly());

        let program_state = ProgramBindingState::new(&[program_file], None);
        let program_binding = program_state
            .get_by_source_file(&source_file)
            .expect("source file should be present in program binding state");
        let cached_binding = binder::bind_source_file(&source_file);

        assert_eq!(
            program_binding.symbol_count(),
            cached_binding.symbol_count()
        );
        assert_eq!(
            program_binding.bind_diagnostics().len(),
            cached_binding.bind_diagnostics().len()
        );
        assert_eq!(
            program_binding.bind_suggestion_diagnostics().len(),
            cached_binding.bind_suggestion_diagnostics().len()
        );
        assert!(!Arc::ptr_eq(program_binding, &cached_binding));
    }

    #[test]
    fn collect_checker_diagnostics_from_files_should_use_grouped_compiler_pool() {
        let program = test_program_with_checker_count(
            [
                ("a.ts", "export const a = 1;"),
                ("b.ts", "export const b = 1;"),
                ("c.ts", "export const c = 1;"),
                ("d.ts", "export const d = 1;"),
            ],
            2,
        );
        let visits = Mutex::new(Vec::new());

        let diagnostics = program.collect_checker_diagnostics_from_files(
            core::Context::default(),
            &program.source_files,
            |_, checker, file| {
                visits.lock().unwrap_or_else(|err| err.into_inner()).push((
                    std::thread::current().id(),
                    crate::checkerpool::checker_slot_index_from_state_identity(
                        checker.state_identity(),
                    ),
                    file.file_name(),
                ));
                Vec::new()
            },
        );

        let visits = visits.into_inner().unwrap_or_else(|err| err.into_inner());
        let visited_threads = visits
            .iter()
            .map(|(thread_id, _, _)| *thread_id)
            .collect::<std::collections::HashSet<_>>();
        let mut files_by_checker = HashMap::<usize, Vec<String>>::new();
        for (_, checker_index, file_name) in visits {
            files_by_checker
                .entry(checker_index)
                .or_default()
                .push(file_name);
        }

        assert_eq!(diagnostics.len(), 4);
        assert_eq!(visited_threads.len(), 2);
        assert_eq!(
            files_by_checker.get(&0),
            Some(&vec!["c:/src/a.ts".to_string(), "c:/src/c.ts".to_string()])
        );
        assert_eq!(
            files_by_checker.get(&1),
            Some(&vec!["c:/src/b.ts".to_string(), "c:/src/d.ts".to_string()])
        );
    }

    #[test]
    fn collect_checker_diagnostics_from_files_should_keep_skipped_files_empty() {
        let program = test_program_with_checker_count(
            [
                ("checked.ts", "export const value: string = 1;"),
                (
                    "skipped.ts",
                    "// @ts-nocheck\nexport const value: string = 1;",
                ),
            ],
            2,
        );
        let visits = Mutex::new(Vec::new());

        let diagnostics = program.collect_checker_diagnostics_from_files(
            core::Context::default(),
            &program.source_files,
            |_, _, file| {
                visits
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push(file.file_name());
                vec![ast::new_diagnostic(
                    Some(file),
                    core::TextRange::default(),
                    &diagnostics::Cannot_find_name_0,
                    &[Box::new("synthetic")],
                )]
            },
        );

        let visits = visits.into_inner().unwrap_or_else(|err| err.into_inner());
        assert_eq!(visits, vec!["c:/src/checked.ts".to_string()]);
        assert_eq!(diagnostics[0].len(), 1);
        assert!(diagnostics[1].is_empty());
    }

    #[test]
    fn sort_and_deduplicate_diagnostics_should_merge_related_info_deterministically() {
        let diagnostics = sort_and_deduplicate_diagnostics(vec![
            diagnostic_with_related_info("value", &[30]),
            diagnostic_with_related_info("value", &[10]),
            diagnostic_with_related_info("value", &[30]),
        ]);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(related_positions(&diagnostics[0]), vec![10, 30]);
    }

    #[test]
    fn sort_and_deduplicate_diagnostics_should_ignore_parallel_arrival_order() {
        let forward = sort_and_deduplicate_diagnostics(vec![
            diagnostic_with_related_info("value", &[30]),
            diagnostic_with_related_info("value", &[10]),
            diagnostic_with_related_info("value", &[20, 10]),
        ]);
        let reverse = sort_and_deduplicate_diagnostics(vec![
            diagnostic_with_related_info("value", &[20, 10]),
            diagnostic_with_related_info("value", &[10]),
            diagnostic_with_related_info("value", &[30]),
        ]);

        assert_eq!(forward.len(), 1);
        assert_eq!(reverse.len(), 1);
        assert!(ast::equal_diagnostics(&forward[0], &reverse[0]));
        assert_eq!(related_positions(&forward[0]), vec![10, 20, 30]);
    }

    fn diagnostic_with_related_info(name: &str, related_positions: &[i32]) -> ast::Diagnostic {
        let mut diagnostic = ast::new_diagnostic(
            None,
            core::TextRange::new(5, 6),
            &diagnostics::Cannot_find_name_0,
            &[Box::new(name.to_owned())],
        );
        for pos in related_positions {
            diagnostic.add_related_info(Some(ast::new_diagnostic(
                None,
                core::TextRange::new(*pos, *pos + 1),
                &diagnostics::Non_simple_parameter_declared_here,
                &[],
            )));
        }
        diagnostic
    }

    fn related_positions(diagnostic: &ast::Diagnostic) -> Vec<i32> {
        diagnostic
            .related_information()
            .iter()
            .map(|diagnostic| diagnostic.pos())
            .collect()
    }

    fn test_program_with_checker_count<const N: usize>(
        files: [(&str, &str); N],
        checker_count: usize,
    ) -> Program {
        let file_names = files
            .iter()
            .map(|(name, _)| format!("c:/src/{name}"))
            .collect::<Vec<_>>();
        let fs = vfstest::from_map(
            file_names
                .iter()
                .zip(files.iter())
                .map(|(file_name, (_, text))| (file_name.as_str(), *text)),
            false,
        );
        let host: Arc<dyn CompilerHost> = crate::new_compiler_host(
            "c:/src".to_string(),
            Box::new(fs),
            "c:/lib".to_string(),
            None,
            None,
        )
        .into();
        let mut config = tsoptions::ParsedCommandLine {
            file_names,
            ..Default::default()
        };
        config.set_compiler_options(core::CompilerOptions {
            no_lib: core::TS_TRUE,
            checkers: Some(checker_count),
            ..Default::default()
        });

        new_program(ProgramOptions {
            host,
            config: Box::new(config),
            use_source_of_project_reference: false,
            single_threaded: core::Tristate::default(),
            create_checker_pool: None,
            typings_location: String::new(),
            project_name: String::new(),
            type_script_version: String::new(),
            tracing: None,
        })
    }
}

fn share_source_file_refs(files: Vec<&ast::SourceFile>) -> Vec<ast::SourceFile> {
    files
        .into_iter()
        .map(ast::SourceFile::share_readonly)
        .collect()
}

pub struct ProgramSourceFile {
    source_file: ast::SourceFile,
}

impl ProgramSourceFile {
    fn new(source_file: ast::SourceFile) -> Self {
        Self { source_file }
    }

    fn from_source_file(source_file: &ast::SourceFile) -> Self {
        Self::new(source_file.share_readonly())
    }

    pub fn as_source_file(&self) -> &ast::SourceFile {
        &self.source_file
    }

    pub fn share_source_file(&self) -> ast::SourceFile {
        self.source_file.share_readonly()
    }
}

impl PartialEq for ProgramSourceFile {
    fn eq(&self, other: &Self) -> bool {
        self.as_source_file() == other.as_source_file()
    }
}

impl Eq for ProgramSourceFile {}

impl Hash for ProgramSourceFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(self.as_source_file(), state);
    }
}

impl ast::HasFileName for ProgramSourceFile {
    fn file_name(&self) -> String {
        self.as_source_file().file_name()
    }

    fn path(&self) -> tspath::Path {
        self.as_source_file().path()
    }
}

impl ast::SourceFileLike for ProgramSourceFile {
    fn text(&self) -> String {
        self.as_source_file().text().to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.as_source_file().data().ecma_line_map()
    }
}

impl ast::SourceFileStoreLike for ProgramSourceFile {
    fn store(&self) -> &ast::AstStore {
        self.as_source_file().store()
    }

    fn as_node(&self) -> ast::Node {
        self.as_source_file().as_node()
    }
}

trait ProgramSourceFileRef {
    fn as_program_source_file(&self) -> &ast::SourceFile;
}

impl ProgramSourceFileRef for ProgramSourceFile {
    fn as_program_source_file(&self) -> &ast::SourceFile {
        self.as_source_file()
    }
}

impl ProgramSourceFileRef for ast::SourceFile {
    fn as_program_source_file(&self) -> &ast::SourceFile {
        self
    }
}

fn wrap_program_source_files(files: &[ast::SourceFile]) -> Vec<ProgramSourceFile> {
    files
        .iter()
        .map(ProgramSourceFile::from_source_file)
        .collect()
}

fn share_program_source_files(files: &[ProgramSourceFile]) -> Vec<ProgramSourceFile> {
    files
        .iter()
        .map(|file| ProgramSourceFile::from_source_file(file.as_source_file()))
        .collect()
}

pub struct Program {
    pub(crate) opts: ProgramOptions,
    pub(crate) checker_pool: Box<dyn CheckerPool>,

    // compilerCheckerPool is set only when the built-in compiler checker pool is in use
    // (i.e. CreateCheckerPool was not provided). It enables grouped parallel iteration
    // and direct global diagnostics collection.
    pub(crate) compiler_checker_pool: Option<CheckerPoolImpl>,

    pub(crate) compare_paths_options: tspath::ComparePathsOptions,

    pub(crate) processed_files: ProcessedFiles,
    pub(crate) source_files: Vec<ProgramSourceFile>,

    pub(crate) uses_uri_style_node_core_modules: core::Tristate,

    pub(crate) common_source_directory: OnceLock<String>,

    pub(crate) declaration_diagnostic_cache:
        collections::SyncMap<tspath::Path, Vec<ast::Diagnostic>>,

    pub(crate) program_diagnostics: Mutex<Vec<ast::Diagnostic>>,
    pub(crate) has_emit_blocking_diagnostics: Mutex<collections::Set<tspath::Path>>,

    pub(crate) source_files_to_emit: OnceLock<Vec<ast::SourceFile>>,
    bound_source_files: OnceLock<ProgramBindingState>,

    // Cached unresolved imports for ATA
    pub(crate) unresolved_imports: LazyValue<collections::Set<String>>,
    pub(crate) known_symlinks: LazyValue<symlinks::KnownSymlinks>,

    // Used by auto-imports
    pub(crate) package_names: LazyValue<PackageNamesInfo>,

    // Used by workspace/symbol
    pub(crate) has_ts_file: OnceLock<bool>,

    // Cached map of package names to whether they bundle types
    pub(crate) packages_map: OnceLock<HashMap<String, bool>>,

    pub(crate) compiler_options_cache: OnceLock<core::CompilerOptions>,
}

// FileExists implements checker.Program.
impl Program {
    pub fn id(&self) -> u64 {
        self as *const Self as usize as u64
    }

    pub fn file_exists(&self, path: &str) -> bool {
        self.host().fs().file_exists(path)
    }

    // GetCurrentDirectory implements checker.Program.
    pub fn get_current_directory(&self) -> String {
        self.host().get_current_directory()
    }

    // GetGlobalTypingsCacheLocation implements checker.Program.
    pub fn get_global_typings_cache_location(&self) -> String {
        self.opts.typings_location.clone()
    }

    // GetNearestAncestorDirectoryWithPackageJson implements checker.Program.
    pub fn get_nearest_ancestor_directory_with_package_json(&self, dirname: &str) -> String {
        self.processed_files
            .resolver
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get_package_scope_for_path(dirname)
            .unwrap_or_default()
    }

    // GetPackageJsonInfo implements checker.Program.
    pub fn get_package_json_info(
        &self,
        pkg_json_path: &str,
    ) -> Option<packagejson::InfoCacheEntry> {
        let directory = tspath::get_directory_path(pkg_json_path);
        let scoped = self
            .processed_files
            .resolver
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get_package_scope_for_path(&directory);
        if scoped.as_deref() == Some(directory.as_str()) {
            let (text, ok) = self.host().fs().read_file(pkg_json_path);
            let contents = if ok {
                match packagejson::parse(text.as_bytes()) {
                    Ok(fields) => Some(packagejson::PackageJson::new(fields, true)),
                    Err(_) => Some(packagejson::PackageJson::new(Default::default(), false)),
                }
            } else {
                None
            };
            return Some(packagejson::InfoCacheEntry {
                package_directory: directory,
                directory_exists: true,
                contents,
            });
        }
        None
    }

    // GetRedirectTargets returns the list of file paths that redirect to the given path.
    // These are files from the same package (same name@version) installed in different locations.
    pub fn get_redirect_targets(&self, path: tspath::Path) -> Vec<String> {
        self.processed_files
            .redirect_targets_map
            .get(&path)
            .cloned()
            .unwrap_or_default()
    }

    // gets the original file that was included in program
    // this returns original source file name when including output of project reference
    // otherwise same name
    // Equivalent to originalFileName on SourceFile in Strada
    pub fn get_source_of_project_reference_if_output_included(
        &self,
        file: &dyn ast::HasFileName,
    ) -> String {
        if let Some(source) = self
            .processed_files
            .output_file_to_project_reference_source
            .get(&file.path())
        {
            return source.clone();
        }
        file.file_name().to_string()
    }

    // GetProjectReferenceFromSource implements checker.Program.
    pub fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        self.processed_files
            .project_reference_file_mapper
            .get_project_reference_from_source(path)
    }

    // IsSourceFromProjectReference implements checker.Program.
    pub fn is_source_from_project_reference(&self, path: tspath::Path) -> bool {
        self.processed_files
            .project_reference_file_mapper
            .is_source_from_project_reference(path)
    }

    fn get_project_reference_from_output_dts(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        self.processed_files
            .project_reference_file_mapper
            .get_project_reference_from_output_dts(path)
    }

    fn get_project_reference_from_output_dts_ref(
        &self,
        path: tspath::Path,
    ) -> Option<&tsoptions::SourceOutputAndProjectReference> {
        self.processed_files
            .project_reference_file_mapper
            .output_dts_to_project_reference
            .get(&path)
    }

    fn get_resolved_project_reference_for(
        &self,
        path: tspath::Path,
    ) -> (Option<tsoptions::ParsedCommandLine>, bool) {
        self.processed_files
            .project_reference_file_mapper
            .get_resolved_reference_for(path)
    }

    fn get_redirect_for_resolution(
        &self,
        file: &dyn ast::HasFileName,
    ) -> Option<tsoptions::ParsedCommandLine> {
        let (redirect, _) = self
            .processed_files
            .project_reference_file_mapper
            .get_redirect_for_resolution(file);
        redirect
    }

    fn get_redirect_for_resolution_ref(
        &self,
        file: &dyn ast::HasFileName,
    ) -> Option<&tsoptions::ParsedCommandLine> {
        let mapper = &self.processed_files.project_reference_file_mapper;
        let path = file.path();
        if let Some(output) = mapper.source_to_project_reference.get(&path) {
            return output.resolved.as_deref();
        }
        if let Some(output) = mapper.output_dts_to_project_reference.get(&path) {
            return output.resolved.as_deref();
        }
        let (realpath_dts_to_source, ok) = mapper.realpath_dts_to_source.load(&path);
        if let (Some(realpath_dts_to_source), true) = (realpath_dts_to_source, ok) {
            return mapper
                .output_dts_to_project_reference
                .values()
                .find(|output| {
                    output.source == realpath_dts_to_source.source
                        && output.output_dts == realpath_dts_to_source.output_dts
                })
                .and_then(|output| output.resolved.as_deref());
        }
        None
    }

    pub fn get_parse_file_redirect(&self, file_name: &str) -> String {
        self.processed_files
            .project_reference_file_mapper
            .get_parse_file_redirect(&ast::new_has_file_name(
                file_name.to_string(),
                self.to_path(file_name),
            ))
    }

    pub fn get_resolved_project_references(&self) -> Vec<tsoptions::ParsedCommandLine> {
        self.processed_files
            .project_reference_file_mapper
            .get_resolved_project_references()
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .collect()
    }

    pub fn range_resolved_project_reference(
        &self,
        mut f: impl FnMut(
            tspath::Path,
            Option<tsoptions::ParsedCommandLine>,
            tsoptions::ParsedCommandLine,
            usize,
        ) -> bool,
    ) -> bool {
        self.processed_files
            .project_reference_file_mapper
            .range_resolved_project_reference(|path, config, parent, index| {
                let Some(parent) = parent else {
                    return true;
                };
                f(path, config, parent, index)
            })
    }

    pub fn range_resolved_project_reference_in_child_config(
        &self,
        child_config: &tsoptions::ParsedCommandLine,
        mut f: impl FnMut(
            tspath::Path,
            Option<tsoptions::ParsedCommandLine>,
            tsoptions::ParsedCommandLine,
            usize,
        ) -> bool,
    ) -> bool {
        self.processed_files
            .project_reference_file_mapper
            .range_resolved_project_reference_in_child_config(
                Some(child_config),
                |path, config, parent, index| {
                    let Some(parent) = parent else {
                        return true;
                    };
                    f(path, config, parent, index)
                },
            )
    }

    // UseCaseSensitiveFileNames implements checker.Program.
    pub fn use_case_sensitive_file_names(&self) -> bool {
        self.host().fs().use_case_sensitive_file_names()
    }

    pub fn uses_uri_style_node_core_modules(&self) -> core::Tristate {
        self.uses_uri_style_node_core_modules
    }

    /** This should have similar behavior to 'processSourceFile' without diagnostics or mutation. */
    pub fn get_source_file_from_reference(
        &self,
        origin: &ast::SourceFile,
        r#ref: &ast::FileReference,
    ) -> Option<ast::SourceFile> {
        self.get_source_file_from_reference_ref(origin, r#ref)
            .map(ast::SourceFile::share_readonly)
    }

    pub fn get_source_file_from_reference_ref(
        &self,
        origin: &ast::SourceFile,
        r#ref: &ast::FileReference,
    ) -> Option<&ast::SourceFile> {
        // The module loader in corsa is fairly different than strada; it may eventually expose this functionality,
        // rather than redoing the logic approximately here, since most of the related logic now lives in module.Resolver
        // Still, without the failed lookup reporting that only the loader does, this isn't terribly complicated

        let file_name = tspath::resolve_path(
            &tspath::get_directory_path(&origin.file_name()),
            &[&r#ref.file_name],
        );
        let supported_extensions_base =
            tsoptions::get_supported_extensions(self.options(), &[] /*extraFileExtensions*/);
        let supported_extensions =
            tsoptions::get_supported_extensions_with_json_if_resolve_json_module(
                Some(self.options()),
                supported_extensions_base,
            );
        let allow_non_ts_extensions = self.options().allow_non_ts_extensions.is_true();
        if tspath::has_extension(&file_name) {
            if !allow_non_ts_extensions {
                let canonical_file_name = tspath::get_canonical_file_name(
                    &file_name,
                    self.use_case_sensitive_file_names(),
                );
                let mut supported = false;
                for group in &supported_extensions {
                    let group: Vec<&str> = group.iter().map(String::as_str).collect();
                    if tspath::file_extension_is_one_of(&canonical_file_name, &group) {
                        supported = true;
                        break;
                    }
                }
                if !supported {
                    return None; // unsupported extensions are forced to fail
                }
            }

            return self.get_source_file_ref(&file_name);
        }
        if allow_non_ts_extensions {
            let extensionless = self.get_source_file_ref(&file_name);
            if extensionless.is_some() {
                return extensionless;
            }
        }

        // Only try adding extensions from the first supported group (which should be .ts/.tsx/.d.ts)
        for ext in &supported_extensions[0] {
            let result = self.get_source_file_ref(&(file_name.clone() + ext));
            if result.is_some() {
                return result;
            }
        }
        None
    }
}

pub fn new_program(opts: ProgramOptions) -> Program {
    let mut p = Program::new_empty(opts);
    let pop_trace = p.opts.tracing.as_mut().map(|tracing| {
        tracing.push(
            tracing::PHASE_PROGRAM,
            "createProgram",
            hashmap! {"configFilePath" => p.opts.config.compiler_options().config_file_path.clone()},
            true,
        )
    });
    let processed_program_files = process_all_program_files(p.opts.clone(), p.single_threaded());
    p.processed_files = processed_program_files.processed_files;
    p.source_files = wrap_program_source_files(&processed_program_files.source_files);
    p.init_checker_pool();
    p.verify_compiler_options();
    if let Some(pop_trace) = pop_trace {
        if let Some(tracing) = p.opts.tracing.as_mut() {
            pop_trace(tracing);
        }
    }
    p
}

impl Program {
    // Return an updated program for which it is known that only the file with the given path has changed.
    // In addition to a new program, return a boolean indicating whether the data of the old program was reused.
    // createCheckerPool, if non-nil, overrides the CreateCheckerPool stored in the old program's options,
    // ensuring each caller uses a fresh closure and avoiding data races on captured variables.
    pub fn update_program(
        &self,
        changed_file_path: tspath::Path,
        new_host: Arc<dyn CompilerHost>,
        create_checker_pool: Option<CreateCheckerPool>,
    ) -> (Program, bool) {
        let mut new_opts = self.opts.clone();
        new_opts.host = new_host;
        if create_checker_pool.is_some() {
            new_opts.create_checker_pool = create_checker_pool;
        }

        let old_file_index = self
            .processed_files
            .files_by_path
            .get(&changed_file_path)
            .unwrap();
        let old_file = self.source_files[*old_file_index].as_source_file();
        let new_file = new_opts.host.get_source_file(old_file.parse_options());

        // If this file is part of a package redirect group (same package installed in multiple
        // node_modules locations), we need to rebuild the program because the redirect targets
        // might need recalculation.
        let in_redirect_files = self
            .processed_files
            .redirect_files_by_path
            .contains_key(&changed_file_path);
        let is_redirect_target = self
            .processed_files
            .redirect_targets_map
            .contains_key(&changed_file_path);
        if in_redirect_files || is_redirect_target {
            return (new_program(new_opts), false);
        }

        if !can_replace_file_in_program(old_file, new_file.as_ref()) {
            return (new_program(new_opts), false);
        }
        // Matches Go: compiler options are not reverified for this single-file update path.
        let mut result = Program {
            opts: new_opts,
            compare_paths_options: self.compare_paths_options.clone(),
            processed_files: self.processed_files.clone(),
            uses_uri_style_node_core_modules: self.uses_uri_style_node_core_modules,
            program_diagnostics: Mutex::new(
                self.program_diagnostics
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone(),
            ),
            has_emit_blocking_diagnostics: Mutex::new(
                self.has_emit_blocking_diagnostics
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone(),
            ),
            source_files: share_program_source_files(&self.source_files),
            ..Program::new_empty(self.opts.clone())
        };
        result
            .unresolved_imports
            .try_reuse(&self.unresolved_imports);
        result.known_symlinks.try_reuse(&self.known_symlinks);
        result.package_names.try_reuse(&self.package_names);
        let new_file = new_file.unwrap();
        let index = result
            .source_files
            .iter()
            .position(|file| file.path() == new_file.path())
            .unwrap();
        result.source_files[index] = ProgramSourceFile::new(new_file);
        result.bind_source_files_mut_reusing(self.bound_source_files.get());
        result
            .processed_files
            .files_by_path
            .insert(result.source_files[index].path(), index);
        update_file_include_processor(&mut result);
        result.init_checker_pool();
        (result, true)
    }

    fn init_checker_pool(&mut self) {
        if !self.processed_files.finished_processing {
            panic!("Program must finish processing files before initializing checker pool");
        }

        if let Some(create_checker_pool) = self.opts.create_checker_pool.as_ref() {
            self.checker_pool = create_checker_pool(self);
        } else {
            let pool = new_checker_pool(self);
            self.compiler_checker_pool = Some(pool);
        }
    }
}

fn can_replace_file_in_program(file1: &ast::SourceFile, file2: Option<&ast::SourceFile>) -> bool {
    file2.is_some_and(|file2| {
        file1.parse_options() == file2.parse_options()
            && file1.uses_uri_style_node_core_modules() == file2.uses_uri_style_node_core_modules()
            && file1.imports().len() == file2.imports().len()
            && file1
                .imports()
                .iter()
                .zip(file2.imports())
                .all(|(a, b)| equal_module_specifiers(file1.store(), file2.store(), a, b))
            && file1.module_augmentations().len() == file2.module_augmentations().len()
            && file1
                .module_augmentations()
                .iter()
                .zip(file2.module_augmentations())
                .all(|(a, b)| equal_module_augmentation_names(file1.store(), file2.store(), a, b))
            && file1.ambient_module_names() == file2.ambient_module_names()
            && file1.referenced_files().len() == file2.referenced_files().len()
            && file1
                .referenced_files()
                .iter()
                .zip(file2.referenced_files())
                .all(|(a, b)| equal_file_references(a, b))
            && file1.type_reference_directives().len() == file2.type_reference_directives().len()
            && file1
                .type_reference_directives()
                .iter()
                .zip(file2.type_reference_directives())
                .all(|(a, b)| equal_file_references(a, b))
            && file1.lib_reference_directives().len() == file2.lib_reference_directives().len()
            && file1
                .lib_reference_directives()
                .iter()
                .zip(file2.lib_reference_directives())
                .all(|(a, b)| equal_file_references(a, b))
            && equal_check_jsdirectives(file1.check_js_directive(), file2.check_js_directive())
    })
}

fn equal_module_specifiers(
    store1: &ast::AstStore,
    store2: &ast::AstStore,
    n1: &ast::Node,
    n2: &ast::Node,
) -> bool {
    store1.kind(*n1) == store2.kind(*n2)
        && (!ast::is_string_literal(store1, *n1) || store1.text(*n1) == store2.text(*n2))
}

fn equal_module_augmentation_names(
    store1: &ast::AstStore,
    store2: &ast::AstStore,
    n1: &ast::Node,
    n2: &ast::Node,
) -> bool {
    store1.kind(*n1) == store2.kind(*n2) && store1.text(*n1) == store2.text(*n2)
}

fn equal_file_references(f1: &ast::FileReference, f2: &ast::FileReference) -> bool {
    f1.file_name == f2.file_name
        && f1.resolution_mode == f2.resolution_mode
        && f1.preserve == f2.preserve
}

fn equal_check_jsdirectives(
    d1: Option<&ast::CheckJsDirective>,
    d2: Option<&ast::CheckJsDirective>,
) -> bool {
    d1.is_none() && d2.is_none()
        || d1.is_some() && d2.is_some() && d1.unwrap().enabled == d2.unwrap().enabled
}

impl Program {
    pub fn source_files(&self) -> Vec<ast::SourceFile> {
        self.source_files
            .iter()
            .map(ProgramSourceFile::share_source_file)
            .collect()
    }

    pub fn source_files_for_auto_imports(&self) -> Vec<ast::SourceFile> {
        self.source_files()
    }
    pub fn duplicate_source_files(&self) -> Vec<DuplicateSourceFile> {
        self.processed_files.duplicate_source_files.clone()
    }
    pub fn options(&self) -> &core::CompilerOptions {
        self.compiler_options_cache
            .get_or_init(|| self.opts.config.compiler_options())
    }
    pub fn compiler_options(&self) -> &core::CompilerOptions {
        self.options()
    }
    pub fn command_line(&self) -> &tsoptions::ParsedCommandLine {
        &self.opts.config
    }
    pub fn host(&self) -> &dyn CompilerHost {
        self.opts.host.as_ref()
    }
    pub fn host_arc(&self) -> Arc<dyn CompilerHost> {
        Arc::clone(&self.opts.host)
    }
    pub fn tracing(&self) -> Option<tracing::Tracing> {
        self.opts.tracing.clone()
    }
    pub fn get_config_file_parsing_diagnostics(&self) -> Vec<ast::Diagnostic> {
        let mut diagnostics = self.opts.config.get_config_file_parsing_ast_diagnostics();
        let mut encoded_diagnostics =
            if let Some(config_file) = self.opts.config.config_file.as_ref() {
                config_file
                    .diagnostics
                    .iter()
                    .filter(|message| {
                        !diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.to_string() == message.as_str())
                    })
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
        encoded_diagnostics.extend(self.opts.config.errors.clone());
        diagnostics.extend(
            encoded_diagnostics
                .into_iter()
                .map(config_file_parsing_diagnostic),
        );
        diagnostics
    }

    // GetUnresolvedImports returns the unresolved imports for this program.
    // The result is cached and computed only once.
    pub fn get_unresolved_imports(&self) -> collections::Set<String> {
        self.unresolved_imports
            .get_value(|| self.extract_unresolved_imports())
    }

    fn extract_unresolved_imports(&self) -> collections::Set<String> {
        let mut unresolved_set = collections::Set::new();

        for source_file in &self.source_files {
            let unresolved_imports =
                self.extract_unresolved_imports_from_source_file(source_file.as_source_file());
            for imp in unresolved_imports {
                unresolved_set.add(imp);
            }
        }

        unresolved_set
    }

    fn extract_unresolved_imports_from_source_file(&self, file: &ast::SourceFile) -> Vec<String> {
        let mut unresolved_imports = Vec::new();

        if let Some(resolved_modules) = self.processed_files.resolved_modules.get(&file.path()) {
            for (cache_key, resolution) in resolved_modules {
                let resolved = resolution.is_resolved();
                if (!resolved
                    || !tspath::extension_is_one_of(
                        &resolution.extension,
                        &tspath::supported_ts_extensions_with_json_flat(),
                    ))
                    && !tspath::is_external_module_name_relative(&cache_key.name)
                {
                    unresolved_imports.push(cache_key.name.clone());
                }
            }
        }

        unresolved_imports
    }

    pub fn single_threaded(&self) -> bool {
        self.opts
            .single_threaded
            .default_if_unknown(self.options().single_threaded)
            .is_true()
    }

    pub fn bind_source_files(&self) {
        let _ = self.binding_state();
    }

    fn bind_source_files_mut_reusing(&mut self, previous: Option<&ProgramBindingState>) {
        if self.bound_source_files.get().is_none() {
            let state = ProgramBindingState::new(&self.source_files, previous);
            let _ = self.bound_source_files.set(state);
        }
    }

    fn binding_state(&self) -> &ProgramBindingState {
        self.bound_source_files
            .get_or_init(|| ProgramBindingState::new(&self.source_files, None))
    }

    fn bound_source_files_refs(&self) -> Vec<&ast::SourceFile> {
        self.binding_state();
        self.source_files
            .iter()
            .map(ProgramSourceFile::as_source_file)
            .collect()
    }

    fn get_bound_source_file_by_path_ref(&self, path: &tspath::Path) -> Option<&ast::SourceFile> {
        self.binding_state();
        self.processed_files
            .files_by_path
            .get(path)
            .map(|index| self.source_files[*index].as_source_file())
    }

    fn bind_diagnostics_for_file(&self, file: &ast::SourceFile) -> &[ast::Diagnostic] {
        self.binding_state().bind_diagnostics(file)
    }

    fn bind_suggestion_diagnostics_for_file(&self, file: &ast::SourceFile) -> &[ast::Diagnostic] {
        self.binding_state().bind_suggestion_diagnostics(file)
    }

    fn for_each_checker_parallel(&self, cb: impl Fn(usize, &checker::Checker) + Sync) {
        if let Some(pool) = &self.compiler_checker_pool {
            pool.for_each_checker_parallel(self, |idx, checker| cb(idx, checker));
        }
    }

    // Run a callback with the active checker for the given file. Concurrent
    // programs may use multiple checker slots, and TypeScript object identity is
    // scoped to the selected slot.
    pub fn with_type_checker_for_file_using<'a, 'access, 'checker, 'state, R>(
        &'a self,
        access: CheckerAccess<'a, 'access, 'checker, 'state>,
        file: &'a ast::SourceFile,
        f: impl for<'active_checker, 'active_state> FnOnce(
            &mut ActiveChecker<'a, 'active_checker, 'active_state>,
        ) -> R,
    ) -> R {
        let ctx = match access {
            CheckerAccess::Active(active) => {
                self.validate_active_type_checker_for_file(active, file);
                return f(active);
            }
            CheckerAccess::Context(ctx) => ctx,
        };
        let mut result = None;
        let mut f = Some(f);
        let mut cb = |mut active: ActiveChecker<'a, '_, '_>| {
            let f = f
                .take()
                .expect("checker callback must be invoked exactly once");
            result = Some(f(&mut active));
        };
        if let Some(pool) = &self.compiler_checker_pool {
            pool.with_checker_for_file_non_exclusive(self, file, &mut cb);
        } else {
            self.checker_pool
                .with_checker(self, &ctx, Some(file), &mut cb);
        }
        result.expect("checker callback must produce a result")
    }

    pub fn with_type_checker_for_state_identity_using<'a, 'access, 'checker, 'state, R>(
        &'a self,
        access: CheckerAccess<'a, 'access, 'checker, 'state>,
        identity: checker::CheckerStateIdentity,
        f: impl for<'active_checker, 'active_state> FnOnce(
            &mut ActiveChecker<'a, 'active_checker, 'active_state>,
        ) -> R,
    ) -> R {
        let ctx = match access {
            CheckerAccess::Active(active) => {
                self.validate_active_type_checker_for_state_identity(active, identity);
                return f(active);
            }
            CheckerAccess::Context(ctx) => ctx,
        };
        let mut result = None;
        let mut f = Some(f);
        let mut cb = |mut active: ActiveChecker<'a, '_, '_>| {
            let f = f
                .take()
                .expect("checker callback must be invoked exactly once");
            if identity != active.state_identity() {
                panic!("active checker state identity does not match requested checker identity");
            }
            result = Some(f(&mut active));
        };
        if let Some(pool) = &self.compiler_checker_pool {
            pool.with_checker_for_state_identity_non_exclusive(self, identity, &mut cb);
        } else {
            self.checker_pool
                .with_checker_for_state_identity(self, &ctx, identity, &mut cb);
        }
        result.expect("checker callback must produce a result")
    }

    fn validate_active_type_checker_for_file<'a, 'checker, 'state>(
        &'a self,
        active: &ActiveChecker<'a, 'checker, 'state>,
        file: &'a ast::SourceFile,
    ) {
        self.validate_active_type_checker_for_program(active);
        if let Some(pool) = &self.compiler_checker_pool {
            if !pool.file_matches_state_identity(self, file, active.state_identity()) {
                panic!("active checker is not associated with requested source file");
            }
        }
    }

    fn validate_active_type_checker_for_state_identity<'a, 'checker, 'state>(
        &'a self,
        active: &ActiveChecker<'a, 'checker, 'state>,
        identity: checker::CheckerStateIdentity,
    ) {
        self.validate_active_type_checker_for_program(active);
        if identity != active.state_identity() {
            panic!("active checker state identity does not match requested checker identity");
        }
    }

    fn validate_active_type_checker_for_program<'a, 'checker, 'state>(
        &'a self,
        active: &ActiveChecker<'a, 'checker, 'state>,
    ) {
        if !std::ptr::eq(self, active.program()) {
            panic!("active checker belongs to a different program");
        }
    }

    pub fn with_type_checker_for_file_exclusive<'a, R>(
        &'a self,
        ctx: Context,
        file: &'a ast::SourceFile,
        f: impl for<'checker, 'state> FnOnce(&mut ActiveChecker<'a, 'checker, 'state>) -> R,
    ) -> R {
        let mut result = None;
        let mut f = Some(f);
        let mut cb = |mut active: ActiveChecker<'a, '_, '_>| {
            let f = f
                .take()
                .expect("checker callback must be invoked exactly once");
            let _ = file;
            result = Some(f(&mut active));
        };
        if let Some(pool) = &self.compiler_checker_pool {
            pool.with_checker_for_file_exclusive(&ctx, self, file, &mut cb);
        } else {
            self.checker_pool
                .with_checker(self, &ctx, Some(file), &mut cb);
        }
        result.expect("checker callback must produce a result")
    }

    fn get_resolved_module(
        &self,
        file: &dyn ast::HasFileName,
        module_reference: &str,
        mode: core::ResolutionMode,
    ) -> Option<module::ResolvedModule> {
        self.processed_files
            .resolved_modules
            .get(&file.path())
            .and_then(|resolutions| {
                resolutions
                    .get(&module::ModeAwareCacheKey {
                        name: module_reference.to_string(),
                        mode,
                    })
                    .cloned()
            })
    }

    fn get_resolved_module_ref(
        &self,
        file: &dyn ast::HasFileName,
        module_reference: &str,
        mode: core::ResolutionMode,
    ) -> Option<&module::ResolvedModule> {
        self.processed_files
            .resolved_modules
            .get(&file.path())
            .and_then(|resolutions| {
                resolutions.get(&module::ModeAwareCacheKey {
                    name: module_reference.to_string(),
                    mode,
                })
            })
    }

    pub fn get_resolved_module_from_module_specifier(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> Option<module::ResolvedModule> {
        let source_file = self.get_source_file_by_path_ref(&file.path()).unwrap();
        let module_specifier_store = source_file.store();
        if !ast::is_string_literal_like(module_specifier_store, *module_specifier) {
            panic!("moduleSpecifier must be a StringLiteralLike");
        }
        let mode = self.get_mode_for_usage_location(file, module_specifier);
        self.get_resolved_module(file, &module_specifier_store.text(*module_specifier), mode)
    }

    fn get_resolved_modules(
        &self,
    ) -> HashMap<tspath::Path, module::ModeAwareCache<module::ResolvedModule>> {
        self.processed_files.resolved_modules.clone()
    }

    // GetPackagesMap returns a lazily-cached map of package names to whether they bundle types.
    // This is used by incremental diagnostic repopulation.
    fn get_packages_map(&self) -> HashMap<String, bool> {
        self.packages_map
            .get_or_init(|| {
                let mut packages_map = HashMap::new();
                for resolved_modules_in_file in self.processed_files.resolved_modules.values() {
                    for r#mod in resolved_modules_in_file.values() {
                        if !r#mod.package_id.name.is_empty() {
                            let bundles_types = r#mod.extension == tspath::EXTENSION_DTS;
                            let entry = packages_map
                                .entry(r#mod.package_id.name.clone())
                                .or_insert(false);
                            *entry = *entry || bundles_types;
                        }
                    }
                }
                packages_map
            })
            .clone()
    }

    // collectDiagnostics collects diagnostics from a single file or all files.
    // If sourceFile is non-nil, returns diagnostics for just that file.
    // If sourceFile is nil, returns diagnostics for all files in the program.
    fn collect_diagnostics(
        &self,
        ctx: Context,
        source_file: Option<&ast::SourceFile>,
        concurrent: bool,
        collect: impl Fn(Context, &ast::SourceFile) -> Vec<ast::Diagnostic>,
    ) -> Vec<ast::Diagnostic> {
        let result = if let Some(source_file) = source_file {
            collect(ctx, source_file)
        } else {
            self.collect_diagnostics_from_files(ctx, &self.source_files, concurrent, collect)
                .into_iter()
                .flatten()
                .collect()
        };
        sort_and_deduplicate_diagnostics(result)
    }

    fn collect_diagnostics_from_files(
        &self,
        ctx: Context,
        source_files: &[impl ProgramSourceFileRef],
        _concurrent: bool,
        collect: impl Fn(Context, &ast::SourceFile) -> Vec<ast::Diagnostic>,
    ) -> Vec<Vec<ast::Diagnostic>> {
        let mut diagnostics = vec![Vec::new(); source_files.len()];
        for (i, file) in source_files.iter().enumerate() {
            let file = file.as_program_source_file();
            diagnostics[i] = collect(ctx.clone(), file);
        }
        diagnostics
    }

    // collectCheckerDiagnostics collects diagnostics from a single file or all files,
    // using a callback that receives the checker for each file. When the checker pool
    // supports grouped iteration (compiler pool), files are grouped by checker and
    // processed in parallel with one task per checker, reducing contention and improving
    // cache locality. Otherwise, falls back to per-file concurrent collection.
    fn collect_checker_diagnostics(
        &self,
        ctx: Context,
        source_file: Option<&ast::SourceFile>,
        collect: impl for<'a> Fn(
            Context,
            &mut checker::Checker<'a, '_>,
            &'a ast::SourceFile,
        ) -> Vec<ast::Diagnostic>
        + Copy
        + Sync,
    ) -> Vec<ast::Diagnostic> {
        if let Some(source_file) = source_file {
            if self.skip_type_checking(source_file, false) {
                return Vec::new();
            }
            let result =
                self.with_type_checker_for_file_exclusive(ctx.clone(), source_file, |active| {
                    collect(ctx, active.checker(), source_file)
                });
            return sort_and_deduplicate_diagnostics(result);
        }
        sort_and_deduplicate_diagnostics(
            self.collect_checker_diagnostics_from_files(ctx, &self.source_files, collect)
                .into_iter()
                .flatten()
                .collect(),
        )
    }

    // collectCheckerDiagnosticsFromFiles collects checker diagnostics for a list of files.
    fn collect_checker_diagnostics_from_files(
        &self,
        ctx: Context,
        source_files: &[impl ProgramSourceFileRef],
        collect: impl for<'a> Fn(
            Context,
            &mut checker::Checker<'a, '_>,
            &'a ast::SourceFile,
        ) -> Vec<ast::Diagnostic>
        + Copy
        + Sync,
    ) -> Vec<Vec<ast::Diagnostic>> {
        if let Some(pool) = &self.compiler_checker_pool {
            let source_files = source_files
                .iter()
                .map(ProgramSourceFileRef::as_program_source_file)
                .collect::<Vec<_>>();
            let diagnostics = (0..source_files.len())
                .map(|_| Mutex::new(Vec::new()))
                .collect::<Vec<_>>();
            pool.for_each_checker_group_do(
                self,
                &ctx,
                &source_files,
                self.single_threaded(),
                |checker, file_index, file| {
                    if self.skip_type_checking(file, false) {
                        return;
                    }
                    *diagnostics[file_index]
                        .lock()
                        .unwrap_or_else(|err| err.into_inner()) =
                        collect(ctx.clone(), checker, file);
                },
            );
            return diagnostics
                .into_iter()
                .map(|diagnostics| {
                    diagnostics
                        .into_inner()
                        .unwrap_or_else(|err| err.into_inner())
                })
                .collect();
        }

        let mut diagnostics = vec![Vec::new(); source_files.len()];
        for (i, file) in source_files.iter().enumerate() {
            let file = file.as_program_source_file();
            if self.skip_type_checking(file, false) {
                continue;
            }
            diagnostics[i] =
                self.with_type_checker_for_file_exclusive(ctx.clone(), file, |active| {
                    collect(ctx.clone(), active.checker(), file)
                });
        }
        diagnostics
    }

    pub fn get_syntactic_diagnostics(
        &self,
        ctx: Context,
        source_file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.collect_diagnostics(ctx, source_file, false /*concurrent*/, |_ctx, file| {
            let mut diags = core::concatenate(file.diagnostics(), file.js_diagnostics());
            // For JS files that won't be checked by the checker (no checkJs/ts-check), we need
            // program-level syntactic checks that require compiler options. This mirrors Strada's
            // getJSSyntacticDiagnosticsForFile in program.ts.
            if ast::is_source_file_js(file)
                && !ast::is_check_jsenabled_for_file(file, self.options())
            {
                diags.extend(get_additional_jssyntactic_diagnostics(file, self.options()));
            }
            diags
        })
    }
}

// getAdditionalJSSyntacticDiagnostics produces option-dependent syntactic diagnostics for JS files
// that aren't covered by the parser or the checker. In Strada, the equivalent logic lives in
// getJSSyntacticDiagnosticsForFile in program.ts. In Corsa, most of that function's checks were
// moved into the parser (checkJSSyntax/checkJSDecoratorSyntax), but checks that depend on compiler
// options can't live in the parser and must remain here. The checker handles these for checked files,
// but doesn't run on unchecked JS files (no checkJs/ts-check).
fn get_additional_jssyntactic_diagnostics(
    file: &ast::SourceFile,
    options: &core::CompilerOptions,
) -> Vec<ast::Diagnostic> {
    if options.experimental_decorators.is_true() {
        return Vec::new();
    }
    let mut diags = Vec::new();
    // Parameter decorators are only valid with experimentalDecorators. Without it,
    // the checker would report this, but the checker doesn't run on unchecked JS files.
    fn walk(node: &ast::Node, file: &ast::SourceFile, diags: &mut Vec<ast::Diagnostic>) -> bool {
        let store = file.store();
        if !store
            .subtree_facts(*node)
            .contains(ast::SUBTREE_CONTAINS_DECORATORS)
        {
            return false;
        }
        if store.kind(*node) == ast::Kind::Parameter && ast::has_decorators(store, *node) {
            let decorator = store
                .modifier_nodes(*node)
                .into_iter()
                .find(|decorator| ast::is_decorator(store, *decorator));
            if let Some(decorator) = decorator {
                diags.push(ast::new_diagnostic(
                    Some(file),
                    store.loc(decorator),
                    &diagnostics::Decorators_are_not_valid_here,
                    &[],
                ));
            }
        }
        let _ = store.for_each_present_child(*node, &mut |child| {
            walk(&child, file, diags);
            std::ops::ControlFlow::Continue(())
        });
        false
    }
    let _ = file
        .store()
        .for_each_present_child(file.as_node(), &mut |node| {
            walk(&node, file, &mut diags);
            std::ops::ControlFlow::Continue(())
        });
    diags
}

impl Program {
    pub fn get_bind_diagnostics(
        &self,
        ctx: Context,
        source_file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        if let Some(source_file) = source_file {
            let path = source_file.path();
            let source_file = self
                .get_bound_source_file_by_path_ref(&path)
                .unwrap_or(source_file);
            return self.collect_diagnostics(ctx, Some(source_file), false, |_ctx, file| {
                self.bind_diagnostics_for_file(file).to_vec()
            });
        } else {
            self.bind_source_files();
        }
        self.collect_diagnostics(ctx, source_file, false /*concurrent*/, |_ctx, file| {
            self.bind_diagnostics_for_file(file).to_vec()
        })
    }

    pub fn get_semantic_diagnostics(
        &self,
        ctx: Context,
        source_file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.collect_checker_diagnostics(ctx, source_file, |ctx, c, source_file| {
            self.get_semantic_diagnostics_with_checker(ctx, c, source_file)
        })
    }

    pub fn get_semantic_diagnostics_without_no_emit_filtering(
        &self,
        ctx: Context,
        source_files: &[ast::SourceFile],
    ) -> HashMap<ast::SourceFile, Vec<ast::Diagnostic>> {
        let all_diags =
            self.collect_checker_diagnostics_from_files(ctx, source_files, |ctx, checker, file| {
                self.get_bind_and_check_diagnostics_with_checker(ctx, checker, file)
            });
        let mut result = HashMap::with_capacity(source_files.len());
        for (i, diags) in all_diags.into_iter().enumerate() {
            result.insert(
                source_files[i].share_readonly(),
                sort_and_deduplicate_diagnostics(diags),
            );
        }
        result
    }

    pub fn get_suggestion_diagnostics(
        &self,
        ctx: Context,
        source_file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.collect_checker_diagnostics(ctx, source_file, |ctx, checker, file| {
            self.get_suggestion_diagnostics_with_checker(ctx, checker, file)
        })
    }

    pub fn get_program_diagnostics(&self) -> Vec<ast::Diagnostic> {
        sort_and_deduplicate_diagnostics(core::concatenate(
            &self
                .program_diagnostics
                .lock()
                .unwrap_or_else(|err| err.into_inner()),
            self.processed_files
                .include_processor
                .get_diagnostics(self)
                .get_global_diagnostics()
                .as_slice(),
        ))
    }

    pub fn get_include_processor_diagnostics(
        &self,
        source_file: &ast::SourceFile,
    ) -> Vec<ast::Diagnostic> {
        if self.skip_type_checking(source_file, false) {
            return Vec::new();
        }
        let (filtered, _) = self.get_diagnostics_with_preceding_directives(
            source_file,
            self.processed_files
                .include_processor
                .get_diagnostics(self)
                .get_diagnostics_for_file(&source_file.file_name()),
        );
        filtered
    }

    pub fn skip_type_checking(&self, source_file: &ast::SourceFile, ignore_no_check: bool) -> bool {
        (!ignore_no_check && self.options().no_check.is_true())
            || self.options().skip_lib_check.is_true() && source_file.is_declaration_file()
            || self.options().skip_default_lib_check.is_true()
                && self.is_source_file_default_library(source_file.path())
            || self.is_source_from_project_reference(source_file.path())
            || !self.can_include_bind_and_check_diagnostics(source_file)
    }

    pub(crate) fn can_include_bind_and_check_diagnostics(
        &self,
        source_file: &ast::SourceFile,
    ) -> bool {
        if source_file
            .check_js_directive()
            .is_some_and(|directive| !directive.enabled)
        {
            return false;
        }

        if source_file.script_kind() == core::ScriptKind::Ts
            || source_file.script_kind() == core::ScriptKind::Tsx
            || source_file.script_kind() == core::ScriptKind::External
        {
            return true;
        }

        let is_js = source_file.script_kind() == core::ScriptKind::Js
            || source_file.script_kind() == core::ScriptKind::Jsx;
        let is_check_js = is_js && ast::is_check_jsenabled_for_file(source_file, self.options());
        let is_plain_js = ast::is_plain_jsfile(Some(source_file), self.options().check_js);

        // By default, only type-check .ts, .tsx, Deferred, plain JS, checked JS and External
        // - plain JS: .js files with no // ts-check and checkJs: undefined
        // - check JS: .js files with either // ts-check or checkJs: true
        // - external: files that are added by plugins
        is_plain_js || is_check_js || source_file.script_kind() == core::ScriptKind::Deferred
    }

    pub fn get_source_files_to_emit(
        &self,
        target_source_file: Option<&ast::SourceFile>,
        force_dts_emit: bool,
    ) -> Vec<ast::SourceFile> {
        if target_source_file.is_none() && !force_dts_emit {
            return self
                .source_files_to_emit
                .get_or_init(|| share_source_file_refs(get_source_files_to_emit(self, None, false)))
                .iter()
                .map(ast::SourceFile::share_readonly)
                .collect();
        }
        share_source_file_refs(get_source_files_to_emit(
            self,
            target_source_file,
            force_dts_emit,
        ))
    }

    fn get_source_files_to_emit_refs<'a>(
        &'a self,
        target_source_file: Option<&'a ast::SourceFile>,
        force_dts_emit: bool,
    ) -> Vec<&'a ast::SourceFile> {
        get_source_files_to_emit(self, target_source_file, force_dts_emit)
    }

    fn verify_compiler_options(&mut self) {
        let options = self.options().clone();
        let raw_options = self.opts.config.options.clone();
        let raw_option_value = |name: &str| {
            raw_options
                .get(name)
                .or_else(|| {
                    raw_options
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case(name))
                        .map(|(_, value)| value)
                })
                .map(String::as_str)
        };
        let raw_option_value_is = |name: &str, value: &str| {
            raw_option_value(name).is_some_and(|actual| actual.eq_ignore_ascii_case(value))
        };
        let config_source_file = self
            .opts
            .config
            .config_file
            .as_ref()
            .map(|config_file| config_file.source_file.share_readonly());
        let config_file_path_value = config_source_file
            .as_ref()
            .map_or(String::new(), |file| file.file_name().to_string());

        let source_file = || {
            config_source_file
                .as_ref()
                .map(ast::SourceFile::share_readonly)
        };

        let config_file_path = || config_file_path_value.clone();

        let get_compiler_options_property_syntax = || {
            tsoptions::for_each_ts_config_prop_array(
                source_file().as_ref(),
                "compilerOptions",
                Some,
            )
        };

        let get_compiler_options_object_literal_syntax = || {
            let source_file = source_file();
            let source_file = source_file.as_ref()?;
            let store = source_file.store();
            let compiler_options_property = get_compiler_options_property_syntax();
            if let Some(compiler_options_property) = compiler_options_property {
                if let Some(initializer) = store.initializer(compiler_options_property)
                    && ast::is_object_literal_expression(store, initializer)
                {
                    return Some(initializer);
                }
            }
            None
        };

        let add_program_diagnostic = |diag: &ast::Diagnostic| {
            self.program_diagnostics
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .push(diag.clone());
        };

        let create_option_diagnostic_in_object_literal_syntax =
            |object_literal: Option<ast::Node>,
             on_key: bool,
             key1: &str,
             key2: &str,
             message: &'static diagnostics::Message,
             args: Vec<Any>|
             -> Option<ast::Diagnostic> {
                let Some(object_literal) = object_literal else {
                    return None;
                };
                let source_file = source_file();
                let source_file = source_file.as_ref().unwrap();
                let store = source_file.store();
                let diag = tsoptions::for_each_property_assignment(
                    store,
                    Some(object_literal),
                    key1,
                    |property| {
                        let property_node = property;
                        let diagnostic_node = if on_key {
                            store.name(property_node).unwrap()
                        } else {
                            store.initializer(property_node).unwrap()
                        };
                        Some(tsoptions::create_diagnostic_for_node_in_source_file(
                            source_file,
                            diagnostic_node,
                            message,
                            &args.clone(),
                        ))
                    },
                    key2,
                );
                diag
            };

        let create_compiler_options_diagnostic =
            |message: &'static diagnostics::Message, args: Vec<Any>| -> ast::Diagnostic {
                let compiler_options_property = get_compiler_options_property_syntax();
                let diag = if let Some(compiler_options_property) = compiler_options_property {
                    let source_file = source_file();
                    let source_file = source_file.as_ref().unwrap();
                    let name = source_file.store().name(compiler_options_property).unwrap();
                    tsoptions::create_diagnostic_for_node_in_source_file(
                        source_file,
                        name,
                        message,
                        &args,
                    )
                } else {
                    ast::new_compiler_diagnostic(message, &args)
                };
                diag
            };

        let create_diagnostic_for_option_no_push = |on_key: bool,
                                                    option1: &str,
                                                    option2: &str,
                                                    message: &'static diagnostics::Message,
                                                    args: Vec<Any>|
         -> ast::Diagnostic {
            if let Some(diag) = create_option_diagnostic_in_object_literal_syntax(
                get_compiler_options_object_literal_syntax(),
                on_key,
                option1,
                option2,
                message,
                args.clone(),
            ) {
                return diag;
            }
            create_compiler_options_diagnostic(message, args)
        };

        let create_diagnostic_for_option = |on_key: bool,
                                            option1: &str,
                                            option2: &str,
                                            message: &'static diagnostics::Message,
                                            args: Vec<Any>|
         -> ast::Diagnostic {
            let diag =
                create_diagnostic_for_option_no_push(on_key, option1, option2, message, args);
            add_program_diagnostic(&diag);
            diag
        };

        macro_rules! create_diagnostic_for_option_name {
            ($message:expr, $option1:expr, $option2:expr, $args:expr $(,)?) => {{
                let mut args: Vec<Any> = $args;
                let mut new_args = Vec::with_capacity(args.len() + 2);
                new_args.push($option1.to_string().into());
                new_args.push($option2.to_string().into());
                new_args.append(&mut args);
                create_diagnostic_for_option(true, $option1, $option2, $message, new_args)
            }};
        }

        macro_rules! create_option_value_diagnostic {
            ($option1:expr, $message:expr, $args:expr $(,)?) => {{ create_diagnostic_for_option(false, $option1, "", $message, $args) }};
        }

        let report_invalid_ignore_deprecations = || {
            create_option_value_diagnostic!(
                "ignoreDeprecations",
                &diagnostics::Invalid_value_for_ignoreDeprecations,
                Vec::new(),
            )
        };

        let ignore_deprecations_version = match options.ignore_deprecations.as_str() {
            "" => None,
            "5.0" | "6.0" => Some(options.ignore_deprecations.as_str()),
            _ => {
                report_invalid_ignore_deprecations();
                None
            }
        };

        let type_script_version = if self.opts.type_script_version.is_empty() {
            core::version_major_minor()
        } else {
            self.opts.type_script_version.clone()
        };
        let ignore_deprecations_version = ignore_deprecations_version.unwrap_or("0");

        let check_deprecations = |deprecated_in: &str, removed_in: &str| -> Option<bool> {
            let must_be_removed = compare_version_text(removed_in, &type_script_version)
                != std::cmp::Ordering::Greater;
            let can_be_silenced = !must_be_removed
                && compare_version_text(ignore_deprecations_version, deprecated_in)
                    == std::cmp::Ordering::Less;
            (must_be_removed || can_be_silenced).then_some(must_be_removed)
        };

        macro_rules! create_deprecated_option_diagnostic {
            ($must_be_removed:expr, $deprecated_in:expr, $removed_in:expr, $name:expr, $value:expr, $use_instead:expr, $related:expr $(,)?) => {{
                let (message, args) = if $must_be_removed {
                    if $value.is_empty() {
                        (
                            &diagnostics::Option_0_has_been_removed_Please_remove_it_from_your_configuration,
                            vec![$name.to_string().into()],
                        )
                    } else {
                        (
                            &diagnostics::Option_0_1_has_been_removed_Please_remove_it_from_your_configuration,
                            vec![$name.to_string().into(), $value.to_string().into()],
                        )
                    }
                } else if $value.is_empty() {
                    (
                        &diagnostics::Option_0_is_deprecated_and_will_stop_functioning_in_TypeScript_1_Specify_compilerOption_ignoreDeprecations_Colon_2_to_silence_this_error,
                        vec![$name.to_string().into(), $removed_in.to_string().into(), $deprecated_in.to_string().into()],
                    )
                } else {
                    (
                        &diagnostics::Option_0_1_is_deprecated_and_will_stop_functioning_in_TypeScript_2_Specify_compilerOption_ignoreDeprecations_Colon_3_to_silence_this_error,
                        vec![$name.to_string().into(), $value.to_string().into(), $removed_in.to_string().into(), $deprecated_in.to_string().into()],
                    )
                };

                let mut diag = create_diagnostic_for_option_no_push($value.is_empty(), $name, "", message, args);
                if !$use_instead.is_empty() {
                    diag.add_message_chain(Some(ast::new_compiler_diagnostic(
                        &diagnostics::Use_0_instead,
                        &[$use_instead.to_string().into()],
                    )));
                }
                if let Some(related) = $related {
                    diag.add_message_chain(Some(ast::new_compiler_diagnostic(related, &[])));
                }
                add_program_diagnostic(&diag);
                diag
            }};
        }

        if let Some(must_be_removed) = check_deprecations("5.0", "5.5") {
            let deprecated_in = "5.0";
            let removed_in = "5.5";
            if options.target_is_es3 {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "target",
                    "ES3",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if options.no_implicit_use_strict.is_true() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "noImplicitUseStrict",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if options.keyof_strings_only.is_true() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "keyofStringsOnly",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if options.suppress_excess_property_errors.is_true() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "suppressExcessPropertyErrors",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if options.suppress_implicit_any_index_errors.is_true() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "suppressImplicitAnyIndexErrors",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if options.no_strict_generic_checks.is_true() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "noStrictGenericChecks",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if !options.charset.is_empty() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "charset",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if !options.out.is_empty() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "out",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
        }

        if let Some(must_be_removed) = check_deprecations("6.0", "7.0") {
            let deprecated_in = "6.0";
            let removed_in = "7.0";

            if !options.base_url.is_empty() {
                // BaseUrl will have been turned absolute by this point.
                let mut use_instead = String::new();
                if !config_file_path().is_empty() {
                    let mut relative = tspath::get_relative_path_from_file(
                        &config_file_path(),
                        &options.base_url,
                        &self.compare_paths_options,
                    );
                    if !(relative.starts_with("./") || relative.starts_with("../")) {
                        relative.insert_str(0, "./");
                    }
                    let suggestion = tspath::combine_paths(&relative, &["*"]);
                    let suggestion_json = json::marshal(&suggestion, &[])
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                        .unwrap_or_else(|| format!("\"{}\"", suggestion));
                    use_instead = format!("\"paths\": {{\"*\": [{}]}}", suggestion_json);
                }
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "baseUrl",
                    "",
                    &use_instead,
                    None::<&diagnostics::Message>,
                );
            }

            if !options.out_file.is_empty() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "outFile",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }

            if options.target == core::ScriptTarget::Es5 {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "target",
                    "ES5",
                    "",
                    None::<&diagnostics::Message>,
                );
            }

            if options.module == core::ModuleKind::Amd {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "module",
                    "AMD",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if options.module == core::ModuleKind::System {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "module",
                    "System",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
            if options.module == core::ModuleKind::Umd {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "module",
                    "UMD",
                    "",
                    None::<&diagnostics::Message>,
                );
            }

            if raw_option_value_is("moduleResolution", "classic") {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "moduleResolution",
                    "classic",
                    "",
                    None::<&diagnostics::Message>,
                );
            }

            if options.always_strict.is_false() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "alwaysStrict",
                    "false",
                    "",
                    None::<&diagnostics::Message>,
                );
            }

            if options.es_module_interop.is_false() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "esModuleInterop",
                    "false",
                    "",
                    None::<&diagnostics::Message>,
                );
            }

            if options.allow_synthetic_default_imports.is_false() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "allowSyntheticDefaultImports",
                    "false",
                    "",
                    None::<&diagnostics::Message>,
                );
            }

            if raw_option_value_is("moduleResolution", "node")
                || raw_option_value_is("moduleResolution", "node10")
            {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "moduleResolution",
                    raw_option_value("moduleResolution").unwrap_or("node10"),
                    "",
                    None::<&diagnostics::Message>,
                );
            }

            if !options.downlevel_iteration.is_unknown() {
                create_deprecated_option_diagnostic!(
                    must_be_removed,
                    deprecated_in,
                    removed_in,
                    "downlevelIteration",
                    "",
                    "",
                    None::<&diagnostics::Message>,
                );
            }
        }

        if options.strict_property_initialization.is_true()
            && !options.get_strict_option_value(options.strict_null_checks)
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1,
                "strictPropertyInitialization",
                "strictNullChecks",
                Vec::new(),
            );
        }
        if options.exact_optional_property_types.is_true()
            && !options.get_strict_option_value(options.strict_null_checks)
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1,
                "exactOptionalPropertyTypes",
                "strictNullChecks",
                Vec::new(),
            );
        }

        if options.isolated_declarations.is_true() {
            if options.get_allow_js() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_with_option_1,
                    "allowJs",
                    "isolatedDeclarations",
                    Vec::new(),
                );
            }
            if !options.get_emit_declarations() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1_or_option_2,
                    "isolatedDeclarations",
                    "declaration",
                    vec!["composite".into()],
                );
            }
        }

        if options.inline_source_map.is_true() {
            if options.source_map.is_true() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_with_option_1,
                    "sourceMap",
                    "inlineSourceMap",
                    Vec::new(),
                );
            }
            if !options.map_root.is_empty() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_with_option_1,
                    "mapRoot",
                    "inlineSourceMap",
                    Vec::new(),
                );
            }
        }

        if options.composite.is_true() {
            if options.declaration.is_false() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Composite_projects_may_not_disable_declaration_emit,
                    "declaration",
                    "",
                    Vec::new(),
                );
            }
            if options.incremental.is_false() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Composite_projects_may_not_disable_incremental_compilation,
                    "declaration",
                    "",
                    Vec::new(),
                );
            }
        }

        if options.ts_build_info_file.is_empty()
            && options.incremental.is_true()
            && options.config_file_path.is_empty()
        {
            let diag = create_compiler_options_diagnostic(
                &diagnostics::Option_incremental_is_only_valid_with_a_known_configuration_file_like_tsconfig_json_or_when_tsBuildInfoFile_is_explicitly_provided,
                Vec::new(),
            );
            add_program_diagnostic(&diag);
        }

        self.verify_project_references();

        if options.composite.is_true() {
            let mut root_paths = collections::Set::new();
            for file_name in self.opts.config.file_names() {
                root_paths.add(self.to_path(&file_name));
            }

            for file in &self.source_files {
                let source_file = file.as_source_file();
                if source_file_may_be_emitted(source_file, self, false)
                    && !root_paths.has(&source_file.path())
                {
                    self.processed_files
                        .include_processor
                        .add_processing_diagnostic(vec![ProcessingDiagnostic {
                            kind: ProcessingDiagnosticKind::ExplainingFileInclude,
                            data: ProcessingDiagnosticData::IncludeExplaining(IncludeExplainingDiagnostic {
                                file: Some(source_file.path()),
                                diagnostic_reason: None,
                                message: &diagnostics::File_0_is_not_listed_within_the_file_list_of_project_1_Projects_must_list_all_files_or_use_an_include_pattern,
                                args: vec![source_file.file_name(), config_file_path()],
                            }),
                        }]);
                }
            }
        }

        let create_diagnostic_for_option_paths = |on_key: bool,
                                                  key: &str,
                                                  message: &'static diagnostics::Message,
                                                  args: Vec<Any>|
         -> ast::Diagnostic {
            let source_file = source_file();
            let Some(source_file) = source_file.as_ref() else {
                return create_compiler_options_diagnostic(message, args);
            };
            let store = source_file.store();
            let diag = tsoptions::for_each_property_assignment(
                store,
                get_compiler_options_object_literal_syntax(),
                "paths",
                |path_prop| {
                    if let Some(initializer) = store.initializer(path_prop)
                        && ast::is_object_literal_expression(store, initializer)
                    {
                        return create_option_diagnostic_in_object_literal_syntax(
                            Some(initializer),
                            on_key,
                            key,
                            "",
                            message,
                            args.clone(),
                        );
                    }
                    None
                },
                "",
            );
            diag.unwrap_or_else(|| create_compiler_options_diagnostic(message, args))
        };

        let create_diagnostic_for_option_path_key_value =
            |key: &str,
             value_index: usize,
             message: &'static diagnostics::Message,
             args: Vec<Any>|
             -> ast::Diagnostic {
                let source_file = source_file();
                let Some(source_file) = source_file.as_ref() else {
                    return create_compiler_options_diagnostic(message, args);
                };
                let store = source_file.store();
                let diag = tsoptions::for_each_property_assignment(
                    store,
                    get_compiler_options_object_literal_syntax(),
                    "paths",
                    |path_prop| {
                        if let Some(paths_initializer) = store.initializer(path_prop)
                            && ast::is_object_literal_expression(store, paths_initializer)
                        {
                            return tsoptions::for_each_property_assignment(
                                store,
                                Some(paths_initializer),
                                key,
                                |key_props| {
                                    let initializer = store.initializer(key_props).unwrap();
                                    if ast::is_array_literal_expression(store, initializer) {
                                        let elements = store.elements(initializer).unwrap();
                                        if elements.len() > value_index {
                                            let element = elements.iter().nth(value_index).unwrap();
                                            let diag =
                                                tsoptions::create_diagnostic_for_node_in_source_file(
                                                    source_file,
                                                    element,
                                                    message,
                                                    &args.clone(),
                                                );
                                            self.program_diagnostics
                                                .lock()
                                                .unwrap_or_else(|err| err.into_inner())
                                                .push(diag.clone());
                                            return Some(diag);
                                        }
                                    }
                                    None
                                },
                                "",
                            );
                        }
                        None
                    },
                    "",
                );
                diag.unwrap_or_else(|| create_compiler_options_diagnostic(message, args))
            };

        if !options.paths_for_validation.is_empty() {
            for (key, value) in options.paths_for_validation.entries() {
                if !has_zero_or_one_asterisk_character(key) {
                    let diag = create_diagnostic_for_option_paths(
                        true,
                        key,
                        &diagnostics::Pattern_0_can_have_at_most_one_Asterisk_character,
                        vec![key.into()],
                    );
                    add_program_diagnostic(&diag);
                }
                if let Value::Array(value) = value {
                    let len = value.len();
                    if len == 0 {
                        let diag = create_diagnostic_for_option_paths(
                            false,
                            key,
                            &diagnostics::Substitutions_for_pattern_0_shouldn_t_be_an_empty_array,
                            vec![key.into()],
                        );
                        add_program_diagnostic(&diag);
                    }
                    for (i, subst) in value.iter().enumerate() {
                        if let Some(subst) = subst.as_str() {
                            if !has_zero_or_one_asterisk_character(subst) {
                                create_diagnostic_for_option_path_key_value(
                                    key,
                                    i,
                                    &diagnostics::Substitution_0_in_pattern_1_can_have_at_most_one_Asterisk_character,
                                    vec![subst.to_owned().into(), key.into()],
                                );
                            }
                            if !tspath::path_is_relative(subst) && !tspath::path_is_absolute(subst)
                            {
                                create_diagnostic_for_option_path_key_value(
                                    key,
                                    i,
                                    &diagnostics::Non_relative_paths_are_not_allowed_Did_you_forget_a_leading_Slash,
                                    Vec::new(),
                                );
                            }
                        } else {
                            create_diagnostic_for_option_path_key_value(
                                key,
                                i,
                                &diagnostics::Substitution_0_for_pattern_1_has_incorrect_type_expected_string_got_2,
                                vec![
                                    json_value_to_diagnostic_arg_string(subst).into(),
                                    key.into(),
                                    json_value_typeof(subst).into(),
                                ],
                            );
                        }
                    }
                } else {
                    let diag = create_diagnostic_for_option_paths(
                        false,
                        key,
                        &diagnostics::Substitutions_for_pattern_0_should_be_an_array,
                        vec![key.into()],
                    );
                    add_program_diagnostic(&diag);
                }
            }
        } else {
            for (key, value) in options.paths.entries() {
                if !has_zero_or_one_asterisk_character(key) {
                    create_diagnostic_for_option_paths(
                        true,
                        key,
                        &diagnostics::Pattern_0_can_have_at_most_one_Asterisk_character,
                        vec![key.into()],
                    );
                }
                if value.is_empty() {
                    create_diagnostic_for_option_paths(
                        false,
                        key,
                        &diagnostics::Substitutions_for_pattern_0_shouldn_t_be_an_empty_array,
                        vec![key.into()],
                    );
                }
                for (i, subst) in value.iter().enumerate() {
                    if !has_zero_or_one_asterisk_character(subst) {
                        create_diagnostic_for_option_path_key_value(
                            key,
                            i,
                            &diagnostics::Substitution_0_in_pattern_1_can_have_at_most_one_Asterisk_character,
                            vec![subst.into(), key.into()],
                        );
                    }
                    if !tspath::path_is_relative(subst) && !tspath::path_is_absolute(subst) {
                        create_diagnostic_for_option_path_key_value(
                            key,
                            i,
                            &diagnostics::Non_relative_paths_are_not_allowed_Did_you_forget_a_leading_Slash,
                            Vec::new(),
                        );
                    }
                }
            }
        }

        if options.source_map.is_false_or_unknown()
            && options.inline_source_map.is_false_or_unknown()
        {
            if options.inline_sources.is_true() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_can_only_be_used_when_either_option_inlineSourceMap_or_option_sourceMap_is_provided,
                    "inlineSources",
                    "",
                    Vec::new(),
                );
            }
            if !options.source_root.is_empty() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_can_only_be_used_when_either_option_inlineSourceMap_or_option_sourceMap_is_provided,
                    "sourceRoot",
                    "",
                    Vec::new(),
                );
            }
        }

        if !options.map_root.is_empty()
            && !(options.source_map.is_true() || options.declaration_map.is_true())
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1_or_option_2,
                "mapRoot",
                "sourceMap",
                vec!["declarationMap".into()],
            );
        }

        if !options.declaration_dir.is_empty() && !options.get_emit_declarations() {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1_or_option_2,
                "declarationDir",
                "declaration",
                vec!["composite".into()],
            );
        }

        if options.declaration_map.is_true() && !options.get_emit_declarations() {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1_or_option_2,
                "declarationMap",
                "declaration",
                vec!["composite".into()],
            );
        }

        if !options.lib.is_empty() && options.no_lib.is_true() {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_with_option_1,
                "lib",
                "noLib",
                Vec::new(),
            );
        }

        if (options.isolated_modules.is_true() || options.verbatim_module_syntax.is_true())
            && options.preserve_const_enums.is_false()
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_preserveConstEnums_cannot_be_disabled_when_0_is_enabled,
                if options.verbatim_module_syntax.is_true() {
                    "verbatimModuleSyntax"
                } else {
                    "isolatedModules"
                },
                "preserveConstEnums",
                Vec::new(),
            );
        }

        if !options.out_dir.is_empty()
            || !options.root_dir.is_empty()
            || !options.source_root.is_empty()
            || !options.map_root.is_empty()
            || (options.get_emit_declarations() && !options.declaration_dir.is_empty())
        {
            let dir = self.common_source_directory();
            if !options.out_dir.is_empty()
                && dir.is_empty()
                && self
                    .source_files
                    .iter()
                    .any(|f| tspath::get_root_length(&f.file_name()) > 1)
            {
                create_diagnostic_for_option_name!(
                    &diagnostics::Cannot_find_the_common_subdirectory_path_for_the_input_files,
                    "outDir",
                    "",
                    Vec::new(),
                );
            }
        }

        if !options.no_emit.is_true()
            && !options.composite.is_true()
            && options.root_dir.is_empty()
            && !options.config_file_path.is_empty()
            && (!options.out_dir.is_empty()
                || (options.get_emit_declarations() && !options.declaration_dir.is_empty())
                || !options.out_file.is_empty())
        {
            let dir = self.common_source_directory();
            let emitted_files: Vec<_> = self
                .source_files
                .iter()
                .filter(|file| {
                    let file = file.as_source_file();
                    !file.is_declaration_file() && source_file_may_be_emitted(file, self, false)
                })
                .map(|file| file.file_name().to_string())
                .collect();
            let dir59 = outputpaths::get_computed_common_source_directory(
                &emitted_files,
                &self.get_current_directory(),
                self.use_case_sensitive_file_names(),
            );
            if !dir59.is_empty()
                && tspath::get_canonical_file_name(&dir, self.use_case_sensitive_file_names())
                    != tspath::get_canonical_file_name(&dir59, self.use_case_sensitive_file_names())
            {
                let option1 = if !options.out_file.is_empty() {
                    "outFile"
                } else if !options.out_dir.is_empty() {
                    "outDir"
                } else {
                    "declarationDir"
                };
                let option2 = if options.out_file.is_empty() && !options.out_dir.is_empty() {
                    "declarationDir"
                } else {
                    ""
                };
                let mut diag = create_diagnostic_for_option_no_push(
                    true,
                    option1,
                    option2,
                    &diagnostics::The_common_source_directory_of_0_is_1_The_rootDir_setting_must_be_explicitly_set_to_this_or_another_path_to_adjust_your_output_s_file_layout,
                    vec![
                        tspath::get_base_file_name(&options.config_file_path).into(),
                        tspath::get_relative_path_from_file(
                            &options.config_file_path,
                            &dir59,
                            &self.compare_paths_options,
                        )
                        .into(),
                    ],
                );
                diag.add_message_chain(Some(ast::new_compiler_diagnostic(
                    &diagnostics::Visit_https_Colon_Slash_Slashaka_ms_Slashts6_for_migration_information,
                    &[],
                )));
                add_program_diagnostic(&diag);
            }
        }

        if options.check_js.is_true() && !options.get_allow_js() {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1,
                "checkJs",
                "allowJs",
                Vec::new(),
            );
        }

        if options.emit_declaration_only.is_true() {
            if !options.get_emit_declarations() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1_or_option_2,
                    "emitDeclarationOnly",
                    "declaration",
                    vec!["composite".into()],
                );
            }
        }

        if options.emit_decorator_metadata.is_true()
            && options.experimental_decorators.is_false_or_unknown()
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1,
                "emitDecoratorMetadata",
                "experimentalDecorators",
                Vec::new(),
            );
        }

        if !options.jsx_factory.is_empty() {
            if !options.react_namespace.is_empty() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_with_option_1,
                    "reactNamespace",
                    "jsxFactory",
                    Vec::new(),
                );
            }
            if options.jsx == core::JsxEmit::ReactJSX || options.jsx == core::JsxEmit::ReactJSXDev {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_when_option_jsx_is_1,
                    "jsxFactory",
                    options.jsx.string(),
                    Vec::new(),
                );
            }
            if parser::parse_isolated_entity_name(&options.jsx_factory).is_none() {
                create_option_value_diagnostic!(
                    "jsxFactory",
                    &diagnostics::Invalid_value_for_jsxFactory_0_is_not_a_valid_identifier_or_qualified_name,
                    vec![options.jsx_factory.clone().into()],
                );
            }
        } else if !options.react_namespace.is_empty()
            && !scanner::is_identifier_text(
                &options.react_namespace,
                core::LanguageVariant::Standard,
            )
        {
            create_option_value_diagnostic!(
                "reactNamespace",
                &diagnostics::Invalid_value_for_reactNamespace_0_is_not_a_valid_identifier,
                vec![options.react_namespace.clone().into()],
            );
        }

        if !options.jsx_fragment_factory.is_empty() {
            if options.jsx_factory.is_empty() {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_without_specifying_option_1,
                    "jsxFragmentFactory",
                    "jsxFactory",
                    Vec::new(),
                );
            }
            if options.jsx == core::JsxEmit::ReactJSX || options.jsx == core::JsxEmit::ReactJSXDev {
                create_diagnostic_for_option_name!(
                    &diagnostics::Option_0_cannot_be_specified_when_option_jsx_is_1,
                    "jsxFragmentFactory",
                    options.jsx.string(),
                    Vec::new(),
                );
            }
            if parser::parse_isolated_entity_name(&options.jsx_fragment_factory).is_none() {
                create_option_value_diagnostic!(
                    "jsxFragmentFactory",
                    &diagnostics::Invalid_value_for_jsxFragmentFactory_0_is_not_a_valid_identifier_or_qualified_name,
                    vec![options.jsx_fragment_factory.clone().into()],
                );
            }
        }

        if !options.react_namespace.is_empty()
            && (options.jsx == core::JsxEmit::ReactJSX || options.jsx == core::JsxEmit::ReactJSXDev)
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_when_option_jsx_is_1,
                "reactNamespace",
                options.jsx.string(),
                Vec::new(),
            );
        }

        if !options.jsx_import_source.is_empty() && options.jsx == core::JsxEmit::React {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_cannot_be_specified_when_option_jsx_is_1,
                "jsxImportSource",
                options.jsx.string(),
                Vec::new(),
            );
        }

        let module_kind = options.get_emit_module_kind();

        if options.allow_importing_ts_extensions.is_true()
            && !(options.no_emit.is_true()
                || options.emit_declaration_only.is_true()
                || options.rewrite_relative_import_extensions.is_true())
        {
            create_option_value_diagnostic!(
                "allowImportingTsExtensions",
                &diagnostics::Option_allowImportingTsExtensions_can_only_be_used_when_one_of_noEmit_emitDeclarationOnly_or_rewriteRelativeImportExtensions_is_set,
                Vec::new(),
            );
        }

        let module_resolution = options.get_module_resolution_kind();
        if options.resolve_package_json_exports.is_true()
            && !module_resolution_supports_package_json_exports_and_imports(module_resolution)
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_can_only_be_used_when_moduleResolution_is_set_to_node16_nodenext_or_bundler,
                "resolvePackageJsonExports",
                "",
                Vec::new(),
            );
        }
        if options.resolve_package_json_imports.is_true()
            && !module_resolution_supports_package_json_exports_and_imports(module_resolution)
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_can_only_be_used_when_moduleResolution_is_set_to_node16_nodenext_or_bundler,
                "resolvePackageJsonImports",
                "",
                Vec::new(),
            );
        }
        if !options.custom_conditions.is_empty()
            && !module_resolution_supports_package_json_exports_and_imports(module_resolution)
        {
            create_diagnostic_for_option_name!(
                &diagnostics::Option_0_can_only_be_used_when_moduleResolution_is_set_to_node16_nodenext_or_bundler,
                "customConditions",
                "",
                Vec::new(),
            );
        }

        if module_resolution == core::ModuleResolutionKind::Bundler
            && !emit_module_kind_is_non_node_esm(module_kind)
            && module_kind != core::ModuleKind::Preserve
            && module_kind != core::ModuleKind::CommonJS
        {
            create_option_value_diagnostic!(
                "moduleResolution",
                &diagnostics::Option_0_can_only_be_used_when_module_is_set_to_preserve_commonjs_or_es2015_or_later,
                vec!["bundler".into()],
            );
        }

        if core::ModuleKind::Node16 <= module_kind
            && module_kind <= core::ModuleKind::NodeNext
            && !(core::ModuleResolutionKind::Node16 <= module_resolution
                && module_resolution <= core::ModuleResolutionKind::NodeNext)
        {
            let module_kind_name = module_kind.to_string();
            let module_resolution_name = core::module_kind_to_module_resolution_kind()
                .get(&module_kind)
                .map(|kind| kind.string())
                .unwrap_or("Node16");
            create_option_value_diagnostic!(
                "moduleResolution",
                &diagnostics::Option_moduleResolution_must_be_set_to_0_or_left_unspecified_when_option_module_is_set_to_1,
                vec![module_resolution_name.into(), module_kind_name.into()],
            );
        } else if core::ModuleResolutionKind::Node16 <= module_resolution
            && module_resolution <= core::ModuleResolutionKind::NodeNext
            && !(core::ModuleKind::Node16 <= module_kind
                && module_kind <= core::ModuleKind::NodeNext)
        {
            let module_resolution_name = module_resolution.string();
            create_option_value_diagnostic!(
                "module",
                &diagnostics::Option_module_must_be_set_to_0_when_option_moduleResolution_is_set_to_1,
                vec![module_resolution_name.into(), module_resolution_name.into()],
            );
        }

        // The Go implementation notes this should use filesByName, which is not equivalent to filesByPath.

        // If the emit is enabled make sure that every output file is unique and not overwriting any of the input files
        if !options.no_emit.is_true() && !options.suppress_output_path_check.is_true() {
            let mut emit_files_seen = collections::Set::new();

            // Verify that all the emit files are unique and don't overwrite input files
            let mut verify_emit_file_path = |emit_file_name: &str| {
                if !emit_file_name.is_empty() {
                    let emit_file_path = self.to_path(emit_file_name);
                    // Report error if the output overwrites input file
                    if self
                        .processed_files
                        .files_by_path
                        .contains_key(&emit_file_path)
                    {
                        let mut diag = ast::new_compiler_diagnostic(
                            &diagnostics::Cannot_write_file_0_because_it_would_overwrite_input_file,
                            &[emit_file_name.to_string().into()],
                        );
                        if config_file_path().is_empty() {
                            // The program is from either an inferred project or an external project
                            diag.add_message_chain(Some(ast::new_compiler_diagnostic(
                                &diagnostics::Adding_a_tsconfig_json_file_will_help_organize_projects_that_contain_both_TypeScript_and_JavaScript_files_Learn_more_at_https_Colon_Slash_Slashaka_ms_Slashtsconfig,
                                &[],
                            )));
                        }
                        self.block_emitting_of_file(emit_file_name, diag);
                    }

                    let emit_file_key = if !self.host().fs().use_case_sensitive_file_names() {
                        tspath::to_file_name_lower_case(&emit_file_path.to_string())
                    } else {
                        emit_file_path.to_string()
                    };

                    // Report error if multiple files write into same file
                    if emit_files_seen.has(&emit_file_key) {
                        // Already seen the same emit file - report error
                        self.block_emitting_of_file(
                            emit_file_name,
                            ast::new_compiler_diagnostic(
                                &diagnostics::Cannot_write_file_0_because_it_would_be_overwritten_by_multiple_input_files,
                                &[emit_file_name.to_string().into()],
                            ),
                        );
                    } else {
                        emit_files_seen.add(emit_file_key);
                    }
                }
            };

            let emit_source_files: Vec<_> = self
                .get_source_files_to_emit_refs(None, false)
                .iter()
                .map(|source_file| outputpaths_source_file(source_file))
                .collect();
            outputpaths::for_each_emitted_file(
                self,
                &outputpaths_compiler_options(&options),
                |emit_file_names, _source_file| {
                    verify_emit_file_path(&emit_file_names.js_file_path());
                    verify_emit_file_path(&emit_file_names.source_map_file_path());
                    verify_emit_file_path(&emit_file_names.declaration_file_path());
                    verify_emit_file_path(&emit_file_names.declaration_map_path());
                    false
                },
                &emit_source_files,
                false,
            );
            verify_emit_file_path(&self.opts.config.get_build_info_file_name());
        }
    }

    fn block_emitting_of_file(&self, emit_file_name: &str, diag: ast::Diagnostic) {
        self.has_emit_blocking_diagnostics
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .add(self.to_path(emit_file_name));
        self.program_diagnostics
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(diag);
    }

    pub fn is_emit_blocked(&self, emit_file_name: &str) -> bool {
        self.has_emit_blocking_diagnostics
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .has(&self.to_path(emit_file_name))
    }

    fn verify_project_references(&self) {
        let build_info_file_name = if !self.options().suppress_output_path_check.is_true() {
            self.opts.config.get_build_info_file_name()
        } else {
            String::new()
        };
        let create_diagnostic_for_reference =
            |parent: &tsoptions::ParsedCommandLine,
             index: usize,
             message: &'static diagnostics::Message,
             args: Vec<Any>| {
                let mut diag = tsoptions::create_diagnostic_at_reference_syntax(
                    parent,
                    index,
                    message,
                    &args.clone(),
                );
                if diag.is_none() {
                    diag = Some(ast::new_compiler_diagnostic(message, &args));
                }
                self.program_diagnostics
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .push(diag.unwrap());
            };

        self.range_resolved_project_reference(|_path, config, parent, index| {
            let r#ref = &parent.project_references()[index];
            // !!! Deprecated in 5.0 and removed since 5.5
            // verifyRemovedProjectReference(ref, parent, index);
            if config.is_none() {
                create_diagnostic_for_reference(&parent, index, &diagnostics::File_0_not_found, vec![r#ref.path.clone().into()]);
                return true;
            }
            let config = config.unwrap();
            let ref_options = config.compiler_options();
            if !ref_options.composite.is_true() || ref_options.no_emit.is_true() {
                if !parent.file_names().is_empty() {
                    if !ref_options.composite.is_true() {
                        create_diagnostic_for_reference(
                            &parent,
                            index,
                            &diagnostics::Referenced_project_0_must_have_setting_composite_Colon_true,
                            vec![r#ref.path.clone().into()],
                        );
                    }
                    if ref_options.no_emit.is_true() {
                        create_diagnostic_for_reference(
                            &parent,
                            index,
                            &diagnostics::Referenced_project_0_may_not_disable_emit,
                            vec![r#ref.path.clone().into()],
                        );
                    }
                }
            }
            if !build_info_file_name.is_empty() && build_info_file_name == config.get_build_info_file_name() {
                create_diagnostic_for_reference(
                    &parent,
                    index,
                    &diagnostics::Cannot_write_file_0_because_it_will_overwrite_tsbuildinfo_file_generated_by_referenced_project_1,
                    vec![build_info_file_name.clone().into(), r#ref.path.clone().into()],
                );
                self.has_emit_blocking_diagnostics
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .add(self.to_path(&build_info_file_name));
            }
            true
        });
    }
}

fn has_zero_or_one_asterisk_character(str_: &str) -> bool {
    let mut seen_asterisk = false;
    for ch in str_.chars() {
        if ch == '*' {
            if !seen_asterisk {
                seen_asterisk = true;
            } else {
                // have already seen asterisk
                return false;
            }
        }
    }
    true
}

fn json_value_typeof(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Null | Value::Array(_) | Value::Object(_) => "object",
    }
}

fn json_value_to_diagnostic_arg_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn module_resolution_supports_package_json_exports_and_imports(
    module_resolution: core::ModuleResolutionKind,
) -> bool {
    module_resolution >= core::ModuleResolutionKind::Node16
        && module_resolution <= core::ModuleResolutionKind::NodeNext
        || module_resolution == core::ModuleResolutionKind::Bundler
}

fn emit_module_kind_is_non_node_esm(module_kind: core::ModuleKind) -> bool {
    module_kind >= core::ModuleKind::Es2015 && module_kind <= core::ModuleKind::EsNext
}

impl Program {
    pub fn get_global_diagnostics(&self, _ctx: Context) -> Vec<ast::Diagnostic> {
        if self.source_files.is_empty() {
            return Vec::new();
        }
        if let Some(pool) = &self.compiler_checker_pool {
            return pool.get_global_diagnostics(self);
        }
        // For external pools (project system), global diagnostics are collected
        // incrementally as checkers are used, not via a bulk query.
        Vec::new()
    }

    pub fn get_declaration_diagnostics(
        &self,
        ctx: Context,
        source_file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        self.collect_diagnostics(ctx, source_file, true /*concurrent*/, |ctx, file| {
            self.get_declaration_diagnostics_for_file(ctx, file)
        })
    }
}

pub fn filter_no_emit_semantic_diagnostics(
    diagnostics: Vec<ast::Diagnostic>,
    options: &core::CompilerOptions,
) -> Vec<ast::Diagnostic> {
    if !options.no_emit.is_true() {
        return diagnostics;
    }
    diagnostics
        .into_iter()
        .filter(|d| !d.skipped_on_no_emit())
        .collect()
}

impl Program {
    pub fn get_semantic_diagnostics_with_checker<'a>(
        &self,
        ctx: Context,
        c: &mut checker::Checker<'a, '_>,
        source_file: &'a ast::SourceFile,
    ) -> Vec<ast::Diagnostic> {
        core::concatenate(
            &filter_no_emit_semantic_diagnostics(
                self.get_bind_and_check_diagnostics_with_checker(ctx, c, source_file),
                self.options(),
            ),
            &self.get_include_processor_diagnostics(source_file),
        )
    }

    // getBindAndCheckDiagnosticsWithChecker gets semantic diagnostics for a single file using a
    // caller-provided checker, including bind diagnostics, checker diagnostics, and handling
    // of @ts-ignore/@ts-expect-error directives.
    fn get_bind_and_check_diagnostics_with_checker<'a>(
        &self,
        ctx: Context,
        file_checker: &mut checker::Checker<'a, '_>,
        source_file: &'a ast::SourceFile,
    ) -> Vec<ast::Diagnostic> {
        let compiler_options = self.options();
        if self.skip_type_checking(source_file, false) {
            return Vec::new();
        }

        let mut diags = self.bind_diagnostics_for_file(source_file).to_vec();
        diags.extend(file_checker.get_diagnostics(ctx, source_file));

        let is_plain_js = ast::is_plain_jsfile(Some(source_file), compiler_options.check_js);
        if is_plain_js {
            return diags
                .into_iter()
                .filter(|d| plain_jserrors().has(&d.code()))
                .collect();
        }

        let (mut filtered, directives_by_line) =
            self.get_diagnostics_with_preceding_directives(source_file, diags);
        for directive in directives_by_line.values() {
            // Above we changed all used directive kinds to @ts-ignore, so any @ts-expect-error directives that
            // remain are unused and thus errors.
            if directive.kind == ast::CommentDirectiveKind::ExpectError {
                filtered.push(ast::new_diagnostic(
                    Some(source_file),
                    directive.loc,
                    &diagnostics::Unused_ts_expect_error_directive,
                    &[],
                ));
            }
        }
        filtered
    }

    fn get_diagnostics_with_preceding_directives(
        &self,
        source_file: &ast::SourceFile,
        diags: Vec<ast::Diagnostic>,
    ) -> (Vec<ast::Diagnostic>, HashMap<i32, ast::CommentDirective>) {
        if source_file.comment_directives().is_empty() {
            return (diags, HashMap::new());
        }
        // Build map of directives by line number
        let mut directives_by_line = HashMap::new();
        for directive in source_file.comment_directives() {
            let line = scanner::get_ecmaline_of_position(source_file, directive.loc.pos() as usize);
            directives_by_line.insert(line as i32, directive.clone());
        }
        let line_starts = scanner::get_ecmaline_starts(source_file);
        let mut filtered = Vec::with_capacity(diags.len());
        for diagnostic in diags {
            let mut ignore_diagnostic = false;
            let mut line =
                scanner::compute_line_of_position(&line_starts, diagnostic.pos() as usize) as isize
                    - 1;
            while line >= 0 {
                // If line contains a @ts-ignore or @ts-expect-error directive, ignore this diagnostic and change
                // the directive kind to @ts-ignore to indicate it was used.
                if let Some(directive) = directives_by_line.get_mut(&(line as i32)) {
                    ignore_diagnostic = true;
                    directive.kind = ast::CommentDirectiveKind::Ignore;
                    break;
                }
                // Stop searching backwards when we encounter a line that isn't blank or a comment.
                if !is_comment_or_blank_line(
                    source_file.text(),
                    line_starts[line as usize] as usize,
                ) {
                    break;
                }
                line -= 1;
            }
            if !ignore_diagnostic {
                filtered.push(diagnostic);
            }
        }
        (filtered, directives_by_line)
    }

    fn get_declaration_diagnostics_for_file(
        &self,
        ctx: Context,
        source_file: &ast::SourceFile,
    ) -> Vec<ast::Diagnostic> {
        if source_file.is_declaration_file() {
            return Vec::new();
        }

        if let (Some(cached), true) = self.declaration_diagnostic_cache.load(&source_file.path()) {
            return cached;
        }

        let diagnostics = self.with_type_checker_for_file_using(
            CheckerAccess::context(&ctx),
            source_file,
            |active| {
                let mut host = new_emit_host_with_checker(self, active.checker());
                get_declaration_diagnostics(&mut host, source_file)
            },
        );
        self.declaration_diagnostic_cache
            .load_or_store(source_file.path(), Some(diagnostics))
            .0
            .unwrap_or_default()
    }

    pub fn get_declaration_diagnostics_with_checker<'a>(
        &'a self,
        _ctx: Context,
        file_checker: &mut checker::Checker<'a, '_>,
        source_file: &'a ast::SourceFile,
    ) -> Vec<ast::Diagnostic> {
        if source_file.is_declaration_file() {
            return Vec::new();
        }

        if let (Some(cached), true) = self.declaration_diagnostic_cache.load(&source_file.path()) {
            return cached;
        }

        let mut host = new_emit_host_with_checker(self, file_checker);
        let diagnostics = get_declaration_diagnostics(&mut host, source_file);
        self.declaration_diagnostic_cache
            .load_or_store(source_file.path(), Some(diagnostics))
            .0
            .unwrap_or_default()
    }

    pub fn get_suggestion_diagnostics_with_checker<'a>(
        &self,
        ctx: Context,
        file_checker: &mut checker::Checker<'a, '_>,
        source_file: &'a ast::SourceFile,
    ) -> Vec<ast::Diagnostic> {
        if self.skip_type_checking(source_file, false) {
            return Vec::new();
        }

        let mut diags = self
            .bind_suggestion_diagnostics_for_file(source_file)
            .to_vec();
        diags.extend(file_checker.get_suggestion_diagnostics(ctx, source_file));

        diags
    }
}

fn is_comment_or_blank_line(text: &str, mut pos: usize) -> bool {
    while pos < text.len() && (text.as_bytes()[pos] == b' ' || text.as_bytes()[pos] == b'\t') {
        pos += 1;
    }
    pos == text.len()
        || pos < text.len() && (text.as_bytes()[pos] == b'\r' || text.as_bytes()[pos] == b'\n')
        || pos + 1 < text.len() && text.as_bytes()[pos] == b'/' && text.as_bytes()[pos + 1] == b'/'
}

pub fn sort_and_deduplicate_diagnostics(
    mut diagnostics: Vec<ast::Diagnostic>,
) -> Vec<ast::Diagnostic> {
    diagnostics.sort_by(compare_diagnostics_ordering);
    compact_and_merge_related_infos(diagnostics)
}

// Remove duplicate diagnostics and, for sequences of diagnostics that differ only by related information,
// create a single diagnostic with sorted and deduplicated related information.
fn compact_and_merge_related_infos(mut diagnostics: Vec<ast::Diagnostic>) -> Vec<ast::Diagnostic> {
    if diagnostics.len() < 2 {
        return diagnostics;
    }
    let mut i = 0;
    let mut j = 0;
    while i < diagnostics.len() {
        let mut d = diagnostics[i].clone();
        let mut n = 1;
        while i + n < diagnostics.len()
            && ast::equal_diagnostics_no_related_info(&d, &diagnostics[i + n])
        {
            n += 1;
        }
        if n > 1 {
            let mut related_infos = Vec::new();
            for k in 0..n {
                related_infos.extend(diagnostics[i + k].related_information().iter().cloned());
            }
            if !related_infos.is_empty() {
                related_infos.sort_by(compare_diagnostics_ordering);
                related_infos.dedup_by(|a, b| ast::equal_diagnostics(a, b));
                d.set_related_info(related_infos);
            }
        }
        diagnostics[j] = d;
        i += n;
        j += 1;
    }
    diagnostics.truncate(j);
    diagnostics
}

impl Program {
    pub fn line_count(&self) -> usize {
        let mut count = 0;
        for file in &self.source_files {
            count += file.as_source_file().ecma_line_map().len();
        }
        count
    }

    pub fn identifier_count(&self) -> usize {
        let mut count = 0;
        for file in &self.source_files {
            count += file.as_source_file().identifier_count() as usize;
        }
        count
    }

    pub fn symbol_count(&self) -> usize {
        let count = self.binding_state().symbol_count();
        let val = AtomicU32::new(count as u32);
        self.for_each_checker_parallel(|_, c| {
            val.fetch_add(c.symbol_count(), Ordering::SeqCst);
        });
        val.load(Ordering::SeqCst) as usize
    }

    pub fn type_count(&self) -> usize {
        let val = AtomicU32::new(0);
        self.for_each_checker_parallel(|_, c| {
            val.fetch_add(c.type_count(), Ordering::SeqCst);
        });
        val.load(Ordering::SeqCst) as usize
    }

    pub fn instantiation_count(&self) -> usize {
        let val = AtomicU32::new(0);
        self.for_each_checker_parallel(|_, c| {
            val.fetch_add(c.total_instantiation_count(), Ordering::SeqCst);
        });
        val.load(Ordering::SeqCst) as usize
    }

    pub fn mapper_perf_counters(&self) -> checker::TypeMapperPerfCounters {
        let counters = Mutex::new(checker::TypeMapperPerfCounters::default());
        self.for_each_checker_parallel(|_, c| {
            counters
                .lock()
                .expect("mapper counter aggregate mutex poisoned")
                .accumulate(c.mapper_perf_counters());
        });
        counters
            .into_inner()
            .expect("mapper counter aggregate mutex poisoned")
    }

    fn program(&self) -> &Program {
        self
    }

    pub fn get_source_file_meta_data(&self, path: tspath::Path) -> ast::SourceFileMetaData {
        self.processed_files.source_file_meta_datas[&path].clone()
    }

    pub fn get_emit_module_format_of_file(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ModuleKind {
        ast::get_emit_module_format_of_file_worker(
            &source_file.file_name(),
            &self
                .processed_files
                .project_reference_file_mapper
                .get_compiler_options_for_file(source_file),
            self.get_source_file_meta_data(source_file.path()),
        )
    }

    pub fn get_emit_module_format_of_file_for_auto_imports(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ModuleKind {
        self.get_emit_module_format_of_file(source_file)
    }

    fn get_emit_syntax_for_usage_location(
        &self,
        source_file: &dyn ast::HasFileName,
        location: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        let source_file_node = self
            .get_source_file_by_path_ref(&source_file.path())
            .unwrap();
        get_emit_syntax_for_usage_location_worker(
            source_file_node.store(),
            &source_file.file_name(),
            &self.processed_files.source_file_meta_datas[&source_file.path()],
            location,
            &self
                .processed_files
                .project_reference_file_mapper
                .get_compiler_options_for_file(source_file),
        )
    }

    pub fn get_implied_node_format_for_emit(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        ast::get_implied_node_format_for_emit_worker(
            &source_file.file_name(),
            self.processed_files
                .project_reference_file_mapper
                .get_compiler_options_for_file(source_file)
                .get_emit_module_kind(),
            self.get_source_file_meta_data(source_file.path()),
        )
    }

    pub fn get_implied_node_format_for_emit_for_auto_imports(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        self.get_implied_node_format_for_emit(source_file)
    }

    pub fn get_mode_for_usage_location(
        &self,
        source_file: &dyn ast::HasFileName,
        location: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        let source_file_node = self
            .get_source_file_by_path_ref(&source_file.path())
            .unwrap();
        get_mode_for_usage_location(
            source_file_node.store(),
            &source_file.file_name(),
            &self.processed_files.source_file_meta_datas[&source_file.path()],
            location,
            &self
                .processed_files
                .project_reference_file_mapper
                .get_compiler_options_for_file(source_file),
        )
    }

    pub fn get_default_resolution_mode_for_file(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        get_default_resolution_mode_for_file(
            &source_file.file_name(),
            &self.processed_files.source_file_meta_datas[&source_file.path()],
            &self
                .processed_files
                .project_reference_file_mapper
                .get_compiler_options_for_file(source_file),
        )
    }

    pub fn is_source_file_default_library(&self, path: tspath::Path) -> bool {
        self.processed_files.lib_files.contains_key(&path)
    }
    pub fn is_source_file_default_library_for_auto_imports(&self, path: tspath::Path) -> bool {
        self.is_source_file_default_library(path)
    }

    fn is_global_typings_file(&self, file_name: &str) -> bool {
        if !tspath::is_declaration_file_name(file_name) {
            return false;
        }
        tspath::contains_path(
            &self.get_global_typings_cache_location(),
            file_name,
            &self.compare_paths_options,
        )
    }
    pub fn is_global_typings_file_for_auto_imports(&self, file_name: &str) -> bool {
        self.is_global_typings_file(file_name)
    }

    pub fn get_default_lib_file(&self, path: tspath::Path) -> Option<LibFile> {
        self.processed_files.lib_files.get(&path).cloned()
    }

    pub fn common_source_directory(&self) -> String {
        self.common_source_directory
            .get_or_init(|| {
                let files = || {
                    self.source_files
                        .iter()
                        .map(ProgramSourceFile::as_source_file)
                        .filter_map(|file| {
                            if source_file_may_be_emitted(file, self, false /*forceDtsEmit*/)
                                && !file.is_declaration_file()
                            {
                                Some(file.file_name().to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                };
                outputpaths::get_common_source_directory(
                    &outputpaths_compiler_options(self.options()),
                    files,
                    &self.get_current_directory(),
                    self.use_case_sensitive_file_names(),
                    Some(|source_files: Vec<String>, root_directory: &str| {
                        self.check_source_files_belong_to_path(&source_files, root_directory)
                    }),
                )
            })
            .clone()
    }

    fn check_source_files_belong_to_path(
        &self,
        source_files: &[String],
        root_directory: &str,
    ) -> bool {
        let mut all_files_belong_to_path = true;
        for file in source_files {
            let absolute_source_file_path = tspath::get_canonical_file_name(
                &tspath::get_normalized_absolute_path(file, &self.get_current_directory()),
                self.use_case_sensitive_file_names(),
            );
            if !tspath::contains_path(root_directory, file, &self.compare_paths_options) {
                self.processed_files
                    .include_processor
                    .add_processing_diagnostic(vec![ProcessingDiagnostic {
                        kind: ProcessingDiagnosticKind::ExplainingFileInclude,
                        data: ProcessingDiagnosticData::IncludeExplaining(IncludeExplainingDiagnostic {
                            file: Some(tspath::Path::from(absolute_source_file_path)),
                            diagnostic_reason: None,
                            message: &diagnostics::File_0_is_not_under_rootDir_1_rootDir_is_expected_to_contain_all_source_files,
                            args: vec![file.clone(), root_directory.to_string()],
                        }),
                    }]);
                all_files_belong_to_path = false;
            }
        }

        all_files_belong_to_path
    }
}

pub struct WriteFileData {
    pub source_map_url_pos: i32,
    pub build_info: Option<Box<dyn StdAny>>,
    pub diagnostics: Vec<ast::Diagnostic>,
    pub skipped_dts_write: bool,
}

impl Default for WriteFileData {
    fn default() -> Self {
        Self {
            source_map_url_pos: -1,
            build_info: None,
            diagnostics: Vec::new(),
            skipped_dts_write: false,
        }
    }
}

pub struct EmitOptions {
    pub target_source_file: Option<ast::SourceFile>, // Single file to emit. If `nil`, emits all files
    pub emit_only: EmitOnly,
    pub write_file: Option<crate::WriteFile>,
}

impl Clone for EmitOptions {
    fn clone(&self) -> Self {
        Self {
            target_source_file: self
                .target_source_file
                .as_ref()
                .map(ast::SourceFile::share_readonly),
            emit_only: self.emit_only,
            write_file: self.write_file.clone(),
        }
    }
}

impl Default for EmitOptions {
    fn default() -> Self {
        Self {
            target_source_file: None,
            emit_only: EMIT_ALL,
            write_file: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct EmitResult {
    pub emit_skipped: bool,
    pub diagnostics: Vec<ast::Diagnostic>, // Contains declaration emit diagnostics
    pub emitted_files: Vec<String>,        // Array of files the compiler wrote to disk
    pub source_maps: Vec<SourceMapEmitResult>, // Array of sourceMapData if compiler emitted sourcemaps
}

#[derive(Clone)]
pub struct SourceMapEmitResult {
    pub input_source_file_names: Vec<String>, // Input source file (which one can use on program to get the file), 1:1 mapping with the sourceMap.sources list
    pub source_map: sourcemap::RawSourceMap,
    pub generated_file: String,
}

impl Program {
    fn emit(&mut self, ctx: Context, options: EmitOptions) -> EmitResult {
        let _trace = self.opts.tracing.as_mut().map(|tracing| {
            tracing.push(
                tracing::PHASE_EMIT,
                "emit",
                HashMap::<String, tracing::Any>::new(),
                true,
            )
        });

        if options.emit_only != EMIT_ONLY_FORCED_DTS {
            let result =
                handle_no_emit_on_error(ctx.clone(), self, options.target_source_file.as_ref());
            if let Some(result) = result {
                return result;
            }
            if ctx.err().is_some() {
                return EmitResult {
                    emit_skipped: true,
                    diagnostics: Vec::new(),
                    emitted_files: Vec::new(),
                    source_maps: Vec::new(),
                };
            }
        }

        let new_line = self.options().new_line.get_new_line_character();
        let source_files = self.get_source_files_to_emit_refs(
            options.target_source_file.as_ref(),
            options.emit_only == EMIT_ONLY_FORCED_DTS,
        );

        let mut results = Vec::with_capacity(source_files.len());
        for source_file in source_files {
            let result = self.with_type_checker_for_file_using(
                CheckerAccess::context(&ctx),
                source_file,
                |active| {
                    if options.emit_only != EMIT_ONLY_FORCED_DTS {
                        active.checker().get_diagnostics(ctx.clone(), source_file);
                    }
                    let mut host = new_emit_host_with_checker(self, active.checker());
                    let mut tracing = self.opts.tracing.clone();
                    emit_source_file(
                        &mut host,
                        source_file,
                        options.emit_only,
                        options.write_file.clone(),
                        tracing.as_mut(),
                        new_line,
                    )
                },
            );
            results.push(result);
        }

        // collect results from emit, preserving input order
        combine_emit_results(results)
    }
}

pub fn combine_emit_results(results: Vec<EmitResult>) -> EmitResult {
    let mut result = EmitResult {
        emit_skipped: false,
        diagnostics: Vec::new(),
        emitted_files: Vec::new(),
        source_maps: Vec::new(),
    };
    for emit_result in results {
        if emit_result.emit_skipped {
            result.emit_skipped = true;
        }
        result.diagnostics.extend(emit_result.diagnostics);
        result.emitted_files.extend(emit_result.emitted_files);
        if !emit_result.source_maps.is_empty() {
            result.source_maps.extend(emit_result.source_maps);
        }
    }
    result
}

pub trait ProgramLike {
    fn options(&self) -> &core::CompilerOptions;
    fn get_source_file(&self, path: &str) -> Option<ast::SourceFile>;
    fn get_source_files(&self) -> Vec<ast::SourceFile>;
    fn get_parsed_source_files_refs(&self) -> Vec<&ast::SourceFile>;
    fn get_config_file_parsing_diagnostics(&self) -> Vec<ast::Diagnostic>;
    fn get_syntactic_diagnostics(
        &self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic>;
    fn get_bind_diagnostics(
        &self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic>;
    fn get_program_diagnostics(&self) -> Vec<ast::Diagnostic>;
    fn get_global_diagnostics(&self, ctx: Context) -> Vec<ast::Diagnostic>;
    fn get_semantic_diagnostics(
        &mut self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic>;
    fn get_declaration_diagnostics(
        &mut self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic>;
    fn get_suggestion_diagnostics(
        &self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic>;
    fn emit(&mut self, ctx: Context, options: EmitOptions) -> Option<EmitResult>;
    fn common_source_directory(&self) -> String;
    fn is_source_file_default_library(&self, path: tspath::Path) -> bool;
    fn program(&self) -> &Program;
}

impl ProgramLike for Program {
    fn options(&self) -> &core::CompilerOptions {
        Program::options(self)
    }

    fn get_source_file(&self, path: &str) -> Option<ast::SourceFile> {
        Program::get_source_file(self, path)
    }

    fn get_source_files(&self) -> Vec<ast::SourceFile> {
        Program::get_source_files(self)
    }

    fn get_parsed_source_files_refs(&self) -> Vec<&ast::SourceFile> {
        Program::get_parsed_source_files_refs(self)
    }

    fn get_config_file_parsing_diagnostics(&self) -> Vec<ast::Diagnostic> {
        Program::get_config_file_parsing_diagnostics(self)
    }

    fn get_syntactic_diagnostics(
        &self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_syntactic_diagnostics(self, ctx, file)
    }

    fn get_bind_diagnostics(
        &self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_bind_diagnostics(self, ctx, file)
    }

    fn get_program_diagnostics(&self) -> Vec<ast::Diagnostic> {
        Program::get_program_diagnostics(self)
    }

    fn get_global_diagnostics(&self, ctx: Context) -> Vec<ast::Diagnostic> {
        Program::get_global_diagnostics(self, ctx)
    }

    fn get_semantic_diagnostics(
        &mut self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_semantic_diagnostics(self, ctx, file)
    }

    fn get_declaration_diagnostics(
        &mut self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_declaration_diagnostics(self, ctx, file)
    }

    fn get_suggestion_diagnostics(
        &self,
        ctx: Context,
        file: Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic> {
        Program::get_suggestion_diagnostics(self, ctx, file)
    }

    fn emit(&mut self, ctx: Context, options: EmitOptions) -> Option<EmitResult> {
        Some(Program::emit(self, ctx, options))
    }

    fn common_source_directory(&self) -> String {
        Program::common_source_directory(self)
    }

    fn is_source_file_default_library(&self, path: tspath::Path) -> bool {
        Program::is_source_file_default_library(self, path)
    }

    fn program(&self) -> &Program {
        self
    }
}

impl modulespecifiers::ModuleSpecifierGenerationHost for Program {
    fn symlink_cache(&self) -> Option<symlinks::KnownSymlinks> {
        Some(self.get_symlink_cache())
    }

    fn common_source_directory(&self) -> String {
        Program::common_source_directory(self)
    }

    fn global_typings_cache_location(&self) -> String {
        self.get_global_typings_cache_location()
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        Program::use_case_sensitive_file_names(self)
    }

    fn current_directory(&self) -> String {
        self.get_current_directory()
    }

    fn project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        self.get_project_reference_from_source(path)
    }

    fn redirect_targets(&self, path: tspath::Path) -> Vec<String> {
        self.get_redirect_targets(path)
    }

    fn source_of_project_reference_if_output_included(
        &self,
        file: &dyn ast::HasFileName,
    ) -> String {
        self.get_source_of_project_reference_if_output_included(file)
    }

    fn file_exists(&self, path: &str) -> bool {
        Program::file_exists(self, path)
    }

    fn nearest_ancestor_directory_with_package_json(&self, dirname: &str) -> String {
        self.get_nearest_ancestor_directory_with_package_json(dirname)
    }

    fn package_json_info(&self, pkg_json_path: &str) -> Option<packagejson::InfoCacheEntry> {
        self.get_package_json_info(pkg_json_path)
    }

    fn default_resolution_mode_for_file(
        &self,
        file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        self.get_default_resolution_mode_for_file(file)
    }

    fn resolved_module_from_module_specifier(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> Option<module::ResolvedModule> {
        self.get_resolved_module_from_module_specifier(file, module_specifier)
    }

    fn mode_for_usage_location(
        &self,
        file: &dyn ast::HasFileName,
        module_specifier: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        self.get_mode_for_usage_location(file, module_specifier)
    }
}

impl checker::Host for Program {}

impl checker::Program for Program {
    fn options(&self) -> &core::CompilerOptions {
        Program::options(self)
    }

    fn source_files(&self) -> Vec<&ast::SourceFile> {
        self.bound_source_files_refs()
    }

    fn bind_source_files(&self) {
        Program::bind_source_files(self)
    }

    fn binding_state(&self, source_file: &ast::SourceFile) -> Arc<binder::ProgramBindingState> {
        Arc::clone(
            Program::binding_state(self)
                .get_by_source_file(source_file)
                .unwrap_or_else(|| {
                    panic!(
                        "source file `{}` is not part of this program binding state",
                        source_file.file_name()
                    )
                }),
        )
    }

    fn file_exists(&self, file_name: &str) -> bool {
        Program::file_exists(self, file_name)
    }

    fn get_source_file(&self, file_name: &str) -> Option<ast::SourceFile> {
        Program::get_source_file(self, file_name)
    }

    fn get_source_file_for_resolved_module(&self, file_name: &str) -> Option<ast::SourceFile> {
        Program::get_source_file_for_resolved_module(self, file_name)
    }

    fn get_emit_module_format_of_file(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ModuleKind {
        Program::get_emit_module_format_of_file(self, source_file)
    }

    fn get_emit_syntax_for_usage_location(
        &self,
        source_file: &dyn ast::HasFileName,
        usage_location: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        Program::get_emit_syntax_for_usage_location(self, source_file, usage_location)
    }

    fn get_mode_for_usage_location(
        &self,
        source_file: &dyn ast::HasFileName,
        usage_location: &ast::StringLiteralLike,
    ) -> core::ResolutionMode {
        Program::get_mode_for_usage_location(self, source_file, usage_location)
    }

    fn get_default_resolution_mode_for_file(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ResolutionMode {
        Program::get_default_resolution_mode_for_file(self, source_file)
    }

    fn get_implied_node_format_for_emit(
        &self,
        source_file: &dyn ast::HasFileName,
    ) -> core::ModuleKind {
        Program::get_implied_node_format_for_emit(self, source_file)
    }

    fn get_resolved_module(
        &self,
        current_source_file: &dyn ast::HasFileName,
        module_reference: &str,
        mode: core::ResolutionMode,
    ) -> Option<module::ResolvedModule> {
        self.get_resolved_module(current_source_file, module_reference, mode)
    }

    fn get_resolved_modules(
        &self,
    ) -> HashMap<tspath::Path, module::ModeAwareCache<module::ResolvedModule>> {
        Program::get_resolved_modules(self)
    }

    fn get_packages_map(&self) -> HashMap<String, bool> {
        Program::get_packages_map(self)
    }

    fn get_source_file_meta_data(&self, path: tspath::Path) -> ast::SourceFileMetaData {
        Program::get_source_file_meta_data(self, path)
    }

    fn get_jsx_runtime_import_specifier(&self, path: tspath::Path) -> (String, Option<ast::Node>) {
        self.get_jsx_runtime_import_specifier(path)
    }

    fn get_import_helpers_import_specifier(&self, path: tspath::Path) -> Option<ast::Node> {
        self.get_import_helpers_import_specifier(path)
    }

    fn source_file_may_be_emitted(
        &self,
        source_file: &ast::SourceFile,
        force_dts_emit: bool,
    ) -> bool {
        Program::source_file_may_be_emitted(self, source_file, force_dts_emit)
    }

    fn is_source_file_default_library(&self, path: tspath::Path) -> bool {
        Program::is_source_file_default_library(self, path)
    }

    fn get_project_reference_from_output_dts(
        &self,
        path: tspath::Path,
    ) -> Option<&tsoptions::SourceOutputAndProjectReference> {
        self.get_project_reference_from_output_dts_ref(path)
    }

    fn get_redirect_for_resolution(
        &self,
        file: &dyn ast::HasFileName,
    ) -> Option<&tsoptions::ParsedCommandLine> {
        self.get_redirect_for_resolution_ref(file)
    }

    fn common_source_directory(&self) -> String {
        Program::common_source_directory(self)
    }
}

pub fn handle_no_emit_on_error(
    ctx: Context,
    program: &mut dyn ProgramLike,
    file: Option<&ast::SourceFile>,
) -> Option<EmitResult> {
    if !program.options().no_emit_on_error.is_true() {
        return None; // No emit on error is not set, so we can proceed with emitting
    }

    let diagnostics = get_diagnostics_of_any_program(
        ctx,
        program,
        file,
        true,
        |program, ctx, file| program.get_bind_diagnostics(ctx, file),
        |program, ctx, file| program.get_semantic_diagnostics(ctx, file),
    );
    if diagnostics.is_empty() {
        return None; // No diagnostics, so we can proceed with emitting
    }
    Some(EmitResult {
        diagnostics,
        emit_skipped: true,
        emitted_files: Vec::new(),
        source_maps: Vec::new(),
    })
}

pub fn get_diagnostics_of_any_program(
    ctx: Context,
    program: &mut dyn ProgramLike,
    file: Option<&ast::SourceFile>,
    skip_no_emit_check_for_dts_diagnostics: bool,
    get_bind_diagnostics: impl Fn(
        &mut dyn ProgramLike,
        Context,
        Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic>,
    get_semantic_diagnostics: impl Fn(
        &mut dyn ProgramLike,
        Context,
        Option<&ast::SourceFile>,
    ) -> Vec<ast::Diagnostic>,
) -> Vec<ast::Diagnostic> {
    let mut all_diagnostics = program.get_config_file_parsing_diagnostics();
    let config_file_parsing_diagnostics_length = all_diagnostics.len();

    all_diagnostics.extend(program.get_syntactic_diagnostics(ctx.clone(), file));
    all_diagnostics.extend(program.get_program_diagnostics());

    if all_diagnostics.len() == config_file_parsing_diagnostics_length {
        // Do binding early so we can track the time.
        get_bind_diagnostics(program, ctx.clone(), file);

        if program.options().list_files_only.is_false_or_unknown() {
            all_diagnostics.extend(program.get_global_diagnostics(ctx.clone()));

            if all_diagnostics.len() == config_file_parsing_diagnostics_length {
                all_diagnostics.extend(get_semantic_diagnostics(program, ctx.clone(), file));
                // Ask for the global diagnostics again (they were empty above); we may have found new during checking, e.g. missing globals.
                all_diagnostics.extend(program.get_global_diagnostics(ctx.clone()));
            }

            if (skip_no_emit_check_for_dts_diagnostics || program.options().no_emit.is_true())
                && program.options().get_emit_declarations()
                && all_diagnostics.len() == config_file_parsing_diagnostics_length
            {
                all_diagnostics.extend(program.get_declaration_diagnostics(ctx, file));
            }
        }
    }
    sort_and_deduplicate_diagnostics(all_diagnostics)
}

impl Program {
    fn to_path(&self, filename: &str) -> tspath::Path {
        tspath::to_path(
            filename,
            &self.get_current_directory(),
            self.use_case_sensitive_file_names(),
        )
    }

    pub fn get_source_file(&self, filename: &str) -> Option<ast::SourceFile> {
        let path = self.to_path(filename);
        self.get_source_file_by_path(path)
    }

    pub fn get_source_file_ref(&self, filename: &str) -> Option<&ast::SourceFile> {
        let path = self.to_path(filename);
        self.get_source_file_by_path_ref(&path)
    }

    pub fn get_source_file_for_resolved_module(&self, file_name: &str) -> Option<ast::SourceFile> {
        let file = self.get_source_file(file_name);
        if file.is_none() {
            let filename = self.get_parse_file_redirect(file_name);
            if !filename.is_empty() {
                return self.get_source_file(&filename);
            }
        }
        file
    }

    fn get_source_file_for_resolved_module_ref(&self, file_name: &str) -> Option<&ast::SourceFile> {
        let file = self.get_source_file_ref(file_name);
        if file.is_none() {
            let filename = self.get_parse_file_redirect(file_name);
            if !filename.is_empty() {
                return self.get_source_file_ref(&filename);
            }
        }
        file
    }

    pub fn files_by_path(&self) -> HashMap<tspath::Path, ast::SourceFile> {
        self.processed_files
            .files_by_path
            .iter()
            .map(|(path, index)| (path.clone(), self.source_files[*index].share_source_file()))
            .collect()
    }

    pub fn get_source_file_by_path(&self, path: tspath::Path) -> Option<ast::SourceFile> {
        self.processed_files
            .files_by_path
            .get(&path)
            .map(|index| self.source_files[*index].share_source_file())
    }

    pub fn get_source_file_by_path_ref(&self, path: &tspath::Path) -> Option<&ast::SourceFile> {
        self.processed_files
            .files_by_path
            .get(path)
            .map(|index| self.source_files[*index].as_source_file())
    }

    pub fn has_same_file_names(&self, other: &Program) -> bool {
        self.processed_files.files_by_path.len() == other.processed_files.files_by_path.len()
            && self.processed_files.redirect_files_by_path.len()
                == other.processed_files.redirect_files_by_path.len()
            && self.processed_files.files_by_path.iter().all(|(path, a)| {
                other
                    .processed_files
                    .files_by_path
                    .get(path)
                    .is_some_and(|b| {
                        // checks for casing differences on case-insensitive file systems
                        self.source_files[*a].file_name() == other.source_files[*b].file_name()
                    })
            })
            && self
                .processed_files
                .redirect_files_by_path
                .iter()
                .all(|(path, a)| {
                    other
                        .processed_files
                        .redirect_files_by_path
                        .get(path)
                        .is_some_and(|b| a.file_name() == b.file_name())
                })
    }

    pub fn get_source_files(&self) -> Vec<ast::SourceFile> {
        self.source_files
            .iter()
            .map(ProgramSourceFile::share_source_file)
            .collect()
    }

    pub fn get_parsed_source_files_refs(&self) -> Vec<&ast::SourceFile> {
        self.source_files
            .iter()
            .map(ProgramSourceFile::as_source_file)
            .collect()
    }

    // Testing only
    pub fn get_include_reasons(&self) -> HashMap<tspath::Path, Vec<FileIncludeReason>> {
        self.processed_files
            .include_processor
            .file_include_reasons
            .clone()
    }

    // Testing only
    pub fn is_missing_path(&self, path: tspath::Path) -> bool {
        self.processed_files
            .missing_files
            .iter()
            .any(|missing_path| self.to_path(missing_path) == path)
    }

    pub fn explain_files(&self, w: &mut dyn Write, locale: locale::Locale) {
        let to_relative_file_name = |file_name: &str| {
            tspath::get_relative_path_from_directory(
                &self.get_current_directory(),
                file_name,
                &self.compare_paths_options,
            )
        };
        let mut files_explained = 0;
        let mut redirect_files: Vec<_> = self
            .processed_files
            .redirect_files_by_path
            .values()
            .cloned()
            .collect();
        redirect_files.sort_by(|a, b| a.index.cmp(&b.index));

        let files = self.get_source_files();
        let mut source_file_index = 0;
        let mut explain_one = |file: &dyn ast::HasFileName| {
            writeln!(w, "{}", to_relative_file_name(&file.file_name())).ok();
            if let Some(reasons) = self
                .processed_files
                .include_processor
                .file_include_reasons
                .get(&file.path())
            {
                for reason in reasons {
                    writeln!(
                        w,
                        "   {}",
                        reason.to_diagnostic(self, true).localize(locale.clone())
                    )
                    .ok();
                }
            }
            for diag in self
                .processed_files
                .include_processor
                .explain_redirect_and_implied_format(self, file.path(), |file_name| {
                    to_relative_file_name(file_name)
                })
            {
                writeln!(w, "   {}", diag.localize(locale.clone())).ok();
            }
        };

        for redirect_file in &redirect_files {
            while files_explained < redirect_file.index {
                explain_one(&files[source_file_index]);
                source_file_index += 1;
                files_explained += 1;
            }
            explain_one(redirect_file);
            files_explained += 1;
        }

        // Explain any remaining sourceFiles
        while files_explained < files.len() + redirect_files.len() {
            explain_one(&files[source_file_index]);
            source_file_index += 1;
            files_explained += 1;
        }
    }

    pub fn get_lib_file_from_reference(
        &self,
        r#ref: &ast::FileReference,
    ) -> Option<ast::SourceFile> {
        self.get_lib_file_from_reference_ref(r#ref)
            .map(ast::SourceFile::share_readonly)
    }

    pub fn get_lib_file_from_reference_ref(
        &self,
        r#ref: &ast::FileReference,
    ) -> Option<&ast::SourceFile> {
        let path = tsoptions::get_lib_file_name(&r#ref.file_name)?;
        if let Some(index) = self
            .processed_files
            .files_by_path
            .get(&tspath::Path::from(path))
        {
            return Some(self.source_files[*index].as_source_file());
        }
        None
    }

    pub fn get_resolved_type_reference_directive_from_type_reference_directive(
        &self,
        type_ref: &ast::FileReference,
        source_file: &ast::SourceFile,
    ) -> Option<module::ResolvedTypeReferenceDirective> {
        self.processed_files
            .type_resolutions_in_file
            .get(&source_file.path())
            .and_then(|resolutions| {
                resolutions
                    .get(&module::ModeAwareCacheKey {
                        name: type_ref.file_name.clone(),
                        mode: self
                            .get_mode_for_type_reference_directive_in_file(type_ref, source_file),
                    })
                    .cloned()
            })
    }

    pub fn get_resolved_type_reference_directives(
        &self,
    ) -> HashMap<tspath::Path, module::ModeAwareCache<module::ResolvedTypeReferenceDirective>> {
        self.processed_files.type_resolutions_in_file.clone()
    }

    fn get_mode_for_type_reference_directive_in_file(
        &self,
        r#ref: &ast::FileReference,
        source_file: &ast::SourceFile,
    ) -> core::ResolutionMode {
        if r#ref.resolution_mode != core::ResolutionMode::None {
            return r#ref.resolution_mode;
        }
        self.get_default_resolution_mode_for_file(source_file)
    }

    fn is_source_file_from_external_library(&self, file: &ast::SourceFile) -> bool {
        self.processed_files
            .source_files_found_searching_node_modules
            .has(&file.path())
    }

    pub fn is_source_file_from_external_library_for_auto_imports(
        &self,
        file: &ast::SourceFile,
    ) -> bool {
        self.is_source_file_from_external_library(file)
    }

    fn get_jsxruntime_import_specifier(&self, path: tspath::Path) -> (String, Option<ast::Node>) {
        if let Some(result) = self
            .processed_files
            .jsx_runtime_import_specifiers
            .get(&path)
        {
            return (result.module_reference.clone(), Some(result.specifier));
        }
        (String::new(), None)
    }

    fn get_jsxruntime_import_specifier_ref(
        &self,
        path: tspath::Path,
    ) -> (String, Option<&ast::Node>) {
        if let Some(result) = self
            .processed_files
            .jsx_runtime_import_specifiers
            .get(&path)
        {
            return (result.module_reference.clone(), Some(&result.specifier));
        }
        (String::new(), None)
    }

    pub fn get_jsx_runtime_import_specifier(
        &self,
        path: tspath::Path,
    ) -> (String, Option<ast::Node>) {
        self.get_jsxruntime_import_specifier(path)
    }

    fn get_jsx_runtime_import_specifier_ref(
        &self,
        path: tspath::Path,
    ) -> (String, Option<&ast::Node>) {
        self.get_jsxruntime_import_specifier_ref(path)
    }

    pub fn get_import_helpers_import_specifier(&self, path: tspath::Path) -> Option<ast::Node> {
        self.processed_files
            .import_helpers_import_specifiers
            .get(&path)
            .copied()
    }

    fn get_import_helpers_import_specifier_ref(&self, path: tspath::Path) -> Option<&ast::Node> {
        self.processed_files
            .import_helpers_import_specifiers
            .get(&path)
            .map(|specifier| specifier)
    }

    pub fn source_file_may_be_emitted(
        &self,
        source_file: &ast::SourceFile,
        force_dts_emit: bool,
    ) -> bool {
        source_file_may_be_emitted(source_file, self, force_dts_emit)
    }

    fn resolved_package_names(&self) -> collections::Set<String> {
        self.collect_package_names().resolved
    }
    pub fn resolved_package_names_for_auto_imports(&self) -> collections::Set<String> {
        self.resolved_package_names()
    }

    fn unresolved_package_names(&self) -> collections::Set<String> {
        self.collect_package_names().unresolved
    }
    pub fn unresolved_package_names_for_auto_imports(&self) -> collections::Set<String> {
        self.unresolved_package_names()
    }

    fn deep_import_package_names(&self) -> collections::Set<String> {
        self.collect_package_names().deep_import_packages
    }
    pub fn deep_import_package_names_for_auto_imports(&self) -> collections::Set<String> {
        self.deep_import_package_names()
    }

    pub fn project_reference_output_mappings_for_auto_imports(
        &self,
    ) -> HashMap<tspath::Path, String> {
        self.processed_files
            .project_reference_file_mapper
            .output_dts_to_project_reference
            .iter()
            .map(|(output_dts_path, mapping)| (output_dts_path.clone(), mapping.source.clone()))
            .collect()
    }

    fn collect_package_names(&self) -> PackageNamesInfo {
        self.package_names.get_value(|| {
            let mut package_names = PackageNamesInfo {
                resolved: collections::Set::new(),
                unresolved: collections::Set::new(),
                deep_import_packages: collections::Set::new(),
            };
            for file in &self.source_files {
                let source_file = file.as_source_file();
                if self.is_source_file_default_library(source_file.path())
                    || self.is_source_file_from_external_library(source_file)
                    || source_file.file_name().contains("/node_modules/")
                {
                    // Checking for /node_modules/ is a little imprecise, but ATA treats locally installed typings
                    // as root files, which would not pass IsSourceFileFromExternalLibrary.
                    continue;
                }
                for imp in source_file.imports() {
                    let import_text = source_file.store().text(*imp);
                    if tspath::is_external_module_name_relative(&import_text) {
                        continue;
                    }
                    if let Some(resolved_modules) = self
                        .processed_files
                        .resolved_modules
                        .get(&source_file.path())
                    {
                        let key = module::ModeAwareCacheKey {
                            name: import_text.clone(),
                            mode: self.get_mode_for_usage_location(source_file, imp),
                        };
                        if let Some(resolved_module) = resolved_modules.get(&key) {
                            if resolved_module.is_resolved() {
                                if !resolved_module.is_external_library_import {
                                    continue;
                                }
                                // Priority order for getting package name:
                                // 1. PackageId.Name (requires both name and version in package.json)
                                let mut name = resolved_module.package_id.name.clone();
                                if name.is_empty() {
                                    // 2. GetPackageScopeForPath - get name from package.json in the package directory
                                    let package_scope = self
                                        .processed_files
                                        .resolver
                                        .lock()
                                        .unwrap_or_else(|err| err.into_inner())
                                        .get_package_scope_for_path(
                                            &resolved_module.resolved_file_name,
                                        );
                                    if let Some(package_directory) = package_scope {
                                        let package_json_path = tspath::combine_paths(
                                            &package_directory,
                                            &["package.json"],
                                        );
                                        if let Some(scope_name) = self
                                            .get_package_json_info(&package_json_path)
                                            .and_then(|info| info.contents)
                                            .and_then(|contents| {
                                                let (scope_name, ok) =
                                                    contents.fields.header_fields.name.get_value();
                                                ok.then_some(scope_name)
                                            })
                                        {
                                            name = scope_name;
                                        }
                                    }
                                }
                                if name.is_empty() {
                                    // 3. GetPackageNameFromDirectory - extract from node_modules path
                                    name = modulespecifiers::get_package_name_from_directory(
                                        &resolved_module.resolved_file_name,
                                    );
                                }
                                // 4. If all fail, don't add empty string
                                if !name.is_empty() {
                                    package_names.resolved.add(name.clone());
                                    // Detect deep imports: subpath imports in packages without exports.
                                    // These are imports like "lodash/fp" where the package has no exports
                                    // map, so auto-import can only find them via recursive directory search.
                                    let (_, rest) = module::parse_package_name(&import_text);
                                    if !rest.is_empty() {
                                        let scope = self
                                            .processed_files
                                            .resolver
                                            .lock()
                                            .unwrap_or_else(|err| err.into_inner())
                                            .get_package_scope_for_path(
                                                &resolved_module.resolved_file_name,
                                            );
                                        let has_exports = scope
                                            .and_then(|package_directory| {
                                                let package_json_path = tspath::combine_paths(
                                                    &package_directory,
                                                    &["package.json"],
                                                );
                                                self.get_package_json_info(&package_json_path)
                                            })
                                            .and_then(|info| info.contents)
                                            .is_some_and(|contents| {
                                                contents.fields.path_fields.exports.is_present()
                                            });
                                        if !has_exports {
                                            package_names.deep_import_packages.add(
                                                module::get_package_name_from_types_package_name(
                                                    &name,
                                                ),
                                            );
                                        }
                                    }
                                }
                                continue;
                            }
                        }
                    }
                    package_names.unresolved.add(import_text);
                }
            }
            package_names
        })
    }

    pub fn is_lib_file(&self, source_file: &ast::SourceFile) -> bool {
        self.processed_files
            .lib_files
            .contains_key(&source_file.path())
    }

    fn has_tsfile(&self) -> bool {
        *self.has_ts_file.get_or_init(|| {
            for file in &self.source_files {
                if tspath::has_implementation_tsfile_extension(&file.file_name()) {
                    return true;
                }
            }
            false
        })
    }

    pub fn has_ts_file(&self) -> bool {
        self.has_tsfile()
    }

    pub fn get_symlink_cache(&self) -> symlinks::KnownSymlinks {
        self.known_symlinks.get_value(|| {
            let mut known_symlinks = symlinks::new_known_symlink(
                &self.get_current_directory(),
                self.use_case_sensitive_file_names(),
            );

            // Resolved modules store realpath information when they're resolved inside node_modules
            if !self.processed_files.resolved_modules.is_empty()
                || !self.processed_files.type_resolutions_in_file.is_empty()
            {
                known_symlinks.set_symlinks_from_resolutions(
                    |callback, _file| {
                        for resolution in self
                            .processed_files
                            .resolved_modules
                            .values()
                            .flat_map(|resolutions| resolutions.values())
                        {
                            callback(resolution, "", core::ResolutionMode::None, String::new());
                        }
                    },
                    |callback, _file| {
                        for resolution in self
                            .processed_files
                            .type_resolutions_in_file
                            .values()
                            .flat_map(|resolutions| resolutions.values())
                        {
                            callback(resolution, "", core::ResolutionMode::None, String::new());
                        }
                    },
                );
            }

            // Check other dependencies for symlinks
            let mut seen_package_jsons = collections::Set::new();
            for (file_path, meta) in &self.processed_files.source_file_meta_datas {
                if meta.package_json_directory.is_empty()
                    || !self.source_file_may_be_emitted(
                        &self.get_source_file_by_path(file_path.clone()).unwrap(),
                        false,
                    )
                    || !seen_package_jsons.add_if_absent(self.to_path(&meta.package_json_directory))
                {
                    continue;
                }
                let package_json_name =
                    tspath::combine_paths(&meta.package_json_directory, &["package.json"]);
                let info = self.get_package_json_info(&package_json_name);
                if info.as_ref().and_then(|info| info.get_contents()).is_none() {
                    continue;
                }

                for dep in info
                    .unwrap()
                    .get_contents()
                    .unwrap()
                    .fields
                    .dependency_fields
                    .get_runtime_dependency_names()
                {
                    // Skip work in common case: we already saved a symlink for this package directory
                    // in the node_modules adjacent to this package.json
                    let possible_directory_path = self.to_path(&tspath::combine_paths(
                        &meta.package_json_directory,
                        &["node_modules", &dep],
                    ));
                    if known_symlinks.has_directory(possible_directory_path) {
                        continue;
                    }
                    if !dep.starts_with("@types") {
                        let types_package_name = module::get_types_package_name(&dep);
                        let possible_types_directory_path = self.to_path(&tspath::combine_paths(
                            &meta.package_json_directory,
                            &["node_modules", &types_package_name],
                        ));
                        if known_symlinks.has_directory(possible_types_directory_path) {
                            continue;
                        }
                    }

                    let package_resolution = self
                        .processed_files
                        .resolver
                        .lock()
                        .unwrap_or_else(|err| err.into_inner())
                        .resolve_package_directory(
                            &dep,
                            &package_json_name,
                            core::ResolutionMode::CommonJs,
                            None,
                        );
                    if let Some(package_resolution) = package_resolution
                        && package_resolution.is_resolved()
                    {
                        known_symlinks.process_resolution(
                            tspath::combine_paths(
                                &package_resolution.original_path,
                                &["package.json"],
                            ),
                            tspath::combine_paths(
                                &package_resolution.resolved_file_name,
                                &["package.json"],
                            ),
                        );
                    }
                }
            }
            known_symlinks
        })
    }
    pub fn get_symlink_cache_for_auto_imports(&self) -> symlinks::KnownSymlinks {
        self.get_symlink_cache()
    }

    pub fn resolve_module_name(
        &self,
        module_name: &str,
        containing_file: &str,
        resolution_mode: core::ResolutionMode,
    ) -> Option<module::ResolvedModule> {
        let (resolved, _) = self
            .processed_files
            .resolver
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .resolve_module_name(module_name, containing_file, resolution_mode, None);
        resolved.is_resolved().then_some(resolved)
    }

    fn for_each_resolved_module(
        &self,
        callback: impl Fn(&module::ResolvedModule, &str, core::ResolutionMode, tspath::Path),
        file: Option<&ast::SourceFile>,
    ) {
        for_each_resolution(&self.processed_files.resolved_modules, callback, file)
    }

    fn for_each_resolved_type_reference_directive(
        &self,
        callback: impl Fn(
            &module::ResolvedTypeReferenceDirective,
            &str,
            core::ResolutionMode,
            tspath::Path,
        ),
        file: Option<&ast::SourceFile>,
    ) {
        for_each_resolution(
            &self.processed_files.type_resolutions_in_file,
            callback,
            file,
        )
    }
}

fn for_each_resolution<T>(
    resolution_cache: &HashMap<tspath::Path, module::ModeAwareCache<T>>,
    callback: impl Fn(&T, &str, core::ResolutionMode, tspath::Path),
    file: Option<&ast::SourceFile>,
) {
    if let Some(file) = file {
        if let Some(resolutions) = resolution_cache.get(&file.path()) {
            for (key, resolution) in resolutions {
                callback(resolution, &key.name, key.mode, file.path());
            }
        }
    } else {
        for (file_path, resolutions) in resolution_cache {
            for (key, resolution) in resolutions {
                callback(resolution, &key.name, key.mode, file_path.clone());
            }
        }
    }
}

impl outputpaths::OutputPathsHost for Program {
    fn common_source_directory(&self) -> String {
        Program::common_source_directory(self)
    }

    fn get_current_directory(&self) -> String {
        Program::get_current_directory(self)
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        Program::use_case_sensitive_file_names(self)
    }
}

impl crate::emitter::SourceFileMayBeEmittedHost for Program {
    fn options(&self) -> &core::CompilerOptions {
        Program::options(self)
    }

    fn get_project_reference_from_source(
        &self,
        path: tspath::Path,
    ) -> Option<tsoptions::SourceOutputAndProjectReference> {
        Program::get_project_reference_from_source(self, path)
    }

    fn is_source_file_from_external_library(&self, file: &ast::SourceFile) -> bool {
        Program::is_source_file_from_external_library(self, file)
    }

    fn get_current_directory(&self) -> String {
        Program::get_current_directory(self)
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        Program::use_case_sensitive_file_names(self)
    }

    fn source_files(&self) -> Vec<&ast::SourceFile> {
        self.bound_source_files_refs()
    }
}

fn plain_jserrors() -> collections::Set<i32> {
    collections::new_set_from_items(vec![
        // binder errors
        diagnostics::Cannot_redeclare_block_scoped_variable_0.code(),
        diagnostics::A_module_cannot_have_multiple_default_exports.code(),
        diagnostics::Another_export_default_is_here.code(),
        diagnostics::The_first_export_default_is_here.code(),
        diagnostics::Identifier_expected_0_is_a_reserved_word_at_the_top_level_of_a_module.code(),
        diagnostics::Identifier_expected_0_is_a_reserved_word_in_strict_mode_Modules_are_automatically_in_strict_mode.code(),
        diagnostics::Identifier_expected_0_is_a_reserved_word_that_cannot_be_used_here.code(),
        diagnostics::X_constructor_is_a_reserved_word.code(),
        diagnostics::X_delete_cannot_be_called_on_an_identifier_in_strict_mode.code(),
        diagnostics::Code_contained_in_a_class_is_evaluated_in_JavaScript_s_strict_mode_which_does_not_allow_this_use_of_0_For_more_information_see_https_Colon_Slash_Slashdeveloper_mozilla_org_Slashen_US_Slashdocs_SlashWeb_SlashJavaScript_SlashReference_SlashStrict_mode.code(),
        diagnostics::Invalid_use_of_0_Modules_are_automatically_in_strict_mode.code(),
        diagnostics::Invalid_use_of_0_in_strict_mode.code(),
        diagnostics::A_label_is_not_allowed_here.code(),
        diagnostics::X_with_statements_are_not_allowed_in_strict_mode.code(),
        // grammar errors
        diagnostics::A_break_statement_can_only_be_used_within_an_enclosing_iteration_or_switch_statement.code(),
        diagnostics::A_break_statement_can_only_jump_to_a_label_of_an_enclosing_statement.code(),
        diagnostics::A_class_declaration_without_the_default_modifier_must_have_a_name.code(),
        diagnostics::A_class_member_cannot_have_the_0_keyword.code(),
        diagnostics::A_comma_expression_is_not_allowed_in_a_computed_property_name.code(),
        diagnostics::A_continue_statement_can_only_be_used_within_an_enclosing_iteration_statement.code(),
        diagnostics::A_continue_statement_can_only_jump_to_a_label_of_an_enclosing_iteration_statement.code(),
        diagnostics::A_default_clause_cannot_appear_more_than_once_in_a_switch_statement.code(),
        diagnostics::A_default_export_must_be_at_the_top_level_of_a_file_or_module_declaration.code(),
        diagnostics::A_definite_assignment_assertion_is_not_permitted_in_this_context.code(),
        diagnostics::A_destructuring_declaration_must_have_an_initializer.code(),
        diagnostics::A_get_accessor_cannot_have_parameters.code(),
        diagnostics::A_rest_element_cannot_contain_a_binding_pattern.code(),
        diagnostics::A_rest_element_cannot_have_a_property_name.code(),
        diagnostics::A_rest_element_cannot_have_an_initializer.code(),
        diagnostics::A_rest_element_must_be_last_in_a_destructuring_pattern.code(),
        diagnostics::A_rest_parameter_cannot_have_an_initializer.code(),
        diagnostics::A_rest_parameter_must_be_last_in_a_parameter_list.code(),
        diagnostics::A_rest_parameter_or_binding_pattern_may_not_have_a_trailing_comma.code(),
        diagnostics::A_return_statement_cannot_be_used_inside_a_class_static_block.code(),
        diagnostics::A_set_accessor_cannot_have_rest_parameter.code(),
        diagnostics::A_set_accessor_must_have_exactly_one_parameter.code(),
        diagnostics::An_export_declaration_can_only_be_used_at_the_top_level_of_a_module.code(),
        diagnostics::An_export_declaration_cannot_have_modifiers.code(),
        diagnostics::An_import_declaration_can_only_be_used_at_the_top_level_of_a_module.code(),
        diagnostics::An_import_declaration_cannot_have_modifiers.code(),
        diagnostics::An_object_member_cannot_be_declared_optional.code(),
        diagnostics::Argument_of_dynamic_import_cannot_be_spread_element.code(),
        diagnostics::Cannot_assign_to_private_method_0_Private_methods_are_not_writable.code(),
        diagnostics::Cannot_redeclare_identifier_0_in_catch_clause.code(),
        diagnostics::Catch_clause_variable_cannot_have_an_initializer.code(),
        diagnostics::Class_decorators_can_t_be_used_with_static_private_identifier_Consider_removing_the_experimental_decorator.code(),
        diagnostics::Classes_can_only_extend_a_single_class.code(),
        diagnostics::Classes_may_not_have_a_field_named_constructor.code(),
        diagnostics::Did_you_mean_to_use_a_Colon_An_can_only_follow_a_property_name_when_the_containing_object_literal_is_part_of_a_destructuring_pattern.code(),
        diagnostics::Duplicate_label_0.code(),
        diagnostics::Dynamic_imports_can_only_accept_a_module_specifier_and_an_optional_set_of_attributes_as_arguments.code(),
        diagnostics::X_for_await_loops_cannot_be_used_inside_a_class_static_block.code(),
        diagnostics::JSX_attributes_must_only_be_assigned_a_non_empty_expression.code(),
        diagnostics::JSX_elements_cannot_have_multiple_attributes_with_the_same_name.code(),
        diagnostics::JSX_expressions_may_not_use_the_comma_operator_Did_you_mean_to_write_an_array.code(),
        diagnostics::JSX_property_access_expressions_cannot_include_JSX_namespace_names.code(),
        diagnostics::Jump_target_cannot_cross_function_boundary.code(),
        diagnostics::Line_terminator_not_permitted_before_arrow.code(),
        diagnostics::Modifiers_cannot_appear_here.code(),
        diagnostics::Only_a_single_variable_declaration_is_allowed_in_a_for_in_statement.code(),
        diagnostics::Only_a_single_variable_declaration_is_allowed_in_a_for_of_statement.code(),
        diagnostics::Private_identifiers_are_not_allowed_outside_class_bodies.code(),
        diagnostics::Private_identifiers_are_only_allowed_in_class_bodies_and_may_only_be_used_as_part_of_a_class_member_declaration_property_access_or_on_the_left_hand_side_of_an_in_expression.code(),
        diagnostics::Property_0_is_not_accessible_outside_class_1_because_it_has_a_private_identifier.code(),
        diagnostics::Tagged_template_expressions_are_not_permitted_in_an_optional_chain.code(),
        diagnostics::The_left_hand_side_of_a_for_of_statement_may_not_be_async.code(),
        diagnostics::The_variable_declaration_of_a_for_in_statement_cannot_have_an_initializer.code(),
        diagnostics::The_variable_declaration_of_a_for_of_statement_cannot_have_an_initializer.code(),
        diagnostics::Trailing_comma_not_allowed.code(),
        diagnostics::Variable_declaration_list_cannot_be_empty.code(),
        diagnostics::X_0_and_1_operations_cannot_be_mixed_without_parentheses.code(),
        diagnostics::X_0_expected.code(),
        diagnostics::X_0_is_not_a_valid_meta_property_for_keyword_1_Did_you_mean_2.code(),
        diagnostics::X_0_list_cannot_be_empty.code(),
        diagnostics::X_0_modifier_already_seen.code(),
        diagnostics::X_0_modifier_cannot_appear_on_a_constructor_declaration.code(),
        diagnostics::X_0_modifier_cannot_appear_on_a_module_or_namespace_element.code(),
        diagnostics::X_0_modifier_cannot_appear_on_a_parameter.code(),
        diagnostics::X_0_modifier_cannot_appear_on_class_elements_of_this_kind.code(),
        diagnostics::X_0_modifier_cannot_be_used_here.code(),
        diagnostics::X_0_modifier_must_precede_1_modifier.code(),
        diagnostics::X_0_declarations_can_only_be_declared_inside_a_block.code(),
        diagnostics::X_0_declarations_must_be_initialized.code(),
        diagnostics::X_extends_clause_already_seen.code(),
        diagnostics::X_let_is_not_allowed_to_be_used_as_a_name_in_let_or_const_declarations.code(),
        diagnostics::Class_constructor_may_not_be_a_generator.code(),
        diagnostics::Class_constructor_may_not_be_an_accessor.code(),
        diagnostics::X_await_expressions_are_only_allowed_within_async_functions_and_at_the_top_levels_of_modules.code(),
        diagnostics::X_await_using_statements_are_only_allowed_within_async_functions_and_at_the_top_levels_of_modules.code(),
        diagnostics::Private_field_0_must_be_declared_in_an_enclosing_class.code(),
        // Type errors
        diagnostics::This_condition_will_always_return_0_since_JavaScript_compares_objects_by_reference_not_value.code(),
    ])
}

impl Program {
    fn new_empty(opts: ProgramOptions) -> Self {
        Self {
            opts,
            checker_pool: Box::new(NullCheckerPool),
            compiler_checker_pool: None,
            compare_paths_options: Default::default(),
            processed_files: Default::default(),
            source_files: Vec::new(),
            uses_uri_style_node_core_modules: core::Tristate::Unknown,
            common_source_directory: OnceLock::new(),
            declaration_diagnostic_cache: collections::SyncMap::default(),
            program_diagnostics: Mutex::new(Vec::new()),
            has_emit_blocking_diagnostics: Mutex::new(collections::Set::new()),
            source_files_to_emit: OnceLock::new(),
            bound_source_files: OnceLock::new(),
            unresolved_imports: LazyValue {
                value: OnceLock::new(),
                initialized: AtomicBool::new(false),
            },
            known_symlinks: LazyValue {
                value: OnceLock::new(),
                initialized: AtomicBool::new(false),
            },
            package_names: LazyValue {
                value: OnceLock::new(),
                initialized: AtomicBool::new(false),
            },
            has_ts_file: OnceLock::new(),
            packages_map: OnceLock::new(),
            compiler_options_cache: OnceLock::new(),
        }
    }
}
