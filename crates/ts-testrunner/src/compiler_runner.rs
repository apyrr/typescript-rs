use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Once, OnceLock};

use crate::{
    Runner,
    test_case_parser::{extract_compiler_settings, make_units_from_test},
};
use ts_testutil::{baseline, harnessutil, tsbaseline};
use ts_tspath as tspath;

pub const REQUIRE_STR: &str = "require(";
pub const SRC_FOLDER: &str = "/.src";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerTestType {
    Conformance,
    Regression,
}

impl CompilerTestType {
    pub fn string(&self) -> &'static str {
        match self {
            CompilerTestType::Regression => "compiler",
            CompilerTestType::Conformance => "conformance",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerBaselineRunner {
    pub is_submodule: bool,
    pub test_files: Vec<String>,
    pub base_path: String,
    pub test_suit_name: String,
    pub clean_up_local_before_run: bool,
}

pub fn new_compiler_baseline_runner(
    test_type: CompilerTestType,
    is_submodule: bool,
) -> CompilerBaselineRunner {
    let test_suit_name = test_type.string().to_string();
    let base_path = ts_repo::type_script_submodule_path()
        .join("tests")
        .join("cases")
        .join(&test_suit_name)
        .to_string_lossy()
        .replace('\\', "/");
    CompilerBaselineRunner {
        base_path,
        test_suit_name,
        is_submodule,
        test_files: Vec::new(),
        clean_up_local_before_run: true,
    }
}

impl Runner for CompilerBaselineRunner {
    fn enumerate_test_files(&self) -> Vec<String> {
        if !self.test_files.is_empty() {
            return self.test_files.clone();
        }
        enumerate_compiler_test_files(&self.base_path).unwrap_or_default()
    }

    fn run_tests(&self, t: &mut dyn crate::TestContext) {
        t.helper();
        if self.clean_up_local_before_run {
            self.clean_up_local();
        }
        let test_files = self.enumerate_test_files();
        let total = test_files.len();
        for (index, filename) in test_files.iter().enumerate() {
            log_compiler_test_progress(&format!(
                "[ts_testrunner] {}/{} {}: {}",
                index + 1,
                total,
                self.test_suit_name,
                filename
            ));
            self.run_test(filename);
        }
    }
}

impl CompilerBaselineRunner {
    pub fn clean_up_local(&self) {
        let mut local_path = local_base_path();
        if self.is_submodule {
            local_path.push("diff");
        }
        local_path.push(&self.test_suit_name);
        fs::remove_dir_all(&local_path).unwrap_or_else(|err| {
            if err.kind() != std::io::ErrorKind::NotFound {
                panic!("Could not clean up local compiler tests: {err}");
            }
        });
    }

    fn run_test(&self, filename: &str) {
        let test = read_compiler_file_based_test(filename);
        if !test.configurations.is_empty() {
            for config in &test.configurations {
                let mut test_name = test.basename.clone();
                if !config.name.is_empty() {
                    test_name.push(' ');
                    test_name.push_str(&config.name);
                }
                self.run_single_config_test(test_name, &test, Some(config));
            }
        } else {
            self.run_single_config_test(test.basename.clone(), &test, None);
        }
    }

    fn run_single_config_test(
        &self,
        test_name: String,
        test: &CompilerFileBasedTestData,
        config: Option<&NamedTestConfiguration>,
    ) {
        log_compiler_test_progress(&format!(
            "[ts_testrunner] running {} config: {}",
            self.test_suit_name, test_name
        ));
        let _link_store_stats = LinkStoreStatsReporter::new(&self.test_suit_name, &test_name);
        let compiler_test = new_compiler_test_from_file_based(test_name, test, config);
        self.run_compiler_test(compiler_test);
    }

    fn verify_phase(&self, phase: &str, verify: impl FnOnce()) {
        log_compiler_test_progress(&format!(
            "[ts_testrunner] phase {}: {}",
            self.test_suit_name, phase
        ));
        verify();
    }

    pub fn run_test_config(&self, filename: &str, config_index: Option<usize>) {
        let test = read_compiler_file_based_test(filename);

        match (config_index, test.configurations.is_empty()) {
            (None, true) => {
                let test_name = test.basename.clone();
                let _link_store_stats =
                    LinkStoreStatsReporter::new(&self.test_suit_name, &test_name);
                let compiler_test = new_compiler_test_from_file_based(test_name, &test, None);
                self.run_compiler_test(compiler_test);
            }
            (Some(index), false) => {
                let Some(config) = test.configurations.get(index) else {
                    panic!(
                        "generated config index {index} is out of range for {}",
                        test.filename
                    );
                };
                let mut test_name = test.basename.clone();
                if !config.name.is_empty() {
                    test_name.push(' ');
                    test_name.push_str(&config.name);
                }
                let _link_store_stats =
                    LinkStoreStatsReporter::new(&self.test_suit_name, &test_name);
                let compiler_test =
                    new_compiler_test_from_file_based(test_name, &test, Some(config));
                self.run_compiler_test(compiler_test);
            }
            (None, false) => {
                panic!(
                    "generated file-level test for configured test {}",
                    test.filename
                );
            }
            (Some(index), true) => {
                panic!(
                    "generated config index {index} for unconfigured test {}",
                    test.filename
                );
            }
        }
    }

    fn run_compiler_test(&self, compiler_test: CompilerTest) {
        let CompilerTest {
            test_name,
            filename,
            basename,
            configured_name,
            options,
            harness_options,
            result,
            tsconfig_files,
            to_be_compiled,
            other_files,
            has_non_dts_files,
        } = compiler_test;
        let common = CompilerTestCommon {
            test_name,
            filename,
            basename,
            configured_name,
            options,
            harness_options,
            tsconfig_files,
            to_be_compiled,
            other_files,
            has_non_dts_files,
        };

        if let Some(reason) = harnessutil::skip_unsupported_compiler_options(&common.options) {
            println!(
                "[ts_testrunner] skip {} config {}: {}",
                self.test_suit_name, common.test_name, reason
            );
            return;
        }

        let (
            program,
            diagnostics_failed,
            trace,
            union_type_ordering_checks,
            source_file_parent_pointer_checks,
        ) = {
            let CompilationResult {
                program,
                emit_result,
                diagnostics,
                trace,
                union_type_ordering_checks,
                source_file_parent_pointer_checks,
                symlinks,
                js,
                dts,
                maps,
                outputs,
                inputs,
                inputs_and_outputs,
                ..
            } = result;

            self.verify_phase("diagnostics", || {
                common.verify_diagnostics(&diagnostics, &self.test_suit_name, self.is_submodule)
            });
            self.verify_phase("javascript output", || {
                common.verify_javascript_output(
                    &diagnostics,
                    program.as_ref(),
                    &symlinks,
                    &js,
                    &dts,
                    &inputs_and_outputs,
                    &self.test_suit_name,
                    self.is_submodule,
                )
            });
            self.verify_phase("sourcemap output", || {
                common.verify_source_map_output(
                    &diagnostics,
                    &maps,
                    &js,
                    &outputs,
                    &inputs,
                    &self.test_suit_name,
                    self.is_submodule,
                )
            });
            self.verify_phase("sourcemap record", || {
                common.verify_source_map_record(
                    &diagnostics,
                    emit_result.as_ref(),
                    program.as_ref(),
                    &js,
                    &dts,
                    &self.test_suit_name,
                    self.is_submodule,
                )
            });
            (
                program,
                !diagnostics.is_empty(),
                trace,
                union_type_ordering_checks,
                source_file_parent_pointer_checks,
            )
        };
        self.verify_phase("types and symbols", || {
            common.verify_types_and_symbols(
                program,
                diagnostics_failed,
                &self.test_suit_name,
                self.is_submodule,
            )
        });
        self.verify_phase("module resolution", || {
            common.verify_module_resolution(&trace, &self.test_suit_name, self.is_submodule)
        });
        self.verify_phase("union ordering", || {
            harnessutil::verify_union_ordering_checks(&union_type_ordering_checks)
        });
        self.verify_phase("parent pointers", || {
            harnessutil::verify_parent_pointer_checks(&source_file_parent_pointer_checks)
        });
    }
}

fn log_compiler_test_progress(message: &str) {
    if std::env::var_os("TSGO_TEST_PROGRESS").is_none() {
        return;
    }
    println!("{message}");
    let _ = io::stdout().flush();
}

static LINK_STORE_STATS_UNAVAILABLE_NOTICE: Once = Once::new();

struct LinkStoreStatsReporter {
    label: String,
    enabled: bool,
}

impl LinkStoreStatsReporter {
    fn new(suite_name: &str, test_name: &str) -> Self {
        if std::env::var_os("TSGO_LINKSTORE_STATS").is_none() {
            return Self {
                label: String::new(),
                enabled: false,
            };
        }
        if !ts_core::link_store_stats_available() {
            LINK_STORE_STATS_UNAVAILABLE_NOTICE.call_once(|| {
                println!(
                    "[ts_linkstore] TSGO_LINKSTORE_STATS requested, but the link_store_stats feature is disabled"
                );
                let _ = io::stdout().flush();
            });
            return Self {
                label: String::new(),
                enabled: false,
            };
        }

        ts_core::reset_link_store_stats();
        ts_core::set_link_store_stats_enabled(true);
        Self {
            label: format!("{suite_name} {test_name}"),
            enabled: true,
        }
    }
}

impl Drop for LinkStoreStatsReporter {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }

        ts_core::set_link_store_stats_enabled(false);
        let stats = ts_core::link_store_stats_snapshot();
        println!(
            "[ts_linkstore] {}: ensure_handle={} hit={} miss={} with_by_handle={} with_by_handle_mut={}",
            self.label,
            stats.ensure_handle,
            stats.ensure_handle_hit,
            stats.ensure_handle_miss,
            stats.with_by_handle,
            stats.with_by_handle_mut,
        );
        let _ = io::stdout().flush();
    }
}

fn local_base_path() -> PathBuf {
    ts_repo::baseline_output_path().join("local")
}

pub fn skipped_emit_tests() -> HashMap<&'static str, &'static str> {
    [
        (
            "filesEmittingIntoSameOutput.ts",
            "Output order nondeterministic due to collision on filename during parallel emit.",
        ),
        (
            "jsFileCompilationWithJsEmitPathSameAsInput.ts",
            "Output order nondeterministic due to collision on filename during parallel emit.",
        ),
        (
            "grammarErrors.ts",
            "Output order nondeterministic due to collision on filename during parallel emit.",
        ),
        (
            "jsFileCompilationEmitBlockedCorrectly.ts",
            "Output order nondeterministic due to collision on filename during parallel emit.",
        ),
        (
            "jsDeclarationsReexportAliasesEsModuleInterop.ts",
            "cls.d.ts is missing statements when run concurrently.",
        ),
        (
            "jsFileCompilationWithoutJsExtensions.ts",
            "No files are emitted.",
        ),
        (
            "typeOnlyMerge2.ts",
            "Nondeterministic contents when run concurrently.",
        ),
        (
            "typeOnlyMerge3.ts",
            "Nondeterministic contents when run concurrently.",
        ),
    ]
    .into_iter()
    .collect()
}

static COMPILER_VARY_BY_MAP: OnceLock<HashMap<String, ()>> = OnceLock::new();

pub fn get_compiler_vary_by_map() -> HashMap<String, ()> {
    compiler_vary_by_map().clone()
}

fn compiler_vary_by_map() -> &'static HashMap<String, ()> {
    COMPILER_VARY_BY_MAP.get_or_init(build_compiler_vary_by_map)
}

fn build_compiler_vary_by_map() -> HashMap<String, ()> {
    let mut vary_by_map = HashMap::new();
    for option in ts_tsoptions::options_declarations() {
        let is_boolean_or_enum = matches!(
            option.kind,
            Some(
                ts_tsoptions::CommandLineOptionKind::Boolean
                    | ts_tsoptions::CommandLineOptionKind::Enum
            )
        );
        if !option.is_command_line_only
            && is_boolean_or_enum
            && (option.affects_program_structure
                || option.affects_emit
                || option.affects_module_resolution
                || option.affects_bind_diagnostics
                || option.affects_semantic_diagnostics
                || option.affects_source_file
                || option.affects_declaration_path
                || option.affects_build_info)
        {
            vary_by_map.insert(option.name.to_ascii_lowercase(), ());
        }
    }
    vary_by_map.insert("noemit".to_string(), ());
    vary_by_map.insert("isolatedmodules".to_string(), ());
    vary_by_map
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFileBasedTest {
    pub filename: String,
    pub content: String,
    pub configurations: Vec<NamedTestConfiguration>,
}

#[derive(Debug, Eq, PartialEq)]
struct CompilerFileBasedTestData {
    filename: String,
    basename: String,
    content: String,
    payload: TestCaseContent,
    configurations: Vec<NamedTestConfiguration>,
}

pub type NamedTestConfiguration = harnessutil::NamedTestConfiguration;
pub type TestConfiguration = harnessutil::TestConfiguration;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestUnit {
    pub name: String,
    pub content: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestCaseContent {
    pub test_unit_data: Vec<TestUnit>,
    pub symlinks: HashMap<String, String>,
    pub tsconfig: Option<ParsedCommandLine>,
    pub tsconfig_file_unit_data: Option<TestUnit>,
}

pub type ParsedCommandLine = harnessutil::ParsedCommandLine;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestCaseContentWithConfig {
    pub test_case_content: TestCaseContent,
    pub configuration: TestConfiguration,
}

#[derive(Clone)]
pub struct CompilerTest {
    pub test_name: String,
    pub filename: String,
    pub basename: String,
    pub configured_name: String,
    pub options: CompilerOptions,
    pub harness_options: HarnessOptions,
    pub result: CompilationResult,
    pub tsconfig_files: Vec<TestFile>,
    pub to_be_compiled: Vec<TestFile>,
    pub other_files: Vec<TestFile>,
    pub has_non_dts_files: bool,
}

struct CompilerTestCommon {
    test_name: String,
    filename: String,
    basename: String,
    configured_name: String,
    options: CompilerOptions,
    harness_options: HarnessOptions,
    tsconfig_files: Vec<TestFile>,
    to_be_compiled: Vec<TestFile>,
    other_files: Vec<TestFile>,
    has_non_dts_files: bool,
}

pub type CompilerOptions = harnessutil::CompilerOptions;
pub type HarnessOptions = harnessutil::HarnessOptions;
pub type CompilationResult = harnessutil::CompilationResult;
pub type TestFile = harnessutil::TestFile;

pub fn get_compiler_file_based_test(filename: &str) -> CompilerFileBasedTest {
    let test = read_compiler_file_based_test(filename);
    CompilerFileBasedTest {
        filename: test.filename.clone(),
        content: test.content.clone(),
        configurations: test.configurations.clone(),
    }
}

fn read_compiler_file_based_test(filename: &str) -> CompilerFileBasedTestData {
    let filename = filename.replace('\\', "/");
    let bytes = fs::read(&filename)
        .unwrap_or_else(|err| panic!("Could not read test file: {filename}: {err}"));
    let content = ts_vfs::internal::decode_bytes(&bytes)
        .unwrap_or_else(|| panic!("Could not decode test file: {filename}"));
    let settings = extract_compiler_settings(&content);
    let configurations =
        harnessutil::get_file_based_test_configurations(&settings, compiler_vary_by_map());
    let payload = make_units_from_test(&content, &filename);
    CompilerFileBasedTestData {
        basename: get_base_file_name(&filename),
        filename,
        content,
        payload,
        configurations,
    }
}

pub fn new_compiler_test(
    test_name: String,
    filename: String,
    test_content: TestCaseContent,
    named_configuration: Option<NamedTestConfiguration>,
) -> CompilerTest {
    let basename = get_base_file_name(&filename);
    new_compiler_test_from_parts(
        test_name,
        &filename,
        &basename,
        &test_content,
        named_configuration.as_ref(),
    )
}

fn new_compiler_test_from_file_based(
    test_name: String,
    test: &CompilerFileBasedTestData,
    named_configuration: Option<&NamedTestConfiguration>,
) -> CompilerTest {
    new_compiler_test_from_parts(
        test_name,
        &test.filename,
        &test.basename,
        &test.payload,
        named_configuration,
    )
}

fn new_compiler_test_from_parts(
    test_name: String,
    filename: &str,
    basename: &str,
    test_content: &TestCaseContent,
    named_configuration: Option<&NamedTestConfiguration>,
) -> CompilerTest {
    let configured_name = configured_name(basename, named_configuration);
    let mut harness_config = named_configuration
        .map(|named| named.config.clone())
        .unwrap_or_default();

    let current_directory = normalize_absolute_path(
        harness_config
            .get("currentdirectory")
            .map(String::as_str)
            .unwrap_or_default(),
        SRC_FOLDER,
    );

    let units = &test_content.test_unit_data;
    let mut to_be_compiled = Vec::new();
    let mut other_files = Vec::new();
    let mut tsconfig = None;
    let mut tsconfig_files = Vec::new();

    let has_non_dts_files = test_content
        .test_unit_data
        .iter()
        .any(|unit| !unit.name.ends_with(".d.ts"));

    if let Some(parsed_tsconfig) = &test_content.tsconfig {
        tsconfig = Some(parsed_tsconfig.clone());
        if let Some(tsconfig_file_unit_data) = &test_content.tsconfig_file_unit_data {
            tsconfig_files.push(create_harness_test_file(
                tsconfig_file_unit_data,
                &current_directory,
            ));
        }
        for unit in units {
            let unit_name = normalize_absolute_path(&unit.name, &current_directory);
            if parsed_tsconfig.file_names.contains(&unit_name) {
                to_be_compiled.push(create_harness_test_file(unit, &current_directory));
            } else {
                other_files.push(create_harness_test_file(unit, &current_directory));
            }
        }
    } else if let Some(base_url) = harness_config.get("baseurl").cloned()
        && !is_rooted_disk_path(&base_url)
    {
        harness_config.insert(
            "baseurl".to_string(),
            normalize_absolute_path(&base_url, &current_directory),
        );
    }

    if tsconfig.is_none() && !units.is_empty() {
        let last_unit = units.last().expect("units is not empty");
        if harness_config
            .get("noimplicitreferences")
            .is_some_and(|value| !value.is_empty())
            || last_unit.content.contains(REQUIRE_STR)
            || contains_reference_path(&last_unit.content)
        {
            to_be_compiled.push(create_harness_test_file(last_unit, &current_directory));
            for unit in &units[..units.len() - 1] {
                other_files.push(create_harness_test_file(unit, &current_directory));
            }
        } else {
            to_be_compiled.extend(
                units
                    .iter()
                    .map(|unit| create_harness_test_file(unit, &current_directory)),
            );
        }
    }

    let result = harnessutil::compile_files(
        &to_be_compiled,
        &other_files,
        harness_config,
        tsconfig,
        &current_directory,
        test_content.symlinks.clone(),
    );

    CompilerTest {
        test_name,
        filename: filename.to_string(),
        basename: basename.to_string(),
        configured_name,
        options: result.options.clone(),
        harness_options: result.harness_options.clone(),
        result,
        tsconfig_files,
        to_be_compiled,
        other_files,
        has_non_dts_files,
    }
}

impl CompilerTestCommon {
    fn verify_diagnostics(
        &self,
        diagnostics: &[harnessutil::Diagnostic],
        suite_name: &str,
        is_submodule: bool,
    ) {
        let files = self.all_harness_files();
        let diagnostics = diagnostics
            .iter()
            .map(|diagnostic| tsbaseline::diagnostic_from_ast_with_files(diagnostic, &files))
            .collect::<Vec<_>>();
        let mut opts = error_baseline_options(suite_name, is_submodule);
        opts.diff_fixup_old = Some(fixup_old_error_baseline);
        tsbaseline::do_error_baseline(
            &self.configured_name,
            &files,
            &diagnostics,
            self.options.pretty,
            opts,
        )
        .unwrap_or_else(|err| {
            panic!(
                "failed to create error baseline for {}: {err}",
                self.filename
            )
        });
    }

    fn verify_javascript_output(
        &self,
        diagnostics: &[harnessutil::Diagnostic],
        program: Option<&harnessutil::ProgramHandle>,
        symlinks: &HashMap<String, String>,
        js: &harnessutil::TextOutputMap,
        dts: &harnessutil::TextOutputMap,
        inputs_and_outputs: &harnessutil::CompilationOutputMap,
        suite_name: &str,
        is_submodule: bool,
    ) {
        if !self.has_non_dts_files {
            return;
        }
        if skipped_emit_tests().contains_key(self.basename.as_str()) {
            return;
        }
        tsbaseline::do_js_emit_baseline(tsbaseline::JsEmitBaselineInput {
            baseline_path: &self.configured_name,
            header: &self.header(is_submodule),
            options: &self.options,
            diagnostics,
            program,
            symlinks,
            js,
            dts,
            inputs_and_outputs,
            ts_config_files: &self.tsconfig_files,
            to_be_compiled: &self.to_be_compiled,
            other_files: &self.other_files,
            harness_settings: &self.harness_options,
            opts: baseline_options(suite_name, is_submodule),
        })
        .unwrap_or_else(|err| {
            panic!(
                "failed to create JS emit baseline for {}: {err}",
                self.filename
            )
        });
    }

    fn verify_source_map_output(
        &self,
        diagnostics: &[harnessutil::Diagnostic],
        maps: &harnessutil::TextOutputMap,
        js: &harnessutil::TextOutputMap,
        outputs: &[TestFile],
        inputs: &[TestFile],
        suite_name: &str,
        is_submodule: bool,
    ) {
        tsbaseline::do_sourcemap_baseline(tsbaseline::SourceMapBaselineInput {
            baseline_path: &self.configured_name,
            options: &self.options,
            diagnostics,
            maps,
            js,
            outputs,
            inputs,
            harness_settings: &self.harness_options,
            opts: baseline_options(suite_name, is_submodule),
        })
        .unwrap_or_else(|err| {
            panic!(
                "failed to create sourcemap baseline for {}: {err}",
                self.filename
            )
        });
    }

    fn verify_source_map_record(
        &self,
        diagnostics: &[harnessutil::Diagnostic],
        emit_result: Option<&harnessutil::EmitResult>,
        program: Option<&harnessutil::ProgramHandle>,
        js: &harnessutil::TextOutputMap,
        dts: &harnessutil::TextOutputMap,
        suite_name: &str,
        is_submodule: bool,
    ) {
        tsbaseline::do_sourcemap_record_baseline(tsbaseline::SourceMapRecordBaselineInput {
            baseline_path: &self.configured_name,
            options: &self.options,
            diagnostics,
            emit_result,
            program,
            js,
            dts,
            opts: baseline_options(suite_name, is_submodule),
        })
        .unwrap_or_else(|err| {
            panic!(
                "failed to create sourcemap record baseline for {}: {err}",
                self.filename
            )
        });
    }

    fn verify_types_and_symbols(
        &self,
        program: Option<harnessutil::ProgramHandle>,
        diagnostics_failed: bool,
        suite_name: &str,
        is_submodule: bool,
    ) {
        if self.harness_options.no_types_and_symbols {
            return;
        }
        let mut all_files = self.to_be_compiled.clone();
        all_files.extend_from_slice(&self.other_files);
        if let Some(program) = &program {
            all_files.retain(|file| {
                let file_name = normalize_absolute_path(
                    &file.unit_name,
                    &self.harness_options.current_directory,
                );
                program.0.get_source_file_ref(&file_name).is_some()
            });
        }
        tsbaseline::do_type_and_symbol_baseline(
            &self.configured_name,
            &self.header(is_submodule),
            program,
            &all_files,
            baseline_options(suite_name, is_submodule),
            false,
            false,
            diagnostics_failed,
        )
        .unwrap_or_else(|err| {
            panic!(
                "failed to create type/symbol baseline for {}: {err}",
                self.filename
            )
        });
    }

    fn verify_module_resolution(&self, trace: &str, suite_name: &str, is_submodule: bool) {
        if !self.options.trace_resolution {
            return;
        }
        let mut opts = baseline_options(suite_name, is_submodule);
        opts.skip_diff_with_old = true;
        tsbaseline::do_module_resolution_baseline(&self.configured_name, trace, opts)
            .unwrap_or_else(|err| {
                panic!(
                    "failed to create module resolution baseline for {}: {err}",
                    self.filename
                )
            });
    }

    fn all_harness_files(&self) -> Vec<TestFile> {
        self.tsconfig_files
            .iter()
            .chain(self.to_be_compiled.iter())
            .chain(self.other_files.iter())
            .cloned()
            .collect()
    }

    fn header(&self, is_submodule: bool) -> String {
        if is_submodule && let Some((_, suffix)) = self.filename.split_once("tests/cases/") {
            return format!("tests/cases/{suffix}");
        }
        let mut components = self
            .filename
            .split('/')
            .filter(|component| !component.is_empty())
            .collect::<Vec<_>>();
        if is_submodule && components.len() > 4 {
            components.drain(..4);
        }
        components.join("/")
    }
}

fn baseline_options(suite_name: &str, is_submodule: bool) -> baseline::Options {
    baseline::Options {
        subfolder: suite_name.to_owned(),
        is_submodule,
        ..baseline::Options::default()
    }
}

fn error_baseline_options(suite_name: &str, is_submodule: bool) -> baseline::Options {
    baseline_options(suite_name, is_submodule)
}

fn fixup_old_error_baseline(text: String) -> String {
    let mut out = String::with_capacity(text.len());

    for mut line in text.split('\n') {
        const RELATIVE_PREFIX_NEW: &str = "==== ";
        const RELATIVE_PREFIX_OLD: &str = "==== ./";
        if let Some(rest) = line.strip_prefix(RELATIVE_PREFIX_OLD) {
            out.push_str(RELATIVE_PREFIX_NEW);
            line = rest;
        }

        out.push_str(line);
        out.push('\n');
    }

    out.pop();
    out
}

pub fn create_harness_test_file(unit: &TestUnit, current_directory: &str) -> TestFile {
    TestFile {
        unit_name: normalize_absolute_path(&unit.name, current_directory),
        content: unit.content.clone(),
    }
}

fn configured_name(basename: &str, named_configuration: Option<&NamedTestConfiguration>) -> String {
    let Some(named_configuration) = named_configuration else {
        return basename.to_string();
    };
    if named_configuration.name.is_empty() {
        return basename.to_string();
    }
    let extension_start = basename.rfind('.').unwrap_or(basename.len());
    format!(
        "{}({}){}",
        &basename[..extension_start],
        named_configuration.name,
        &basename[extension_start..]
    )
}

fn get_base_file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn enumerate_compiler_test_files(base_path: &str) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    enumerate_compiler_test_files_worker(Path::new(base_path), &mut files)?;
    files.sort();
    Ok(files)
}

fn enumerate_compiler_test_files_worker(
    path: &Path,
    files: &mut Vec<String>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            enumerate_compiler_test_files_worker(&path, files)?;
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "ts" || extension == "tsx")
        {
            files.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn normalize_absolute_path(file_name: &str, current_directory: &str) -> String {
    let path = if file_name.is_empty() {
        current_directory.to_string()
    } else if file_name.starts_with('/') || is_rooted_disk_path(file_name) {
        file_name.replace('\\', "/")
    } else if current_directory.ends_with('/') {
        format!("{current_directory}{file_name}")
    } else {
        format!("{current_directory}/{file_name}")
    };
    tspath::normalize_path(&path)
}

fn is_rooted_disk_path(path: &str) -> bool {
    path.len() > 2 && path.as_bytes()[1] == b':'
}

fn contains_reference_path(content: &str) -> bool {
    let bytes = content.as_bytes();
    let reference = b"reference";
    let path = b"path";
    let mut index = 0;
    while let Some(relative_start) = find_bytes(&bytes[index..], reference) {
        let start = index + relative_start + reference.len();
        if bytes
            .get(start)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            let path_start = start + 1;
            if bytes[path_start..].starts_with(path) {
                return true;
            }
        }
        index = start;
    }
    false
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
