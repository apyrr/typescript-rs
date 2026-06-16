#![forbid(unsafe_code)]

mod compiler_runner;
mod test_case_parser;

#[cfg(test)]
mod test_case_parser_test;
#[cfg(test)]
mod testmain_test;

pub use compiler_runner::{
    CompilationResult, CompilerBaselineRunner, CompilerFileBasedTest, CompilerOptions,
    CompilerTest, CompilerTestType, HarnessOptions, NamedTestConfiguration, ParsedCommandLine,
    REQUIRE_STR, SRC_FOLDER, TestCaseContent, TestCaseContentWithConfig, TestConfiguration,
    TestFile, TestUnit, create_harness_test_file, get_compiler_file_based_test,
    get_compiler_vary_by_map, new_compiler_baseline_runner, new_compiler_test, skipped_emit_tests,
};
pub use test_case_parser::{
    ParseTestFilesOptions, RawCompilerSettings, extract_compiler_settings, make_units_from_test,
    parse_symlink_from_test, parse_test_files_and_symlinks,
    parse_test_files_and_symlinks_with_options,
};

pub trait TestContext {
    fn helper(&mut self) {}
}

pub trait Runner {
    fn enumerate_test_files(&self) -> Vec<String>;
    fn run_tests(&self, t: &mut dyn TestContext);
}

#[expect(
    dead_code,
    reason = "ported runner aggregation helper is ahead of current callers"
)]
fn run_tests(t: &mut dyn TestContext, runners: &[Box<dyn Runner>]) {
    for runner in runners {
        runner.run_tests(t);
    }
}
