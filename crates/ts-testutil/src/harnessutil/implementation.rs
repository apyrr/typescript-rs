use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::SystemTime;

use super::recorderfs::new_output_recorder_fs;
use super::sourcemap_recorder::{
    Mapping, RawSourceMap, WriterAggregator, new_source_map_span_writer,
};
use ts_ast as ast;
use ts_collections::{OrderedMap, SyncMap};
use ts_compiler as compiler;
use ts_core as core;
use ts_outputpaths as outputpaths;
use ts_parser as parser;
use ts_sourcemap as sourcemap;
use ts_tsoptions::{CommandLineOption, CommandLineOptionKind, CompilerOptionsValue};
use ts_tspath as tspath;
use ts_vfs as vfs;
use ts_vfs::Fs as _;

pub const TEST_LIB_FOLDER: &str = "/.lib";
pub const FAKE_TS_VERSION: &str = "FakeTSVersion";

static TEST_LIB_FOLDER_MAP: LazyLock<BTreeMap<String, vfs::vfstest::MapFile>> =
    LazyLock::new(load_test_lib_folder_map);
static TEST_LIB_FOLDER_FS_CASE_SENSITIVE: LazyLock<vfs::vfstest::MapFs> =
    LazyLock::new(|| load_test_lib_folder_fs(true));
static TEST_LIB_FOLDER_FS_CASE_INSENSITIVE: LazyLock<vfs::vfstest::MapFs> =
    LazyLock::new(|| load_test_lib_folder_fs(false));
static SHARED_SOURCE_FILE_CACHE: LazyLock<SyncMap<SourceFileCacheKey, ast::ParsedSourceFile>> =
    LazyLock::new(SyncMap::new);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestFile {
    pub unit_name: String,
    pub content: String,
}

pub type TestConfiguration = HashMap<String, String>;
pub type Diagnostic = ast::Diagnostic;
pub type EmitResult = compiler::EmitResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedTestConfiguration {
    pub name: String,
    pub config: TestConfiguration,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HarnessOptions {
    pub use_case_sensitive_file_names: bool,
    pub baseline_file: String,
    pub include_built_file: String,
    pub file_name: String,
    pub lib_files: Vec<String>,
    pub no_implicit_references: bool,
    pub current_directory: String,
    pub symlink: String,
    pub link: String,
    pub no_types_and_symbols: bool,
    pub full_emit_paths: bool,
    pub report_diagnostics: bool,
    pub capture_suggestions: bool,
    pub typescript_version: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilerOptions {
    pub module: String,
    pub module_resolution: String,
    pub module_detection: String,
    pub target: String,
    pub jsx: String,
    pub jsx_factory: String,
    pub jsx_fragment_factory: String,
    pub jsx_import_source: String,
    pub react_namespace: String,
    pub strict: Option<bool>,
    pub strict_null_checks: Option<bool>,
    pub exact_optional_property_types: Option<bool>,
    pub no_implicit_any: Option<bool>,
    pub no_implicit_this: Option<bool>,
    pub no_implicit_returns: Option<bool>,
    pub no_implicit_override: Option<bool>,
    pub strict_function_types: Option<bool>,
    pub strict_bind_call_apply: Option<bool>,
    pub strict_builtin_iterator_return: Option<bool>,
    pub strict_property_initialization: Option<bool>,
    pub stable_type_ordering: Option<bool>,
    pub allow_arbitrary_extensions: Option<bool>,
    pub allow_importing_ts_extensions: Option<bool>,
    pub no_property_access_from_index_signature: Option<bool>,
    pub no_unchecked_indexed_access: Option<bool>,
    pub use_unknown_in_catch_variables: Option<bool>,
    pub use_define_for_class_fields: Option<bool>,
    pub experimental_decorators: Option<bool>,
    pub emit_decorator_metadata: Option<bool>,
    pub isolated_modules: Option<bool>,
    pub isolated_declarations: Option<bool>,
    pub verbatim_module_syntax: Option<bool>,
    pub erasable_syntax_only: Option<bool>,
    pub es_module_interop: Option<bool>,
    pub es_module_interop_is_false: bool,
    pub allow_synthetic_default_imports: Option<bool>,
    pub allow_synthetic_default_imports_is_false: bool,
    pub always_strict_is_false: bool,
    pub downlevel_iteration: Option<bool>,
    pub charset: String,
    pub ignore_deprecations: String,
    pub keyof_strings_only: Option<bool>,
    pub no_implicit_use_strict: Option<bool>,
    pub no_strict_generic_checks: Option<bool>,
    pub out: String,
    pub suppress_excess_property_errors: Option<bool>,
    pub suppress_implicit_any_index_errors: Option<bool>,
    pub target_is_es3: bool,
    pub new_line: String,
    pub pretty: bool,
    pub skip_lib_check: bool,
    pub skip_default_lib_check: bool,
    pub no_error_truncation: bool,
    pub no_check: bool,
    pub allow_js: Option<bool>,
    pub check_js: Option<bool>,
    pub allow_umd_global_access: Option<bool>,
    pub allow_unreachable_code: Option<bool>,
    pub allow_unused_labels: Option<bool>,
    pub no_fallthrough_cases_in_switch: Option<bool>,
    pub no_unused_locals: Option<bool>,
    pub no_unused_parameters: Option<bool>,
    pub no_unchecked_side_effect_imports: Option<bool>,
    pub preserve_symlinks: Option<bool>,
    pub resolve_json_module: Option<bool>,
    pub resolve_package_json_exports: Option<bool>,
    pub resolve_package_json_imports: Option<bool>,
    pub rewrite_relative_import_extensions: Option<bool>,
    pub out_dir: String,
    pub out_file: String,
    pub project: String,
    pub root_dir: String,
    pub ts_build_info_file: String,
    pub base_url: String,
    pub paths: OrderedMap<String, Vec<String>>,
    pub paths_base_path: String,
    pub declaration_dir: String,
    pub root_dirs: Vec<String>,
    pub type_roots: Vec<String>,
    pub type_roots_configured: bool,
    pub types: Vec<String>,
    pub custom_conditions: Vec<String>,
    pub lib: Vec<String>,
    pub lib_replacement: bool,
    pub module_suffixes: Vec<String>,
    pub no_lib: Option<bool>,
    pub no_resolve: Option<bool>,
    pub trace_resolution: bool,
    pub no_emit: bool,
    pub no_emit_helpers: bool,
    pub no_emit_on_error: bool,
    pub remove_comments: bool,
    pub strip_internal: Option<bool>,
    pub source_map: bool,
    pub source_root: String,
    pub map_root: String,
    pub inline_source_map: bool,
    pub inline_sources: bool,
    pub declaration: bool,
    pub declaration_map: bool,
    pub composite: bool,
    pub incremental: Option<bool>,
    pub preserve_const_enums: Option<bool>,
    pub emit_declaration_only: bool,
    pub emit_bom: Option<bool>,
    pub import_helpers: Option<bool>,
    pub max_node_module_js_depth: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct ParsedCommandLine {
    pub compiler_options: CompilerOptions,
    pub file_names: Vec<String>,
    pub errors: Vec<String>,
    pub config_file: Option<ts_tsoptions::TsConfigSourceFile>,
    pub config_file_path: String,
}

impl PartialEq for ParsedCommandLine {
    fn eq(&self, other: &Self) -> bool {
        self.compiler_options == other.compiler_options
            && self.file_names == other.file_names
            && self.errors == other.errors
    }
}

impl Eq for ParsedCommandLine {}

#[derive(Clone)]
pub struct ProgramHandle(pub Arc<compiler::Program>);

impl std::fmt::Debug for ProgramHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ProgramHandle")
            .field(&Arc::as_ptr(&self.0))
            .finish()
    }
}

impl PartialEq for ProgramHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ProgramHandle {}

#[derive(Clone, Default)]
pub struct CompilationResult {
    pub options: CompilerOptions,
    pub harness_options: HarnessOptions,
    pub program: Option<ProgramHandle>,
    pub emit_result: Option<compiler::EmitResult>,
    pub diagnostics: Vec<ast::Diagnostic>,
    pub trace: String,
    pub union_type_ordering_checks: Vec<UnionTypeOrderingCheck>,
    pub source_file_parent_pointer_checks: Vec<SourceFileParentPointerCheck>,
    pub symlinks: HashMap<String, String>,
    pub js: OrderedMap<String, String>,
    pub dts: OrderedMap<String, String>,
    pub maps: OrderedMap<String, String>,
    pub outputs: Vec<TestFile>,
    pub inputs: Vec<TestFile>,
    pub inputs_and_outputs: OrderedMap<String, CompilationOutput>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnionTypeOrderingCheck {
    pub types: Vec<TypeOrderingEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeOrderingEntry {
    pub display: String,
    pub sort_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFileParentPointerCheck {
    pub path: String,
    pub is_default_library: bool,
    pub root: ParentPointerNode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParentPointerNode {
    pub id: usize,
    pub parent_id: Option<usize>,
    pub kind: String,
    pub text: String,
    pub children: Vec<ParentPointerNode>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompilationOutput {
    pub inputs: Vec<TestFile>,
    pub js: Option<TestFile>,
    pub dts: Option<TestFile>,
    pub map: Option<TestFile>,
}

pub type TextOutputMap = OrderedMap<String, String>;
pub type CompilationOutputMap = OrderedMap<String, CompilationOutput>;

#[derive(Clone, Default)]
struct CompilationArtifacts {
    program: Option<ProgramHandle>,
    emit_result: Option<compiler::EmitResult>,
    diagnostics: Vec<ast::Diagnostic>,
    trace: String,
    js: OrderedMap<String, String>,
    dts: OrderedMap<String, String>,
    maps: OrderedMap<String, String>,
    outputs: Vec<TestFile>,
    inputs: Vec<TestFile>,
    inputs_and_outputs: OrderedMap<String, CompilationOutput>,
}

pub fn compile_files(
    input_files: &[TestFile],
    other_files: &[TestFile],
    test_config: TestConfiguration,
    tsconfig: Option<ParsedCommandLine>,
    current_directory: &str,
    symlinks: HashMap<String, String>,
) -> CompilationResult {
    let (compiler_options, harness_options) =
        compiler_and_harness_options(&test_config, tsconfig.as_ref(), current_directory);

    compile_files_ex(
        input_files,
        other_files,
        &harness_options,
        &compiler_options,
        current_directory,
        symlinks,
        tsconfig,
    )
}

pub fn compiler_options_for_test_config(
    test_config: &TestConfiguration,
    tsconfig: Option<&ParsedCommandLine>,
    current_directory: &str,
) -> CompilerOptions {
    compiler_and_harness_options(test_config, tsconfig, current_directory).0
}

fn compiler_and_harness_options(
    test_config: &TestConfiguration,
    tsconfig: Option<&ParsedCommandLine>,
    current_directory: &str,
) -> (CompilerOptions, HarnessOptions) {
    let mut compiler_options = tsconfig
        .map(|config| config.compiler_options.clone())
        .unwrap_or_default();
    if compiler_options.new_line.is_empty() {
        compiler_options.new_line = "crlf".to_string();
    }
    compiler_options.skip_default_lib_check = true;
    compiler_options.no_error_truncation = true;

    let mut harness_options = HarnessOptions {
        use_case_sensitive_file_names: true,
        current_directory: current_directory.to_string(),
        ..HarnessOptions::default()
    };
    set_options_from_test_config(
        &test_config,
        &mut compiler_options,
        &mut harness_options,
        current_directory,
        false,
    );
    normalize_compiler_option_paths(&mut compiler_options, current_directory);

    (compiler_options, harness_options)
}

fn normalize_compiler_option_paths(options: &mut CompilerOptions, current_directory: &str) {
    if !options.out_dir.is_empty() {
        options.out_dir = normalize_absolute_path(&options.out_dir, current_directory);
    }
    if !options.project.is_empty() {
        options.project = normalize_absolute_path(&options.project, current_directory);
    }
    if !options.root_dir.is_empty() {
        options.root_dir = normalize_absolute_path(&options.root_dir, current_directory);
    }
    if !options.ts_build_info_file.is_empty() {
        options.ts_build_info_file =
            normalize_absolute_path(&options.ts_build_info_file, current_directory);
    }
    if !options.base_url.is_empty() {
        options.base_url = normalize_absolute_path(&options.base_url, current_directory);
    }
    if !options.declaration_dir.is_empty() {
        options.declaration_dir =
            normalize_absolute_path(&options.declaration_dir, current_directory);
    }
    for root_dir in &mut options.root_dirs {
        *root_dir = normalize_absolute_path(root_dir, current_directory);
    }
    for type_root in &mut options.type_roots {
        *type_root = normalize_absolute_path(type_root, current_directory);
    }
}

pub fn compile_files_ex(
    input_files: &[TestFile],
    other_files: &[TestFile],
    harness_options: &HarnessOptions,
    compiler_options: &CompilerOptions,
    current_directory: &str,
    symlinks: HashMap<String, String>,
    tsconfig: Option<ParsedCommandLine>,
) -> CompilationResult {
    let mut inputs = Vec::new();
    inputs.extend(input_files.iter().map(|file| TestFile {
        unit_name: normalize_absolute_path(&file.unit_name, current_directory),
        content: file.content.clone(),
    }));
    inputs.extend(other_files.iter().map(|file| TestFile {
        unit_name: normalize_absolute_path(&file.unit_name, current_directory),
        content: file.content.clone(),
    }));

    let artifacts = compile_files_with_host(
        input_files,
        &inputs,
        harness_options,
        compiler_options,
        current_directory,
        &symlinks,
        tsconfig,
    );

    CompilationResult {
        options: compiler_options.clone(),
        harness_options: harness_options.clone(),
        program: artifacts.program,
        emit_result: artifacts.emit_result,
        diagnostics: artifacts.diagnostics,
        trace: artifacts.trace,
        symlinks,
        js: artifacts.js,
        dts: artifacts.dts,
        maps: artifacts.maps,
        outputs: artifacts.outputs,
        inputs: if artifacts.inputs.is_empty() {
            inputs
        } else {
            artifacts.inputs
        },
        inputs_and_outputs: artifacts.inputs_and_outputs,
        ..CompilationResult::default()
    }
}

fn compile_files_with_host(
    input_files: &[TestFile],
    all_inputs: &[TestFile],
    harness_options: &HarnessOptions,
    compiler_options: &CompilerOptions,
    current_directory: &str,
    symlinks: &HashMap<String, String>,
    tsconfig: Option<ParsedCommandLine>,
) -> CompilationArtifacts {
    let mut program_file_names = input_files
        .iter()
        .map(|file| normalize_absolute_path(&file.unit_name, current_directory))
        .filter(|file_name| {
            !tspath::file_extension_is(file_name, tspath::EXTENSION_JSON)
                && !tspath::file_extension_is(file_name, tspath::EXTENSION_TS_BUILD_INFO)
        })
        .collect::<Vec<_>>();

    let test_lib_folder_prefix = format!("{TEST_LIB_FOLDER}/");
    let mut include_lib_dir = input_files
        .iter()
        .any(|file| file.content.contains(&test_lib_folder_prefix));
    for lib_file in &harness_options.lib_files {
        if lib_file == "lib.d.ts" && compiler_options.no_lib != Some(true) {
            continue;
        }
        program_file_names.push(tspath::combine_paths(TEST_LIB_FOLDER, &[lib_file.as_str()]));
        include_lib_dir = true;
    }

    let mut files = BTreeMap::<String, vfs::vfstest::MapFile>::new();
    for file in all_inputs {
        files.insert(
            normalize_absolute_path(&file.unit_name, current_directory),
            vfs::vfstest::MapFile {
                data: Arc::<[u8]>::from(file.content.as_bytes()),
                text: Some(Arc::<str>::from(file.content.as_str())),
                ..vfs::vfstest::MapFile::default()
            },
        );
    }
    let mut overlay_symlink_targets = BTreeMap::new();
    for (src, target) in symlinks {
        let src_file_name = normalize_absolute_path(src, current_directory);
        let target_file_name = normalize_absolute_path(target, current_directory);
        overlay_symlink_targets.insert(src_file_name.clone(), target_file_name.clone());
        files.insert(src_file_name, vfs::vfstest::symlink(target_file_name));
    }
    let map_fs = HarnessFs::new(
        files,
        include_lib_dir,
        harness_options.use_case_sensitive_file_names,
        overlay_symlink_targets,
    );
    let cache_fs = map_fs.clone();
    let fs = Arc::new(ts_bundled::wrap_fs(map_fs));
    let recorder = new_output_recorder_fs(fs);
    let recorder_handle = recorder.clone();

    let core_options = to_core_compiler_options(compiler_options);
    let use_case_sensitive_file_names = harness_options.use_case_sensitive_file_names;
    let config = to_parsed_command_line(
        tsconfig,
        program_file_names,
        core_options.clone(),
        current_directory,
        use_case_sensitive_file_names,
    );

    let trace_recorder = (core_options.trace_resolution == core::TS_TRUE).then(|| {
        Arc::new(Mutex::new(TracerForBaselining::new(
            use_case_sensitive_file_names,
            current_directory,
        )))
    });
    let trace_text = trace_recorder.as_ref().map(|trace_recorder| {
        let trace_recorder = trace_recorder.clone();
        Box::new(move |msg: &str| {
            trace_recorder
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .trace(msg);
        }) as Box<compiler::TraceText>
    });

    let host = compiler::new_compiler_host_with_text_trace(
        current_directory.to_string(),
        Box::new(recorder),
        ts_bundled::lib_path(),
        None,
        None,
        trace_text,
    );
    let host: Box<dyn compiler::CompilerHost> = Box::new(CachedCompilerHost::new(host, cache_fs));
    let host: Arc<dyn compiler::CompilerHost> = Arc::from(host);

    let single_threaded = if crate::testutil::test_program_is_single_threaded() {
        core::TS_TRUE
    } else {
        core::TS_UNKNOWN
    };
    let program_options = compiler::ProgramOptions {
        config: Box::new(config),
        host,
        use_source_of_project_reference: false,
        single_threaded,
        create_checker_pool: None,
        typings_location: String::new(),
        project_name: String::new(),
        type_script_version: harness_options.typescript_version.clone(),
        tracing: None,
    };
    let mut program = compiler::new_program(program_options);
    let ctx = core::Context::background();
    let diagnostics =
        collect_pre_emit_diagnostics(&program, core::Context::background(), harness_options);
    let emit_result =
        compiler::ProgramLike::emit(&mut program, ctx, compiler::EmitOptions::default());

    let artifacts = new_compilation_artifacts(
        &program,
        recorder_handle.outputs(),
        current_directory,
        use_case_sensitive_file_names,
        &core_options,
        diagnostics,
    );

    let trace = trace_recorder
        .as_ref()
        .map(|trace_recorder| {
            trace_recorder
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .trace
                .clone()
        })
        .unwrap_or_default();

    CompilationArtifacts {
        program: Some(ProgramHandle(Arc::new(program))),
        emit_result,
        trace,
        ..artifacts
    }
}

fn collect_pre_emit_diagnostics(
    program: &compiler::Program,
    ctx: core::Context,
    harness_options: &HarnessOptions,
) -> Vec<ast::Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(program.get_program_diagnostics());
    diagnostics.extend(program.get_syntactic_diagnostics(ctx.clone(), None));
    diagnostics.extend(program.get_semantic_diagnostics(ctx.clone(), None));
    diagnostics.extend(program.get_global_diagnostics(ctx.clone()));
    if program.options().get_emit_declarations() {
        diagnostics.extend(program.get_declaration_diagnostics(ctx.clone(), None));
    }
    if harness_options.capture_suggestions {
        diagnostics.extend(program.get_suggestion_diagnostics(ctx.clone(), None));
    }
    diagnostics
}

#[derive(Clone)]
struct HarnessFs {
    upper: vfs::vfstest::MapFs,
    lower: Option<vfs::vfstest::MapFs>,
    upper_symlink_targets: Arc<BTreeMap<String, String>>,
    hidden_lower_paths: Arc<RwLock<BTreeSet<String>>>,
}

impl HarnessFs {
    fn new(
        mut upper_files: BTreeMap<String, vfs::vfstest::MapFile>,
        include_lib_dir: bool,
        use_case_sensitive_file_names: bool,
        upper_symlink_targets: BTreeMap<String, String>,
    ) -> Self {
        let lower = if include_lib_dir {
            prepare_upper_files_for_test_lib_overlay(
                &mut upper_files,
                test_lib_folder_map(),
                use_case_sensitive_file_names,
            );
            Some(test_lib_folder_fs(use_case_sensitive_file_names))
        } else {
            None
        };
        let upper = vfs::vfstest::from_map(upper_files, use_case_sensitive_file_names);
        Self {
            upper,
            lower,
            upper_symlink_targets: Arc::new(upper_symlink_targets),
            hidden_lower_paths: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }

    fn read_file_text(&self, path: &str) -> (Arc<str>, bool) {
        let (text, ok) = self.upper.read_file_text(path);
        if ok || self.upper.stat(path).is_ok() {
            return (text, ok);
        }
        let Some((lower, lower_path)) = self.visible_lower_path(path) else {
            return (Arc::<str>::from(""), false);
        };
        lower.read_file_text(&lower_path)
    }

    fn visible_lower_path(&self, path: &str) -> Option<(&vfs::vfstest::MapFs, String)> {
        let lower = self.lower.as_ref()?;
        if self.is_lower_hidden(path) {
            return None;
        }
        let lower_path = self
            .translate_upper_symlink_path(path)
            .unwrap_or_else(|| path.to_owned());
        (!self.is_lower_hidden(&lower_path)).then_some((lower, lower_path))
    }

    fn lower_mutation_target(&self, path: &str, operation: &str) -> io::Result<Option<String>> {
        let Some(lower) = self.lower.as_ref() else {
            return Ok(None);
        };
        let lower_path = self
            .translate_upper_symlink_path(path)
            .unwrap_or_else(|| path.to_owned());
        if !self.is_lower_hidden(&lower_path) {
            if let Ok(info) = lower.stat(&lower_path) {
                if !info.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("{operation} {path:?}: path exists but is not a regular file"),
                    ));
                }
                return Ok(Some(lower_path));
            }
        }
        let parent = tspath::get_directory_path(&lower_path);
        if parent.is_empty() || parent == lower_path || self.is_lower_hidden(&parent) {
            return Ok(None);
        }
        match lower.stat(&parent) {
            Ok(info) if info.is_dir() => Ok(Some(lower_path)),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{operation} {path:?}: parent path exists but is not a directory"),
            )),
            Err(_) => Ok(None),
        }
    }

    fn write_upper_at_lower_target(
        &self,
        path: &str,
        lower_path: &str,
        data: &str,
    ) -> io::Result<()> {
        self.upper.set(
            lower_path,
            vfs::vfstest::MapFile {
                data: Arc::<[u8]>::from(data.as_bytes()),
                text: Some(Arc::<str>::from(data)),
                realpath: path.to_owned(),
                ..vfs::vfstest::MapFile::default()
            },
        );
        self.hide_lower_path(lower_path);
        Ok(())
    }

    fn translate_upper_symlink_path(&self, path: &str) -> Option<String> {
        let canonical_path = self.upper.get_canonical_path(path);
        for (link_path, target) in self.upper_symlink_targets.iter() {
            let canonical_link_path = self.upper.get_canonical_path(link_path);
            if canonical_path == canonical_link_path {
                return Some(target.clone());
            }
            let prefix = format!("{canonical_link_path}/");
            if let Some(rest) = canonical_path.strip_prefix(&prefix) {
                return Some(format!("{}/{}", target.trim_end_matches('/'), rest));
            }
        }
        None
    }

    fn is_lower_hidden(&self, path: &str) -> bool {
        let canonical_path = self.upper.get_canonical_path(path);
        let hidden_paths = self.hidden_lower_paths.read().unwrap();
        if hidden_paths.is_empty() {
            return false;
        }
        hidden_paths
            .iter()
            .any(|hidden_path| path_is_or_is_under(&canonical_path, hidden_path))
    }

    fn hide_lower_path(&self, path: &str) {
        if self.lower.is_none() {
            return;
        }
        let canonical_path = self.upper.get_canonical_path(path);
        self.hidden_lower_paths
            .write()
            .unwrap()
            .insert(canonical_path);
    }

    fn child_path(path: &str, child: &str) -> String {
        if path == "/" {
            format!("/{child}")
        } else {
            format!("{}/{}", path.trim_end_matches('/'), child)
        }
    }

    fn is_visible_lower_file(&self, path: &str) -> bool {
        if self.upper.stat(path).is_ok() {
            return false;
        }
        self.visible_lower_path(path)
            .is_some_and(|(lower, lower_path)| lower.file_exists(&lower_path))
    }
}

impl vfs::Fs for HarnessFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.upper.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        match self.upper.stat(path) {
            Ok(info) => return info.is_file(),
            Err(_) => {}
        }
        self.visible_lower_path(path)
            .is_some_and(|(lower, lower_path)| lower.file_exists(&lower_path))
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        let (text, ok) = self.upper.read_file(path);
        if ok || self.upper.stat(path).is_ok() {
            return (text, ok);
        }
        self.visible_lower_path(path)
            .map(|(lower, lower_path)| lower.read_file(&lower_path))
            .unwrap_or_else(|| (String::new(), false))
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        if self.upper.stat(path).is_err() {
            if let Some(lower_path) = self.lower_mutation_target(path, "write")? {
                return self.write_upper_at_lower_target(path, &lower_path, data);
            }
        }
        self.upper.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        if self.upper.stat(path).is_ok() {
            return self.upper.append_file(path, data);
        }
        if let Some(lower_path) = self.lower_mutation_target(path, "append")? {
            let mut text = String::new();
            if let Some((lower, visible_lower_path)) = self.visible_lower_path(path) {
                if visible_lower_path == lower_path {
                    let (existing, ok) = lower.read_file(&lower_path);
                    if ok {
                        text = existing;
                    }
                }
            }
            text.push_str(data);
            return self.write_upper_at_lower_target(path, &lower_path, &text);
        }
        self.upper.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.upper.remove(path)?;
        if self
            .lower
            .as_ref()
            .is_some_and(|lower| lower.stat(path).is_ok())
        {
            self.hide_lower_path(path);
        }
        Ok(())
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        if self.upper.stat(path).is_err() {
            if self
                .lower
                .as_ref()
                .is_some_and(|lower| lower.stat(path).is_ok())
                && !self.is_lower_hidden(path)
            {
                let (text, ok) = self.read_file(path);
                if ok {
                    self.write_upper_at_lower_target(path, path, &text)?;
                }
            }
        }
        self.upper.chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        match self.upper.stat(path) {
            Ok(info) => return info.is_dir(),
            Err(_) => {}
        }
        self.visible_lower_path(path)
            .is_some_and(|(lower, lower_path)| lower.directory_exists(&lower_path))
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        let upper_entries = self.upper.get_accessible_entries(path);
        if self
            .upper
            .stat(path)
            .ok()
            .is_some_and(|info| !info.is_dir())
        {
            return upper_entries;
        }
        let Some((lower, lower_path)) = self.visible_lower_path(path) else {
            return upper_entries;
        };
        let lower_entries = lower.get_accessible_entries(&lower_path);
        merge_overlay_entries(self, path, &lower_path, upper_entries, lower_entries)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        match self.upper.stat(path) {
            Ok(info) => Ok(info),
            Err(upper_err) => self
                .visible_lower_path(path)
                .map(|(lower, lower_path)| lower.stat(&lower_path))
                .unwrap_or(Err(upper_err)),
        }
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        let mut dirs = vec![root.to_owned()];
        while let Some(dir) = dirs.pop() {
            let entries = self.get_accessible_entries(&dir);
            for child in entries.directories {
                let path = HarnessFs::child_path(&dir, &child);
                walk_fn(&path, vfs::DirEntry::directory(child), None)?;
                dirs.push(path);
            }
            for child in entries.files {
                let path = HarnessFs::child_path(&dir, &child);
                walk_fn(&path, vfs::DirEntry::file(child), None)?;
            }
        }
        Ok(())
    }

    fn realpath(&self, path: &str) -> String {
        if self.upper.stat(path).is_ok() {
            return self.upper.realpath(path);
        }
        self.visible_lower_path(path)
            .map(|(lower, lower_path)| lower.realpath(&lower_path))
            .unwrap_or_else(|| path.to_owned())
    }
}

fn merge_overlay_entries(
    fs: &HarnessFs,
    upper_path: &str,
    lower_path: &str,
    upper: vfs::Entries,
    lower: vfs::Entries,
) -> vfs::Entries {
    let mut files = upper.files.into_iter().collect::<BTreeSet<_>>();
    let mut directories = upper.directories.into_iter().collect::<BTreeSet<_>>();
    let mut symlinks = upper.symlinks.unwrap_or_default();

    for file in lower.files {
        if files.contains(&file) || directories.contains(&file) {
            continue;
        }
        if fs.is_lower_hidden(&HarnessFs::child_path(lower_path, &file)) {
            continue;
        }
        files.insert(file);
    }
    for directory in lower.directories {
        if files.contains(&directory) || directories.contains(&directory) {
            continue;
        }
        if fs.is_lower_hidden(&HarnessFs::child_path(lower_path, &directory)) {
            continue;
        }
        directories.insert(directory);
    }
    if let Some(lower_symlinks) = lower.symlinks {
        for symlink in lower_symlinks {
            if fs.is_lower_hidden(&HarnessFs::child_path(upper_path, &symlink))
                || (!files.contains(&symlink) && !directories.contains(&symlink))
            {
                continue;
            }
            symlinks.insert(symlink);
        }
    }

    vfs::Entries {
        files: files.into_iter().collect(),
        directories: directories.into_iter().collect(),
        symlinks: Some(symlinks),
    }
}

fn prepare_upper_files_for_test_lib_overlay(
    upper_files: &mut BTreeMap<String, vfs::vfstest::MapFile>,
    lower_files: &BTreeMap<String, vfs::vfstest::MapFile>,
    use_case_sensitive_file_names: bool,
) {
    for lower_path in lower_files.keys() {
        upper_files.remove(lower_path);
    }
    if use_case_sensitive_file_names {
        return;
    }

    let mut lower_canonical_paths = BTreeMap::new();
    for lower_path in lower_files.keys() {
        lower_canonical_paths.insert(
            tspath::get_canonical_file_name(lower_path, use_case_sensitive_file_names),
            lower_path,
        );
    }
    for upper_path in upper_files.keys() {
        let canonical_path =
            tspath::get_canonical_file_name(upper_path, use_case_sensitive_file_names);
        let Some(lower_path) = lower_canonical_paths.get(&canonical_path) else {
            continue;
        };
        let (first, second) = if upper_path <= *lower_path {
            (upper_path.as_str(), lower_path.as_str())
        } else {
            (lower_path.as_str(), upper_path.as_str())
        };
        panic!(
            "duplicate path: {:?} and {:?} have the same canonical path",
            first, second
        );
    }
}

fn path_is_or_is_under(path: &str, parent: &str) -> bool {
    path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}

struct CachedCompilerHost {
    host: Box<dyn compiler::CompilerHost>,
    fs: HarnessFs,
    local_source_file_cache: SyncMap<SourceFileCacheKey, ast::ParsedSourceFile>,
}

impl CachedCompilerHost {
    fn new(host: Box<dyn compiler::CompilerHost>, fs: HarnessFs) -> Self {
        Self {
            host,
            fs,
            local_source_file_cache: SyncMap::new(),
        }
    }
}

impl compiler::CompilerHost for CachedCompilerHost {
    fn fs(&self) -> &dyn vfs::Fs {
        self.host.fs()
    }

    fn default_library_path(&self) -> String {
        self.host.default_library_path()
    }

    fn get_current_directory(&self) -> String {
        self.host.get_current_directory()
    }

    fn trace(&self, msg: &'static ts_diagnostics::Message, args: &compiler::DiagnosticArgs) {
        self.host.trace(msg, args);
    }

    fn trace_text(&self, msg: &str) {
        self.host.trace_text(msg);
    }

    fn get_parsed_source_file(
        &self,
        opts: ast::SourceFileParseOptions,
    ) -> Option<ast::ParsedSourceFile> {
        let (text, ok) = if ts_bundled::is_bundled(&opts.file_name) {
            let (text, ok) = self.host.fs().read_file(&opts.file_name);
            (Arc::<str>::from(text), ok)
        } else {
            self.fs.read_file_text(&opts.file_name)
        };
        if !ok {
            return None;
        }

        let script_kind = core::get_script_kind_from_file_name(&opts.file_name);
        if script_kind == core::ScriptKind::Unknown {
            panic!("Unknown script kind for file {}", opts.file_name);
        }

        let cache = if should_use_shared_source_file_cache(&opts.file_name, &self.fs) {
            &*SHARED_SOURCE_FILE_CACHE
        } else {
            &self.local_source_file_cache
        };
        let key = get_source_file_cache_key(opts.clone(), &text, script_kind);
        if let (Some(source_file), true) = cache.load(&key) {
            return Some(source_file.share_readonly());
        }

        let source_file =
            parser::parse_source_file_as_parsed_with_hash(opts, text, script_kind, key.text_hash);
        let (source_file, _) = cache.load_or_store(key, Some(source_file));
        source_file.map(|source_file| source_file.share_readonly())
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: tspath::Path,
    ) -> Option<ts_tsoptions::ParsedCommandLine> {
        self.host.get_resolved_project_reference(file_name, path)
    }
}

fn should_use_shared_source_file_cache(file_name: &str, fs: &HarnessFs) -> bool {
    ts_bundled::is_bundled(file_name) || fs.is_visible_lower_file(file_name)
}

fn test_lib_folder_map() -> &'static BTreeMap<String, vfs::vfstest::MapFile> {
    &TEST_LIB_FOLDER_MAP
}

fn test_lib_folder_fs(use_case_sensitive_file_names: bool) -> vfs::vfstest::MapFs {
    if use_case_sensitive_file_names {
        TEST_LIB_FOLDER_FS_CASE_SENSITIVE.clone()
    } else {
        TEST_LIB_FOLDER_FS_CASE_INSENSITIVE.clone()
    }
}

fn load_test_lib_folder_fs(use_case_sensitive_file_names: bool) -> vfs::vfstest::MapFs {
    vfs::vfstest::from_map(
        test_lib_folder_map()
            .iter()
            .map(|(path, file)| (path.clone(), file.clone())),
        use_case_sensitive_file_names,
    )
}

fn load_test_lib_folder_map() -> BTreeMap<String, vfs::vfstest::MapFile> {
    let test_lib_dir = ts_repo::type_script_submodule_path()
        .join("tests")
        .join("lib");
    let mut files = BTreeMap::new();
    load_test_lib_folder_map_worker(&mut files, &test_lib_dir, TEST_LIB_FOLDER);
    files
}

fn load_test_lib_folder_map_worker(
    files: &mut BTreeMap<String, vfs::vfstest::MapFile>,
    source_dir: &Path,
    virtual_dir: &str,
) {
    let mut entries = fs::read_dir(source_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", source_dir.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("failed to enumerate {}: {err}", source_dir.display()));
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let source_path = entry.path();
        let virtual_path = format!("{virtual_dir}/{}", entry.file_name().to_string_lossy());
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| panic!("failed to stat {}: {err}", source_path.display()));
        if file_type.is_dir() {
            load_test_lib_folder_map_worker(files, &source_path, &virtual_path);
        } else if file_type.is_file() {
            let data = fs::read(&source_path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", source_path.display()));
            files.insert(
                virtual_path,
                vfs::vfstest::MapFile {
                    text: vfs::vfstest::decode_bytes(&data).map(Arc::<str>::from),
                    data: Arc::<[u8]>::from(data),
                    ..vfs::vfstest::MapFile::default()
                },
            );
        }
    }
}

fn new_compilation_artifacts(
    program: &compiler::Program,
    recorded_outputs: Vec<TestFile>,
    current_directory: &str,
    use_case_sensitive_file_names: bool,
    options: &core::CompilerOptions,
    diagnostics: Vec<ast::Diagnostic>,
) -> CompilationArtifacts {
    let mut js = OrderedMap::new();
    let mut dts = OrderedMap::new();
    let mut maps = OrderedMap::new();
    for document in recorded_outputs {
        if tspath::has_js_file_extension(&document.unit_name)
            || tspath::has_json_file_extension(&document.unit_name)
        {
            js.set(document.unit_name.clone(), document);
        } else if tspath::is_declaration_file_name(&document.unit_name) {
            dts.set(document.unit_name.clone(), document);
        } else if tspath::file_extension_is(&document.unit_name, ".map") {
            maps.set(document.unit_name.clone(), document);
        }
    }

    let mut result = CompilationArtifacts {
        diagnostics,
        ..CompilationArtifacts::default()
    };

    for source_file in compiler::ProgramLike::get_parsed_source_files_refs(program) {
        let unit_name = source_file.file_name();
        if tspath::is_declaration_file_name(&unit_name) {
            continue;
        }
        let input = TestFile {
            unit_name,
            content: source_file.text().to_string(),
        };
        result.inputs.push(input.clone());

        let extname = outputpaths::get_output_extension(&input.unit_name, options.jsx);
        let js_path = get_output_path(
            program,
            current_directory,
            use_case_sensitive_file_names,
            options,
            &input.unit_name,
            &extname,
        );
        let dts_path = get_output_path(
            program,
            current_directory,
            use_case_sensitive_file_names,
            options,
            &input.unit_name,
            &tspath::get_declaration_emit_extension_for_path(&input.unit_name),
        );
        let map_ext = format!("{extname}.map");
        let map_path = get_output_path(
            program,
            current_directory,
            use_case_sensitive_file_names,
            options,
            &input.unit_name,
            &map_ext,
        );
        let outputs = CompilationOutput {
            inputs: vec![input.clone()],
            js: js.get(&js_path).cloned(),
            dts: dts.get(&dts_path).cloned(),
            map: maps.get(&map_path).cloned(),
        };
        result
            .inputs_and_outputs
            .set(input.unit_name.clone(), outputs.clone());
        if let Some(output) = &outputs.js {
            result
                .inputs_and_outputs
                .set(output.unit_name.clone(), outputs.clone());
            result
                .js
                .set(output.unit_name.clone(), output.content.clone());
            js.delete(&output.unit_name);
            result.outputs.push(output.clone());
        }
        if let Some(output) = &outputs.dts {
            result
                .inputs_and_outputs
                .set(output.unit_name.clone(), outputs.clone());
            result
                .dts
                .set(output.unit_name.clone(), output.content.clone());
            dts.delete(&output.unit_name);
            result.outputs.push(output.clone());
        }
        if let Some(output) = &outputs.map {
            result
                .inputs_and_outputs
                .set(output.unit_name.clone(), outputs.clone());
            result
                .maps
                .set(output.unit_name.clone(), output.content.clone());
            maps.delete(&output.unit_name);
            result.outputs.push(output.clone());
        }
    }

    let mut leftover_js = js.values().cloned().collect::<Vec<_>>();
    leftover_js.sort_by(compare_test_files);
    for document in leftover_js {
        result.js.set(document.unit_name, document.content);
    }
    let mut leftover_dts = dts.values().cloned().collect::<Vec<_>>();
    leftover_dts.sort_by(compare_test_files);
    for document in leftover_dts {
        result.dts.set(document.unit_name, document.content);
    }
    let mut leftover_maps = maps.values().cloned().collect::<Vec<_>>();
    leftover_maps.sort_by(compare_test_files);
    for document in leftover_maps {
        result.maps.set(document.unit_name, document.content);
    }

    result
}

fn compare_test_files(left: &TestFile, right: &TestFile) -> std::cmp::Ordering {
    left.unit_name.cmp(&right.unit_name)
}

fn get_output_path(
    program: &compiler::Program,
    current_directory: &str,
    use_case_sensitive_file_names: bool,
    options: &core::CompilerOptions,
    path: &str,
    ext: &str,
) -> String {
    let mut path = tspath::resolve_path(current_directory, &[path]);
    let mut out_dir = if ext == ".d.ts"
        || ext == ".d.mts"
        || ext == ".d.cts"
        || (ext.ends_with(".ts") && ext.contains(".d."))
    {
        options.declaration_dir.clone()
    } else {
        options.out_dir.clone()
    };
    if out_dir.is_empty()
        && (ext == ".d.ts"
            || ext == ".d.mts"
            || ext == ".d.cts"
            || (ext.ends_with(".ts") && ext.contains(".d.")))
    {
        out_dir = options.out_dir.clone();
    }
    if !out_dir.is_empty() {
        let common = compiler::ProgramLike::common_source_directory(program);
        if !common.is_empty() {
            path = tspath::get_relative_path_from_directory(
                &common,
                &path,
                &tspath::ComparePathsOptions {
                    use_case_sensitive_file_names,
                    current_directory: current_directory.to_string(),
                },
            );
            path = tspath::combine_paths(
                &tspath::resolve_path(current_directory, &[&options.out_dir]),
                &[&path],
            );
        }
    }
    tspath::change_extension(&path, ext)
}

fn to_parsed_command_line(
    tsconfig: Option<ParsedCommandLine>,
    file_names: Vec<String>,
    compiler_options: core::CompilerOptions,
    current_directory: &str,
    use_case_sensitive_file_names: bool,
) -> ts_tsoptions::ParsedCommandLine {
    let mut config = ts_tsoptions::ParsedCommandLine {
        file_names: tsconfig
            .as_ref()
            .filter(|config| !config.file_names.is_empty())
            .map(|config| config.file_names.clone())
            .unwrap_or(file_names),
        errors: tsconfig
            .as_ref()
            .map(|config| config.errors.clone())
            .unwrap_or_default(),
        config_file_path: tsconfig
            .as_ref()
            .map(|config| config.config_file_path.clone())
            .unwrap_or_default(),
        config_file: tsconfig.and_then(|config| config.config_file),
        current_directory: current_directory.to_string(),
        use_case_sensitive_file_names,
        ..Default::default()
    };
    config.literal_file_names_len = config.file_names.len();
    config.set_compiler_options(compiler_options);
    config
}

fn to_core_compiler_options(options: &CompilerOptions) -> core::CompilerOptions {
    let mut result = core::CompilerOptions {
        new_line: match options.new_line.as_str() {
            "lf" | "\n" => core::NEW_LINE_KIND_LF,
            "crlf" | "\r\n" | "" => core::NEW_LINE_KIND_CRLF,
            _ => core::NEW_LINE_KIND_NONE,
        },
        pretty: if options.pretty {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        skip_lib_check: if options.skip_lib_check {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        skip_default_lib_check: if options.skip_default_lib_check {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        no_error_truncation: if options.no_error_truncation {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        allow_js: match options.allow_js {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        check_js: match options.check_js {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        allow_umd_global_access: match options.allow_umd_global_access {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        allow_unreachable_code: match options.allow_unreachable_code {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        allow_unused_labels: match options.allow_unused_labels {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_fallthrough_cases_in_switch: match options.no_fallthrough_cases_in_switch {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_unused_locals: match options.no_unused_locals {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_unused_parameters: match options.no_unused_parameters {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_unchecked_side_effect_imports: match options.no_unchecked_side_effect_imports {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        always_strict: if options.always_strict_is_false {
            core::TS_FALSE
        } else {
            core::TS_UNKNOWN
        },
        downlevel_iteration: match options.downlevel_iteration {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        charset: options.charset.clone(),
        ignore_deprecations: options.ignore_deprecations.clone(),
        keyof_strings_only: match options.keyof_strings_only {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_implicit_use_strict: match options.no_implicit_use_strict {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_strict_generic_checks: match options.no_strict_generic_checks {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        out: options.out.clone(),
        suppress_excess_property_errors: match options.suppress_excess_property_errors {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        suppress_implicit_any_index_errors: match options.suppress_implicit_any_index_errors {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        target_is_es3: options.target_is_es3,
        strict: match options.strict {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        strict_null_checks: match options.strict_null_checks {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        exact_optional_property_types: match options.exact_optional_property_types {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_implicit_any: match options.no_implicit_any {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_implicit_this: match options.no_implicit_this {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_implicit_returns: match options.no_implicit_returns {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_implicit_override: match options.no_implicit_override {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        strict_function_types: match options.strict_function_types {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        strict_bind_call_apply: match options.strict_bind_call_apply {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        strict_builtin_iterator_return: match options.strict_builtin_iterator_return {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        strict_property_initialization: match options.strict_property_initialization {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        stable_type_ordering: match options.stable_type_ordering {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        allow_arbitrary_extensions: match options.allow_arbitrary_extensions {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        allow_importing_ts_extensions: match options.allow_importing_ts_extensions {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_property_access_from_index_signature: match options
            .no_property_access_from_index_signature
        {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_unchecked_indexed_access: match options.no_unchecked_indexed_access {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        use_unknown_in_catch_variables: match options.use_unknown_in_catch_variables {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        use_define_for_class_fields: match options.use_define_for_class_fields {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        experimental_decorators: match options.experimental_decorators {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        emit_decorator_metadata: match options.emit_decorator_metadata {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        isolated_modules: match options.isolated_modules {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        isolated_declarations: match options.isolated_declarations {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        verbatim_module_syntax: match options.verbatim_module_syntax {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        erasable_syntax_only: match options.erasable_syntax_only {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        es_module_interop: match options.es_module_interop {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None if options.es_module_interop_is_false => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        allow_synthetic_default_imports: match options.allow_synthetic_default_imports {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None if options.allow_synthetic_default_imports_is_false => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_emit: if options.no_emit {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        no_emit_helpers: if options.no_emit_helpers {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        no_emit_on_error: if options.no_emit_on_error {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        no_check: if options.no_check {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        remove_comments: if options.remove_comments {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        strip_internal: match options.strip_internal {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        source_map: if options.source_map {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        source_root: options.source_root.clone(),
        map_root: options.map_root.clone(),
        inline_source_map: if options.inline_source_map {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        inline_sources: if options.inline_sources {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        declaration: if options.declaration {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        declaration_map: if options.declaration_map {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        composite: if options.composite {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        incremental: match options.incremental {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        jsx_factory: options.jsx_factory.clone(),
        jsx_fragment_factory: options.jsx_fragment_factory.clone(),
        jsx_import_source: options.jsx_import_source.clone(),
        react_namespace: options.react_namespace.clone(),
        preserve_const_enums: match options.preserve_const_enums {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        emit_declaration_only: if options.emit_declaration_only {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        emit_bom: match options.emit_bom {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        import_helpers: match options.import_helpers {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        out_dir: options.out_dir.clone(),
        out_file: options.out_file.clone(),
        root_dir: options.root_dir.clone(),
        ts_build_info_file: options.ts_build_info_file.clone(),
        base_url: options.base_url.clone(),
        paths: options.paths.clone(),
        paths_base_path: options.paths_base_path.clone(),
        declaration_dir: options.declaration_dir.clone(),
        root_dirs: options.root_dirs.clone(),
        type_roots: options.type_roots.clone(),
        type_roots_configured: options.type_roots_configured,
        types: options.types.clone(),
        custom_conditions: options.custom_conditions.clone(),
        lib: options.lib.clone(),
        lib_replacement: if options.lib_replacement {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        module_suffixes: options.module_suffixes.clone(),
        no_lib: match options.no_lib {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        no_resolve: match options.no_resolve {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        max_node_module_js_depth: options.max_node_module_js_depth,
        preserve_symlinks: match options.preserve_symlinks {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        resolve_json_module: match options.resolve_json_module {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        resolve_package_json_exports: match options.resolve_package_json_exports {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        resolve_package_json_imports: match options.resolve_package_json_imports {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        rewrite_relative_import_extensions: match options.rewrite_relative_import_extensions {
            Some(true) => core::TS_TRUE,
            Some(false) => core::TS_FALSE,
            None => core::TS_UNKNOWN,
        },
        trace_resolution: if options.trace_resolution {
            core::TS_TRUE
        } else {
            core::TS_UNKNOWN
        },
        ..Default::default()
    };
    result.target = parse_script_target(&options.target);
    result.module = parse_module_kind(&options.module);
    result.module_resolution = parse_module_resolution_kind(&options.module_resolution);
    result.module_detection = parse_module_detection_kind(&options.module_detection);
    result.jsx = parse_jsx_emit(&options.jsx);
    result
}

fn parse_script_target(value: &str) -> core::ScriptTarget {
    match value.to_ascii_lowercase().as_str() {
        "es3" => core::SCRIPT_TARGET_NONE,
        "es5" => core::SCRIPT_TARGET_ES5,
        "es2015" | "es6" => core::SCRIPT_TARGET_ES2015,
        "es2016" => core::SCRIPT_TARGET_ES2016,
        "es2017" => core::SCRIPT_TARGET_ES2017,
        "es2018" => core::SCRIPT_TARGET_ES2018,
        "es2019" => core::SCRIPT_TARGET_ES2019,
        "es2020" => core::SCRIPT_TARGET_ES2020,
        "es2021" => core::SCRIPT_TARGET_ES2021,
        "es2022" => core::SCRIPT_TARGET_ES2022,
        "es2023" => core::SCRIPT_TARGET_ES2023,
        "es2024" => core::SCRIPT_TARGET_ES2024,
        "es2025" => core::SCRIPT_TARGET_ES2025,
        "esnext" | "latest" => core::SCRIPT_TARGET_ES_NEXT,
        "json" => core::SCRIPT_TARGET_JSON,
        _ => core::SCRIPT_TARGET_NONE,
    }
}

fn parse_module_kind(value: &str) -> core::ModuleKind {
    match value.to_ascii_lowercase().as_str() {
        "none" => core::MODULE_KIND_NONE,
        "commonjs" => core::MODULE_KIND_COMMON_JS,
        "amd" => core::MODULE_KIND_AMD,
        "umd" => core::MODULE_KIND_UMD,
        "system" => core::MODULE_KIND_SYSTEM,
        "es2015" | "es6" => core::MODULE_KIND_ES2015,
        "es2020" => core::MODULE_KIND_ES2020,
        "es2022" => core::MODULE_KIND_ES2022,
        "esnext" => core::MODULE_KIND_ES_NEXT,
        "node16" => core::MODULE_KIND_NODE16,
        "node18" => core::MODULE_KIND_NODE18,
        "node20" => core::MODULE_KIND_NODE20,
        "nodenext" => core::MODULE_KIND_NODE_NEXT,
        "preserve" => core::MODULE_KIND_PRESERVE,
        _ => core::MODULE_KIND_NONE,
    }
}

fn parse_module_resolution_kind(value: &str) -> core::ModuleResolutionKind {
    match value.to_ascii_lowercase().as_str() {
        "classic" => core::MODULE_RESOLUTION_KIND_CLASSIC,
        "node" | "node10" => core::MODULE_RESOLUTION_KIND_NODE10,
        "node16" => core::MODULE_RESOLUTION_KIND_NODE16,
        "nodenext" => core::MODULE_RESOLUTION_KIND_NODE_NEXT,
        "bundler" => core::MODULE_RESOLUTION_KIND_BUNDLER,
        _ => core::MODULE_RESOLUTION_KIND_UNKNOWN,
    }
}

fn parse_module_detection_kind(value: &str) -> core::ModuleDetectionKind {
    match value.to_ascii_lowercase().as_str() {
        "legacy" => core::ModuleDetectionKind::Legacy,
        "auto" => core::ModuleDetectionKind::Auto,
        "force" => core::ModuleDetectionKind::Force,
        _ => core::ModuleDetectionKind::None,
    }
}

fn parse_jsx_emit(value: &str) -> core::JsxEmit {
    match value.to_ascii_lowercase().as_str() {
        "preserve" => core::JsxEmit::Preserve,
        "react-native" | "reactnative" => core::JsxEmit::ReactNative,
        "react" => core::JsxEmit::React,
        "react-jsx" | "reactjsx" => core::JsxEmit::ReactJSX,
        "react-jsxdev" | "reactjsxdev" => core::JsxEmit::ReactJSXDev,
        _ => core::JsxEmit::None,
    }
}

pub fn verify_union_ordering_checks(checks: &[UnionTypeOrderingCheck]) {
    for union in checks {
        let mut reversed = union.types.clone();
        reversed.reverse();
        reversed.sort_by(compare_type_ordering_entries);
        assert_eq!(
            reversed, union.types,
            "compare_types does not sort union types consistently"
        );

        let mut shuffled = union.types.clone();
        for seed in 0..10 {
            deterministic_shuffle(&mut shuffled, seed);
            shuffled.sort_by(compare_type_ordering_entries);
            assert_eq!(
                shuffled, union.types,
                "compare_types does not sort union types consistently"
            );
        }
    }
}

pub fn verify_parent_pointer_checks(checks: &[SourceFileParentPointerCheck]) {
    for source_file in checks {
        if source_file.is_default_library {
            continue;
        }
        for child in &source_file.root.children {
            verify_parent_pointer_node(child, source_file.root.id);
        }
    }
}

fn compare_type_ordering_entries(
    left: &TypeOrderingEntry,
    right: &TypeOrderingEntry,
) -> std::cmp::Ordering {
    left.sort_key
        .cmp(&right.sort_key)
        .then(left.display.cmp(&right.display))
}

fn deterministic_shuffle<T>(items: &mut [T], seed: usize) {
    if items.len() < 2 {
        return;
    }
    let mut state = 0x9E37_79B9_7F4A_7C15usize ^ seed;
    for index in (1..items.len()).rev() {
        state = state
            .wrapping_mul(6364136223846793005usize)
            .wrapping_add(1442695040888963407usize);
        items.swap(index, state % (index + 1));
    }
}

fn verify_parent_pointer_node(node: &ParentPointerNode, parent_id: usize) {
    assert_eq!(
        node.parent_id,
        Some(parent_id),
        "parent node does not match traversed parent: {}: {}",
        node.kind,
        node.text
    );
    for child in &node.children {
        verify_parent_pointer_node(child, node.id);
    }
}

pub fn set_options_from_test_config(
    test_config: &TestConfiguration,
    compiler_options: &mut CompilerOptions,
    harness_options: &mut HarnessOptions,
    current_directory: &str,
    allow_unknown_options: bool,
) {
    for (key, value) in test_config {
        match key.to_ascii_lowercase().as_str() {
            "baselinefile" => harness_options.baseline_file = value.clone(),
            "filename" => harness_options.file_name = value.clone(),
            "libfiles" => {
                harness_options.lib_files = split_option_values(value, key);
            }
            "noimplicitreferences" => {
                harness_options.no_implicit_references = parse_bool(value);
            }
            "currentdirectory" => harness_options.current_directory = value.clone(),
            "symlink" => harness_options.symlink = value.clone(),
            "link" => harness_options.link = value.clone(),
            "notypesandsymbols" => harness_options.no_types_and_symbols = parse_bool(value),
            "fullemitpaths" => harness_options.full_emit_paths = parse_bool(value),
            "reportdiagnostics" => harness_options.report_diagnostics = parse_bool(value),
            "capturesuggestions" => harness_options.capture_suggestions = parse_bool(value),
            "typescriptversion" => harness_options.typescript_version = value.clone(),
            "usecasesensitivefilenames" => {
                harness_options.use_case_sensitive_file_names = parse_bool(value)
            }
            "outdir" => {
                compiler_options.out_dir = normalize_absolute_path(value, current_directory)
            }
            "outfile" => {
                compiler_options.out_file = normalize_absolute_path(value, current_directory)
            }
            "project" => {
                compiler_options.project = normalize_absolute_path(value, current_directory)
            }
            "rootdir" => {
                compiler_options.root_dir = normalize_absolute_path(value, current_directory)
            }
            "tsbuildinfofile" => {
                compiler_options.ts_build_info_file =
                    normalize_absolute_path(value, current_directory)
            }
            "baseurl" => {
                compiler_options.base_url = normalize_absolute_path(value, current_directory)
            }
            "declarationdir" => {
                compiler_options.declaration_dir = normalize_absolute_path(value, current_directory)
            }
            "typeroots" => {
                compiler_options.type_roots = split_comma_list(value)
                    .into_iter()
                    .map(|value| normalize_absolute_path(&value, current_directory))
                    .collect();
                compiler_options.type_roots_configured = true;
            }
            "lib" => {
                let option =
                    get_command_line_option(key).expect("lib should be a known compiler option");
                compiler_options.lib = parse_list_type_option(&option, value, current_directory);
            }
            "libreplacement" => compiler_options.lib_replacement = parse_bool(value),
            "types" => compiler_options.types = split_comma_list(value),
            "customconditions" => compiler_options.custom_conditions = split_comma_list(value),
            "modulesuffixes" => compiler_options.module_suffixes = split_comma_list(value),
            "module" => compiler_options.module = value.clone(),
            "moduleresolution" => compiler_options.module_resolution = value.clone(),
            "moduledetection" => compiler_options.module_detection = value.clone(),
            "target" => {
                compiler_options.target = value.clone();
                compiler_options.target_is_es3 = value.eq_ignore_ascii_case("es3");
            }
            "newline" => compiler_options.new_line = value.to_ascii_lowercase(),
            "jsx" => compiler_options.jsx = value.clone(),
            "jsxfactory" => compiler_options.jsx_factory = value.clone(),
            "jsxfragmentfactory" => compiler_options.jsx_fragment_factory = value.clone(),
            "reactnamespace" => compiler_options.react_namespace = value.clone(),
            "strict" => compiler_options.strict = Some(parse_bool(value)),
            "skiplibcheck" => compiler_options.skip_lib_check = parse_bool(value),
            "skipdefaultlibcheck" => compiler_options.skip_default_lib_check = parse_bool(value),
            "strictnullchecks" => compiler_options.strict_null_checks = Some(parse_bool(value)),
            "exactoptionalpropertytypes" => {
                compiler_options.exact_optional_property_types = Some(parse_bool(value))
            }
            "noimplicitany" => compiler_options.no_implicit_any = Some(parse_bool(value)),
            "noimplicitthis" => compiler_options.no_implicit_this = Some(parse_bool(value)),
            "noimplicitreturns" => compiler_options.no_implicit_returns = Some(parse_bool(value)),
            "noimplicitoverride" => compiler_options.no_implicit_override = Some(parse_bool(value)),
            "strictfunctiontypes" => {
                compiler_options.strict_function_types = Some(parse_bool(value))
            }
            "strictbindcallapply" => {
                compiler_options.strict_bind_call_apply = Some(parse_bool(value))
            }
            "strictbuiltiniteratorreturn" => {
                compiler_options.strict_builtin_iterator_return = Some(parse_bool(value))
            }
            "strictpropertyinitialization" => {
                compiler_options.strict_property_initialization = Some(parse_bool(value))
            }
            "stabletypeordering" => compiler_options.stable_type_ordering = Some(parse_bool(value)),
            "allowarbitraryextensions" => {
                compiler_options.allow_arbitrary_extensions = Some(parse_bool(value))
            }
            "allowimportingtsextensions" => {
                compiler_options.allow_importing_ts_extensions = Some(parse_bool(value))
            }
            "nopropertyaccessfromindexsignature" => {
                compiler_options.no_property_access_from_index_signature = Some(parse_bool(value))
            }
            "nouncheckedindexedaccess" => {
                compiler_options.no_unchecked_indexed_access = Some(parse_bool(value))
            }
            "useunknownincatchvariables" => {
                compiler_options.use_unknown_in_catch_variables = Some(parse_bool(value))
            }
            "usedefineforclassfields" => {
                compiler_options.use_define_for_class_fields = Some(parse_bool(value))
            }
            "experimentaldecorators" => {
                compiler_options.experimental_decorators = Some(parse_bool(value))
            }
            "emitdecoratormetadata" => {
                compiler_options.emit_decorator_metadata = Some(parse_bool(value))
            }
            "isolatedmodules" => compiler_options.isolated_modules = Some(parse_bool(value)),
            "isolateddeclarations" => {
                compiler_options.isolated_declarations = Some(parse_bool(value))
            }
            "verbatimmodulesyntax" => {
                compiler_options.verbatim_module_syntax = Some(parse_bool(value))
            }
            "erasablesyntaxonly" => compiler_options.erasable_syntax_only = Some(parse_bool(value)),
            "jsximportsource" => compiler_options.jsx_import_source = value.to_string(),
            "allowjs" => compiler_options.allow_js = Some(parse_bool(value)),
            "checkjs" => compiler_options.check_js = Some(parse_bool(value)),
            "allowumdglobalaccess" => {
                compiler_options.allow_umd_global_access = Some(parse_bool(value))
            }
            "allowunreachablecode" => {
                compiler_options.allow_unreachable_code = Some(parse_bool(value))
            }
            "allowunusedlabels" => compiler_options.allow_unused_labels = Some(parse_bool(value)),
            "nofallthroughcasesinswitch" => {
                compiler_options.no_fallthrough_cases_in_switch = Some(parse_bool(value))
            }
            "nounusedlocals" => compiler_options.no_unused_locals = Some(parse_bool(value)),
            "nounusedparameters" => compiler_options.no_unused_parameters = Some(parse_bool(value)),
            "nouncheckedsideeffectimports" => {
                compiler_options.no_unchecked_side_effect_imports = Some(parse_bool(value))
            }
            "preservesymlinks" => compiler_options.preserve_symlinks = Some(parse_bool(value)),
            "resolvejsonmodule" => compiler_options.resolve_json_module = Some(parse_bool(value)),
            "resolvepackagejsonexports" => {
                compiler_options.resolve_package_json_exports = Some(parse_bool(value))
            }
            "resolvepackagejsonimports" => {
                compiler_options.resolve_package_json_imports = Some(parse_bool(value))
            }
            "rewriterelativeimportextensions" => {
                compiler_options.rewrite_relative_import_extensions = Some(parse_bool(value))
            }
            "maxnodemodulejsdepth" => {
                compiler_options.max_node_module_js_depth = value.parse().ok()
            }
            "esmoduleinterop" => {
                let value = parse_bool(value);
                compiler_options.es_module_interop = Some(value);
                compiler_options.es_module_interop_is_false = !value;
            }
            "allowsyntheticdefaultimports" => {
                let value = parse_bool(value);
                compiler_options.allow_synthetic_default_imports = Some(value);
                compiler_options.allow_synthetic_default_imports_is_false = !value;
            }
            "alwaysstrict" => compiler_options.always_strict_is_false = !parse_bool(value),
            "downleveliteration" => compiler_options.downlevel_iteration = Some(parse_bool(value)),
            "charset" => compiler_options.charset = value.clone(),
            "ignoredeprecations" => compiler_options.ignore_deprecations = value.clone(),
            "keyofstringsonly" => compiler_options.keyof_strings_only = Some(parse_bool(value)),
            "noimplicitusestrict" => {
                compiler_options.no_implicit_use_strict = Some(parse_bool(value))
            }
            "nostrictgenericchecks" => {
                compiler_options.no_strict_generic_checks = Some(parse_bool(value))
            }
            "out" => compiler_options.out = normalize_absolute_path(value, current_directory),
            "suppressexcesspropertyerrors" => {
                compiler_options.suppress_excess_property_errors = Some(parse_bool(value))
            }
            "suppressimplicitanyindexerrors" => {
                compiler_options.suppress_implicit_any_index_errors = Some(parse_bool(value))
            }
            "pretty" => compiler_options.pretty = parse_bool(value),
            "traceresolution" => compiler_options.trace_resolution = parse_bool(value),
            "nolib" => compiler_options.no_lib = Some(parse_bool(value)),
            "noresolve" => compiler_options.no_resolve = Some(parse_bool(value)),
            "noemit" => compiler_options.no_emit = parse_bool(value),
            "noemithelpers" => compiler_options.no_emit_helpers = parse_bool(value),
            "importhelpers" => compiler_options.import_helpers = Some(parse_bool(value)),
            "noemitonerror" => compiler_options.no_emit_on_error = parse_bool(value),
            "nocheck" => compiler_options.no_check = parse_bool(value),
            "removecomments" => compiler_options.remove_comments = parse_bool(value),
            "stripinternal" => compiler_options.strip_internal = Some(parse_bool(value)),
            "sourcemap" => compiler_options.source_map = parse_bool(value),
            "sourceroot" => compiler_options.source_root = value.to_owned(),
            "maproot" => compiler_options.map_root = value.to_owned(),
            "inlinesourcemap" => compiler_options.inline_source_map = parse_bool(value),
            "inlinesources" => compiler_options.inline_sources = parse_bool(value),
            "declaration" => compiler_options.declaration = parse_bool(value),
            "declarationmap" => compiler_options.declaration_map = parse_bool(value),
            "composite" => compiler_options.composite = parse_bool(value),
            "incremental" => compiler_options.incremental = Some(parse_bool(value)),
            "preserveconstenums" => compiler_options.preserve_const_enums = Some(parse_bool(value)),
            "emitdeclarationonly" => compiler_options.emit_declaration_only = parse_bool(value),
            "emitbom" => compiler_options.emit_bom = Some(parse_bool(value)),
            _ if allow_unknown_options => {}
            _ => {}
        }
    }
}

#[derive(Clone, Default, Eq, Hash, PartialEq)]
struct SourceFileCacheKey {
    parse_options: ast::SourceFileParseOptions,
    text_hash: u128,
    text_len: usize,
    script_kind: core::ScriptKind,
}

fn get_source_file_cache_key(
    parse_options: ast::SourceFileParseOptions,
    text: &str,
    script_kind: core::ScriptKind,
) -> SourceFileCacheKey {
    SourceFileCacheKey {
        parse_options,
        text_hash: xxhash_rust::xxh3::xxh3_128(text.as_bytes()),
        text_len: text.len(),
        script_kind,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TracerForBaselining {
    pub trace: String,
    use_case_sensitive_file_names: bool,
    current_directory: String,
    package_json_cache: HashMap<tspath::Path, bool>,
}

impl TracerForBaselining {
    pub fn new(use_case_sensitive_file_names: bool, current_directory: &str) -> Self {
        Self {
            use_case_sensitive_file_names,
            current_directory: current_directory.to_string(),
            ..Default::default()
        }
    }

    pub fn trace(&mut self, msg: &str) {
        self.trace_with_writer(msg, true);
    }

    pub fn trace_with_writer(&mut self, msg: &str, use_package_json_cache: bool) {
        let msg = self.sanitize_trace(msg, use_package_json_cache);
        self.trace.push_str(&msg);
        self.trace.push('\n');
    }

    pub fn sanitize_trace(&mut self, msg: &str, use_package_json_cache: bool) -> String {
        let msg = msg.replace('\\', "/").replace(
            &format!("'{}'", core::version()),
            &format!("'{FAKE_TS_VERSION}'"),
        );

        if let Some(file) = msg.strip_prefix("File '").and_then(|msg| {
            msg.strip_suffix("' does not exist according to earlier cached lookups.")
        }) {
            if use_package_json_cache {
                let file_path = tspath::to_path(
                    file,
                    &self.current_directory,
                    self.use_case_sensitive_file_names,
                );
                if self.package_json_cache.contains_key(&file_path) {
                    return msg;
                }
                self.package_json_cache.insert(file_path, false);
            }
            return format!("File '{file}' does not exist.");
        }

        if let Some(file) = msg
            .strip_prefix("File '")
            .and_then(|msg| msg.strip_suffix("' exists according to earlier cached lookups."))
        {
            if use_package_json_cache {
                let file_path = tspath::to_path(
                    file,
                    &self.current_directory,
                    self.use_case_sensitive_file_names,
                );
                if self.package_json_cache.contains_key(&file_path) {
                    return msg;
                }
                self.package_json_cache.insert(file_path, true);
            }
            return format!("Found 'package.json' at '{file}'.");
        }

        if use_package_json_cache {
            if let Some(file) = msg
                .strip_prefix("File '")
                .and_then(|msg| msg.strip_suffix("' does not exist."))
            {
                let file_path = tspath::to_path(
                    file,
                    &self.current_directory,
                    self.use_case_sensitive_file_names,
                );
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.package_json_cache.entry(file_path)
                {
                    entry.insert(false);
                    return msg;
                }
                return format!(
                    "File '{file}' does not exist according to earlier cached lookups."
                );
            }

            if let Some(file) = msg
                .strip_prefix("Found 'package.json' at '")
                .and_then(|msg| msg.strip_suffix("'."))
            {
                let file_path = tspath::to_path(
                    file,
                    &self.current_directory,
                    self.use_case_sensitive_file_names,
                );
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.package_json_cache.entry(file_path)
                {
                    entry.insert(true);
                    return msg;
                }
                return format!("File '{file}' exists according to earlier cached lookups.");
            }
        }

        msg
    }

    pub fn reset(&mut self) {
        self.trace.clear();
        self.package_json_cache.clear();
    }
}

impl std::fmt::Display for TracerForBaselining {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.trace)
    }
}

pub fn get_source_map_record_from_parts(
    emit_result: Option<&compiler::EmitResult>,
    program: Option<&ProgramHandle>,
    js: &OrderedMap<String, String>,
    dts: &OrderedMap<String, String>,
) -> String {
    let Some(emit_result) = emit_result else {
        return String::new();
    };
    if emit_result.source_maps.is_empty() {
        return String::new();
    }
    let Some(program) = program else {
        return String::new();
    };

    let mut source_map_recorder = WriterAggregator::default();
    for source_map_data in &emit_result.source_maps {
        let current_file_content =
            if tspath::is_declaration_file_name(&source_map_data.generated_file) {
                dts.get(&source_map_data.generated_file)
            } else {
                js.get(&source_map_data.generated_file)
            }
            .unwrap_or_else(|| {
                panic!(
                    "missing generated file for source map: {}",
                    source_map_data.generated_file
                )
            });
        let current_file = TestFile {
            unit_name: source_map_data.generated_file.clone(),
            content: current_file_content.clone(),
        };
        let source_map = RawSourceMap {
            file: source_map_data.source_map.file.clone(),
            source_root: source_map_data.source_map.source_root.clone(),
            sources: source_map_data.source_map.sources.clone(),
            sources_content: source_map_data.source_map.sources_content.clone(),
            names: source_map_data.source_map.names.clone(),
            mappings: source_map_data.source_map.mappings.clone(),
        };
        let mut source_map_span_writer =
            new_source_map_span_writer(source_map_recorder, &source_map, current_file);
        let mut mapper = sourcemap::decode_mappings(source_map_data.source_map.mappings.clone());
        let mut previous_source_file_name: Option<String> = None;
        for decoded_source_mapping in &mut mapper {
            let current_source_file = if decoded_source_mapping.is_source_mapping() {
                let input_source_file_name = &source_map_data.input_source_file_names
                    [decoded_source_mapping.source_index as usize];
                program.0.get_source_file(input_source_file_name)
            } else {
                None
            };
            let current_source_file_name =
                current_source_file.as_ref().map(|file| file.file_name());
            let source_map_span = mapping_from_sourcemap(&decoded_source_mapping);
            if current_source_file_name != previous_source_file_name {
                if let Some(current_source_file) = current_source_file {
                    source_map_span_writer.record_new_source_file_span(
                        source_map_span,
                        current_source_file.text().to_string(),
                    );
                }
                previous_source_file_name = current_source_file_name;
            } else {
                source_map_span_writer.record_source_map_span(source_map_span);
            }
        }
        source_map_span_writer.close();
        source_map_recorder = source_map_span_writer.source_map_recorder;
    }
    source_map_recorder.to_string()
}

fn mapping_from_sourcemap(mapping: &sourcemap::Mapping) -> Mapping {
    Mapping {
        generated_line: mapping.generated_line,
        generated_character: mapping.generated_character,
        source_index: mapping.source_index,
        source_line: mapping.source_line,
        source_character: mapping.source_character,
        name_index: mapping.name_index,
    }
}

pub fn enumerate_files(
    folder: &str,
    test_regex: &str,
    recursive: bool,
) -> std::io::Result<Vec<String>> {
    list_files(folder, test_regex, recursive)
}

pub fn list_files(path: &str, spec: &str, recursive: bool) -> std::io::Result<Vec<String>> {
    let mut files = list_files_worker(spec, recursive, path)?;
    files.sort();
    Ok(files)
}

pub fn list_files_worker(
    spec: &str,
    recursive: bool,
    folder: &str,
) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && recursive {
            files.extend(list_files_worker(spec, recursive, &path.to_string_lossy())?);
        } else if path.is_file() {
            let path_string = path.to_string_lossy().replace('\\', "/");
            if path_string.contains(spec) || spec.is_empty() {
                files.push(path_string);
            }
        }
    }
    Ok(files)
}

pub fn get_file_based_test_configuration_description(config: &TestConfiguration) -> String {
    let mut entries = config.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(key, _)| key.as_str());
    entries
        .into_iter()
        .map(|(key, value)| format!("{key}={}", value.to_ascii_lowercase()))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn get_file_based_test_configurations(
    settings: &HashMap<String, String>,
    vary_by_options: &HashMap<String, ()>,
) -> Vec<NamedTestConfiguration> {
    let mut option_entries = Vec::new();
    let mut variation_count = 1;
    let mut non_varying_options = TestConfiguration::new();
    for (option, value) in settings {
        if vary_by_options.contains_key(&option.to_ascii_lowercase()) {
            let values = split_option_values(value, option);
            if values.len() > 1 {
                variation_count *= values.len();
                if variation_count > 25 {
                    panic!("Provided test options exceeded the maximum number of variations");
                }
                let mut entries = Vec::with_capacity(values.len() + 1);
                entries.push(option.clone());
                entries.extend(values);
                option_entries.push(entries);
            } else if values.len() == 1 {
                non_varying_options.insert(option.clone(), values[0].clone());
            }
        } else {
            non_varying_options.insert(option.clone(), value.clone());
        }
    }

    if !option_entries.is_empty() {
        option_entries.sort_by(|a, b| a[0].cmp(&b[0]));
        let variations =
            compute_file_based_test_configuration_variations(variation_count, &option_entries);
        return variations
            .into_iter()
            .map(|mut variation| {
                let name = get_file_based_test_configuration_description(&variation);
                variation.extend(non_varying_options.clone());
                NamedTestConfiguration {
                    name,
                    config: variation,
                }
            })
            .collect();
    }

    if !non_varying_options.is_empty() {
        return vec![NamedTestConfiguration {
            name: String::new(),
            config: non_varying_options,
        }];
    }

    Vec::new()
}

fn is_removed_target(target: &str) -> bool {
    matches!(target.to_ascii_lowercase().as_str(), "es3" | "es5")
}

pub fn split_option_values(value: &str, option: &str) -> Vec<String> {
    let mut star = false;
    let mut includes = Vec::new();
    let mut excludes = Vec::new();

    for entry in value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        if entry == "*" {
            star = true;
        } else if let Some(exclude) = entry.strip_prefix('-').or_else(|| entry.strip_prefix('!')) {
            excludes.push(exclude.to_string());
        } else {
            includes.push(entry.trim_start_matches('+').to_string());
        }
    }

    if includes.is_empty() && !star && excludes.is_empty() {
        return Vec::new();
    }

    let mut variations = Vec::<(CompilerOptionsValue, String)>::new();
    for include in includes {
        let parsed = get_value_of_option_string(option, &include);
        if !variations.iter().any(|(existing, _)| *existing == parsed) {
            variations.push((parsed, include));
        }
    }

    if star {
        for include in get_all_values_for_option(option) {
            let parsed = get_value_of_option_string(option, &include);
            if !variations.iter().any(|(existing, _)| *existing == parsed) {
                variations.push((parsed, include));
            }
        }
    }

    for exclude in excludes {
        if let Some(parsed) = try_get_value_of_option_string(option, &exclude) {
            variations.retain(|(value, _)| *value != parsed);
        }
    }

    if variations.is_empty() {
        panic!("Variations in test option '@{option}' resulted in an empty set.");
    }
    variations.into_iter().map(|(_, value)| value).collect()
}

fn split_comma_list(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    value
        .split(',')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_list_type_option(
    option: &CommandLineOption,
    value: &str,
    current_directory: &str,
) -> Vec<String> {
    let value = value.trim();
    if value.starts_with('-') || value.is_empty() {
        return Vec::new();
    }

    let element = option
        .elements()
        .unwrap_or_else(|| panic!("Compiler option '{}' should have elements.", option.name));
    value
        .split(',')
        .filter_map(|value| {
            let value = value.trim();
            if value.is_empty() {
                return None;
            }
            match element.kind {
                Some(CommandLineOptionKind::String) => {
                    if element.is_file_path {
                        Some(normalize_absolute_path(value, current_directory))
                    } else {
                        Some(value.to_string())
                    }
                }
                Some(CommandLineOptionKind::Enum) => match get_option_value(&element, value) {
                    CompilerOptionsValue::String(value) if !value.is_empty() => Some(value),
                    CompilerOptionsValue::Bool(_)
                    | CompilerOptionsValue::Number(_)
                    | CompilerOptionsValue::String(_)
                    | CompilerOptionsValue::Unknown => None,
                },
                Some(
                    CommandLineOptionKind::Boolean
                    | CommandLineOptionKind::Object
                    | CommandLineOptionKind::Number,
                ) => panic!(
                    "List of {} is not yet supported.",
                    element.kind.unwrap().as_str()
                ),
                Some(CommandLineOptionKind::List | CommandLineOptionKind::ListOrElement) | None => {
                    None
                }
            }
        })
        .collect()
}

pub fn get_value_of_option_string(option: &str, value: &str) -> CompilerOptionsValue {
    try_get_value_of_option_string(option, value)
        .unwrap_or_else(|| panic!("Unknown value '{value}' for option '{option}'"))
}

pub fn try_get_value_of_option_string(option: &str, value: &str) -> Option<CompilerOptionsValue> {
    let option_decl = get_command_line_option(option)?;
    match option_decl.kind {
        Some(CommandLineOptionKind::Enum) => option_decl
            .enum_map()?
            .get(&value.to_ascii_lowercase())
            .cloned(),
        Some(CommandLineOptionKind::Boolean) => match value.to_ascii_lowercase().as_str() {
            "true" => Some(CompilerOptionsValue::Bool(true)),
            "false" => Some(CompilerOptionsValue::Bool(false)),
            _ => None,
        },
        Some(CommandLineOptionKind::Number) => {
            value.parse::<i32>().ok().map(CompilerOptionsValue::Number)
        }
        _ => Some(CompilerOptionsValue::String(value.to_string())),
    }
}

fn get_option_value(option: &CommandLineOption, value: &str) -> CompilerOptionsValue {
    match option.kind {
        Some(CommandLineOptionKind::Enum) => option
            .enum_map()
            .and_then(|map| map.get(&value.to_ascii_lowercase()).cloned())
            .unwrap_or_else(|| panic!("Unknown value '{value}' for option '{}'", option.name)),
        _ => CompilerOptionsValue::String(value.to_string()),
    }
}

pub fn get_command_line_option(option: &str) -> Option<CommandLineOption> {
    compiler_option_declarations()
        .into_iter()
        .find(|option_decl| option_decl.name.eq_ignore_ascii_case(option))
}

pub fn get_all_values_for_option(option: &str) -> Vec<String> {
    let Some(option_decl) = get_command_line_option(option) else {
        return Vec::new();
    };
    match option_decl.kind {
        Some(CommandLineOptionKind::Enum) => {
            ts_tsoptions::enum_keys(&option_decl.name).unwrap_or_default()
        }
        Some(CommandLineOptionKind::Boolean) => vec!["true".to_string(), "false".to_string()],
        _ => Vec::new(),
    }
}

fn compiler_option_declarations() -> Vec<CommandLineOption> {
    let mut options = ts_tsoptions::options_declarations().to_vec();
    options.extend([
        CommandLineOption::new("allowNonTsExtensions", CommandLineOptionKind::Boolean),
        CommandLineOption::new("noErrorTruncation", CommandLineOptionKind::Boolean),
        CommandLineOption::new("suppressOutputPathCheck", CommandLineOptionKind::Boolean),
        CommandLineOption::new("noCheck", CommandLineOptionKind::Boolean),
    ]);
    options
}

pub fn compute_file_based_test_configuration_variations(
    variation_count: usize,
    option_entries: &[Vec<String>],
) -> Vec<TestConfiguration> {
    let mut variations = Vec::with_capacity(variation_count);
    compute_file_based_test_configuration_variations_worker(
        variation_count,
        option_entries,
        0,
        TestConfiguration::new(),
        &mut variations,
    );
    variations
}

pub fn compute_file_based_test_configuration_variations_worker(
    variation_count: usize,
    option_entries: &[Vec<String>],
    index: usize,
    current: TestConfiguration,
    out: &mut Vec<TestConfiguration>,
) {
    if index >= option_entries.len() {
        out.push(current);
        return;
    }
    if let Some(entries) = option_entries.get(index) {
        let Some((option_key, entries)) = entries.split_first() else {
            return;
        };
        for entry in entries {
            let mut next = current.clone();
            next.insert(option_key.clone(), entry.clone());
            compute_file_based_test_configuration_variations_worker(
                variation_count,
                option_entries,
                index + 1,
                next,
                out,
            );
        }
    }
}

pub fn get_config_name_from_file_name(filename: &str) -> String {
    let basename = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let basename_lower = basename.to_ascii_lowercase();
    if basename_lower == "tsconfig.json" || basename_lower == "jsconfig.json" {
        basename_lower
    } else {
        String::new()
    }
}

pub fn skip_unsupported_compiler_options(options: &CompilerOptions) -> Option<String> {
    match options.module.to_ascii_lowercase().as_str() {
        "amd" | "umd" | "system" => {
            return Some(format!("unsupported module kind {}", options.module));
        }
        _ => {}
    }
    match options.module_resolution.to_ascii_lowercase().as_str() {
        "node10" | "node" | "classic" => {
            return Some(format!(
                "unsupported module resolution kind {}",
                options.module_resolution
            ));
        }
        _ => {}
    }
    if options.es_module_interop_is_false {
        return Some("esModuleInterop=false is unsupported".to_string());
    }
    if options.allow_synthetic_default_imports_is_false {
        return Some("allowSyntheticDefaultImports=false is unsupported".to_string());
    }
    if !options.base_url.is_empty() {
        return Some(format!("unsupported baseUrl {}", options.base_url));
    }
    if !options.out_file.is_empty() {
        return Some(format!("unsupported outFile {}", options.out_file));
    }
    if is_removed_target(&options.target) {
        return Some(format!("unsupported target {}", options.target));
    }
    if options.always_strict_is_false {
        return Some("alwaysStrict=false is unsupported".to_string());
    }
    None
}

fn normalize_absolute_path(file_name: &str, current_directory: &str) -> String {
    let path = if file_name.starts_with('/') || is_windows_rooted(file_name) {
        file_name.replace('\\', "/")
    } else {
        PathBuf::from(current_directory)
            .join(file_name)
            .to_string_lossy()
            .replace('\\', "/")
    };
    tspath::normalize_path(&path)
}

fn parse_bool(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1")
}

fn is_windows_rooted(path: &str) -> bool {
    path.len() > 2 && path.as_bytes()[1] == b':'
}

#[allow(dead_code)]
fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn harness_map_file(text: &str) -> vfs::vfstest::MapFile {
        vfs::vfstest::IntoMapFile::into_map_file(text, SystemTime::UNIX_EPOCH)
    }

    fn harness_fs_overlay_for_test(
        mut upper_files: BTreeMap<String, vfs::vfstest::MapFile>,
        lower_files: BTreeMap<String, vfs::vfstest::MapFile>,
        use_case_sensitive_file_names: bool,
        upper_symlink_targets: BTreeMap<String, String>,
    ) -> (HarnessFs, vfs::vfstest::MapFs) {
        prepare_upper_files_for_test_lib_overlay(
            &mut upper_files,
            &lower_files,
            use_case_sensitive_file_names,
        );
        let upper = vfs::vfstest::from_map(upper_files, use_case_sensitive_file_names);
        let lower = vfs::vfstest::from_map(lower_files, use_case_sensitive_file_names);
        (
            HarnessFs {
                upper,
                lower: Some(lower.clone()),
                upper_symlink_targets: Arc::new(upper_symlink_targets),
                hidden_lower_paths: Arc::new(RwLock::new(BTreeSet::new())),
            },
            lower,
        )
    }

    #[test]
    fn test_lib_folder_map_is_loaded_once() {
        if ts_repo::skip_if_no_type_script_submodule() {
            return;
        }

        let first = test_lib_folder_map();
        let second = test_lib_folder_map();

        assert!(
            std::ptr::eq(first, second),
            "test lib folder map should be cached for the process"
        );
        assert!(
            first.contains_key("/.lib/react.d.ts"),
            "cached test lib folder should include vendored test libs"
        );
        assert!(
            first.contains_key("/.lib/react18/react18.d.ts"),
            "cached test lib folder should preserve nested lib paths"
        );
    }

    #[test]
    fn source_file_cache_should_be_shared_only_for_immutable_inputs() {
        let (fs, _) = harness_fs_overlay_for_test(
            BTreeMap::from([
                ("/src/main.ts".to_string(), harness_map_file("let x = 1;")),
                (
                    "/.lib/local.d.ts".to_string(),
                    harness_map_file("declare const Local: unknown;"),
                ),
                ("/libs".to_string(), vfs::vfstest::symlink("/.lib")),
            ]),
            BTreeMap::from([
                (
                    "/.lib/react.d.ts".to_string(),
                    harness_map_file("declare const React: unknown;"),
                ),
                (
                    "/.lib/react18/react18.d.ts".to_string(),
                    harness_map_file("declare const React18: unknown;"),
                ),
            ]),
            true,
            BTreeMap::from([("/libs".to_string(), "/.lib".to_string())]),
        );

        let bundled_lib = "bundled:///libs/lib.es5.d.ts";
        assert_eq!(
            should_use_shared_source_file_cache(bundled_lib, &fs),
            ts_bundled::is_bundled(bundled_lib),
            "embedded bundled libs should use the process cache"
        );
        assert!(
            should_use_shared_source_file_cache("/.lib/react.d.ts", &fs),
            "harness lower-overlay libs should use the process cache"
        );
        assert!(
            should_use_shared_source_file_cache("/libs/react18/react18.d.ts", &fs),
            "harness symlinks to lower-overlay libs should use the process cache"
        );
        assert!(
            should_use_shared_source_file_cache("/.lib/react18/react18.d.ts", &fs),
            "nested harness lower-overlay libs should use the process cache"
        );
        assert!(
            !should_use_shared_source_file_cache("/.lib/local.d.ts", &fs),
            "test-owned files under the lib prefix should use the host-local cache"
        );
        assert!(
            !should_use_shared_source_file_cache("/.src/tests/cases/compiler/a.ts", &fs),
            "ordinary test files should use the host-local cache"
        );
        assert!(
            !should_use_shared_source_file_cache("/src/main.ts", &fs),
            "non-lib harness files should use the host-local cache"
        );
    }

    #[test]
    fn compile_files_should_not_store_ordinary_files_in_shared_source_file_cache() {
        let file = TestFile {
            unit_name: "/src/cacheScopeOrdinary.ts".to_string(),
            content: "let cacheScopeOrdinary = 1;".to_string(),
        };
        let result = compile_files(
            std::slice::from_ref(&file),
            &[],
            HashMap::from([("noLib".to_string(), "true".to_string())]),
            None,
            "/",
            HashMap::new(),
        );
        let program = result.program.as_ref().expect("program should be created");
        let source_file = program
            .0
            .get_source_file(&file.unit_name)
            .expect("ordinary source file should be parsed");
        let script_kind = core::get_script_kind_from_file_name(&source_file.file_name());
        let key =
            get_source_file_cache_key(source_file.parse_options(), source_file.text(), script_kind);

        assert!(
            !SHARED_SOURCE_FILE_CACHE.load(&key).1,
            "ordinary source file should be cached only by the compiler host that parsed it"
        );
    }

    #[test]
    fn compile_files_should_store_test_lib_files_in_shared_source_file_cache() {
        if ts_repo::skip_if_no_type_script_submodule() {
            return;
        }

        let result = compile_files_ex(
            &[TestFile {
                unit_name: "/src/main.ts".to_string(),
                content: "let x = 1;".to_string(),
            }],
            &[],
            &HarnessOptions {
                use_case_sensitive_file_names: true,
                lib_files: vec!["react.d.ts".to_string()],
                ..HarnessOptions::default()
            },
            &CompilerOptions {
                no_lib: Some(true),
                ..CompilerOptions::default()
            },
            "/",
            HashMap::new(),
            None,
        );
        let program = result.program.as_ref().expect("program should be created");
        let source_file = program
            .0
            .get_source_file("/.lib/react.d.ts")
            .expect("requested test lib file should be parsed");
        let script_kind = core::get_script_kind_from_file_name(&source_file.file_name());
        let key =
            get_source_file_cache_key(source_file.parse_options(), source_file.text(), script_kind);

        assert!(
            SHARED_SOURCE_FILE_CACHE.load(&key).1,
            "requested harness test lib should be cached in the shared process cache"
        );
    }

    #[test]
    fn harness_fs_overlay_should_match_lib_override_and_share_lower_files() {
        let (fs, lower) = harness_fs_overlay_for_test(
            BTreeMap::from([
                ("/src/main.ts".to_string(), harness_map_file("let x = 1;")),
                ("/libs".to_string(), vfs::vfstest::symlink("/.lib")),
                ("/.lib/shadowed.d.ts".to_string(), harness_map_file("upper")),
            ]),
            BTreeMap::from([
                (
                    "/.lib/react.d.ts".to_string(),
                    harness_map_file("declare const React: unknown;"),
                ),
                (
                    "/.lib/nested/lib.d.ts".to_string(),
                    harness_map_file("declare const Nested: unknown;"),
                ),
                ("/.lib/shadowed.d.ts".to_string(), harness_map_file("lower")),
            ]),
            true,
            BTreeMap::from([("/libs".to_string(), "/.lib".to_string())]),
        );

        assert_eq!(
            fs.read_file("/.lib/react.d.ts"),
            ("declare const React: unknown;".to_string(), true)
        );
        assert_eq!(
            fs.read_file("/.lib/shadowed.d.ts"),
            ("lower".to_string(), true)
        );
        assert!(
            fs.get_accessible_entries("/.lib")
                .directories
                .contains(&"nested".to_string())
        );
        assert_eq!(
            fs.read_file("/libs/nested/lib.d.ts"),
            ("declare const Nested: unknown;".to_string(), true)
        );

        fs.remove("/.lib/react.d.ts").unwrap();
        assert!(!fs.file_exists("/.lib/react.d.ts"));
        assert_eq!(
            lower.read_file("/.lib/react.d.ts"),
            ("declare const React: unknown;".to_string(), true)
        );
    }

    #[test]
    fn harness_fs_overlay_should_match_symlink_write_append_and_remove() {
        let (fs, lower) = harness_fs_overlay_for_test(
            BTreeMap::from([("/libs".to_string(), vfs::vfstest::symlink("/.lib"))]),
            BTreeMap::from([("/.lib/react.d.ts".to_string(), harness_map_file("lower"))]),
            true,
            BTreeMap::from([("/libs".to_string(), "/.lib".to_string())]),
        );

        assert!(
            fs.chtimes(
                "/libs/react.d.ts",
                SystemTime::UNIX_EPOCH,
                SystemTime::UNIX_EPOCH
            )
            .is_err()
        );
        assert_eq!(
            fs.read_file("/libs/react.d.ts"),
            ("lower".to_string(), true)
        );

        fs.write_file("/libs/react.d.ts", "written").unwrap();
        assert_eq!(
            fs.read_file("/.lib/react.d.ts"),
            ("written".to_string(), true)
        );
        assert_eq!(
            lower.read_file("/.lib/react.d.ts"),
            ("lower".to_string(), true)
        );

        fs.append_file("/libs/react.d.ts", " appended").unwrap();
        assert_eq!(
            fs.read_file("/libs/react.d.ts"),
            ("written appended".to_string(), true)
        );
        assert_eq!(fs.realpath("/.lib/react.d.ts"), "/libs/react.d.ts");

        fs.remove("/libs/react.d.ts").unwrap();
        assert_eq!(
            fs.read_file("/.lib/react.d.ts"),
            ("written appended".to_string(), true)
        );

        fs.remove("/.lib/react.d.ts").unwrap();
        assert!(!fs.file_exists("/.lib/react.d.ts"));
        assert_eq!(
            lower.read_file("/.lib/react.d.ts"),
            ("lower".to_string(), true)
        );
    }

    #[test]
    fn harness_fs_overlay_should_detect_case_insensitive_upper_lower_collision() {
        let result = std::panic::catch_unwind(|| {
            let _ = harness_fs_overlay_for_test(
                BTreeMap::from([("/.LIB/react.d.ts".to_string(), harness_map_file("upper"))]),
                BTreeMap::from([("/.lib/react.d.ts".to_string(), harness_map_file("lower"))]),
                false,
                BTreeMap::new(),
            );
        });

        assert!(result.is_err());
    }

    #[test]
    fn compile_files_includes_requested_test_lib_files() {
        if ts_repo::skip_if_no_type_script_submodule() {
            return;
        }

        let result = compile_files_ex(
            &[TestFile {
                unit_name: "/src/main.ts".to_string(),
                content: "let x = 1;".to_string(),
            }],
            &[],
            &HarnessOptions {
                use_case_sensitive_file_names: true,
                lib_files: vec!["react.d.ts".to_string()],
                ..HarnessOptions::default()
            },
            &CompilerOptions {
                no_lib: Some(true),
                ..CompilerOptions::default()
            },
            "/",
            HashMap::new(),
            None,
        );

        let program = result.program.as_ref().expect("program should be created");
        assert!(
            program.0.get_source_file("/.lib/react.d.ts").is_some(),
            "requested @libFiles entry should be added to the program from the cached test lib map"
        );
    }

    #[test]
    fn compile_files_reports_incremental_without_config_or_build_info() {
        let result = compile_files_ex(
            &[TestFile {
                unit_name: "/src/main.ts".to_string(),
                content: "let x = 1;".to_string(),
            }],
            &[],
            &HarnessOptions {
                use_case_sensitive_file_names: true,
                ..HarnessOptions::default()
            },
            &CompilerOptions {
                incremental: Some(true),
                target: "es2015".to_string(),
                ..CompilerOptions::default()
            },
            "/",
            HashMap::new(),
            None,
        );

        let program = result.program.as_ref().expect("program should be created");
        let options = program.0.options();
        assert!(
            options.incremental.is_true(),
            "incremental marker should reach core compiler options: incremental={:?}, configFilePath={:?}, tsBuildInfoFile={:?}",
            options.incremental,
            options.config_file_path,
            options.ts_build_info_file
        );
        assert!(
            options.config_file_path.is_empty(),
            "raw file test should not have configFilePath: {:?}",
            options.config_file_path
        );
        assert!(
            options.ts_build_info_file.is_empty(),
            "raw file test should not have tsBuildInfoFile: {:?}",
            options.ts_build_info_file
        );
        let program_diagnostics = program.0.get_program_diagnostics();
        assert!(
            program_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code() == 5074),
            "program diagnostics should contain TS5074: {:?}",
            program_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code())
                .collect::<Vec<_>>()
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code() == 5074),
            "incremental without tsconfig, outFile, or tsBuildInfoFile should report TS5074"
        );
    }

    #[test]
    fn compile_files_allows_incremental_with_build_info_file() {
        let result = compile_files_ex(
            &[TestFile {
                unit_name: "/src/main.ts".to_string(),
                content: "let x = 1;".to_string(),
            }],
            &[],
            &HarnessOptions {
                use_case_sensitive_file_names: true,
                ..HarnessOptions::default()
            },
            &CompilerOptions {
                incremental: Some(true),
                target: "es2015".to_string(),
                ts_build_info_file: "/src/main.tsbuildinfo".to_string(),
                ..CompilerOptions::default()
            },
            "/",
            HashMap::new(),
            None,
        );

        assert!(
            result
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code() != 5074),
            "explicit tsBuildInfoFile should satisfy incremental validation"
        );
    }

    #[test]
    fn compile_files_preserves_ignore_deprecations() {
        let result = compile_files_ex(
            &[TestFile {
                unit_name: "/src/main.ts".to_string(),
                content: r#"import data from "./data.json" assert { type: "json" };"#.to_string(),
            }],
            &[TestFile {
                unit_name: "/src/data.json".to_string(),
                content: "{}".to_string(),
            }],
            &HarnessOptions {
                use_case_sensitive_file_names: true,
                ..HarnessOptions::default()
            },
            &CompilerOptions {
                module: "esnext".to_string(),
                target: "esnext".to_string(),
                ignore_deprecations: "6.0".to_string(),
                ..CompilerOptions::default()
            },
            "/",
            HashMap::new(),
            None,
        );

        let program = result.program.as_ref().expect("program should be created");
        assert_eq!(program.0.options().ignore_deprecations, "6.0");
    }
}
