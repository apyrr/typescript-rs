use std::sync::Once;

use ts_testrunner::{CompilerTestType, new_compiler_baseline_runner};

static COMPILER_BASELINE_CLEANUP: Once = Once::new();
static CONFORMANCE_BASELINE_CLEANUP: Once = Once::new();
const COMPILER_BASELINE_TEST_STACK_SIZE: usize = 64 * 1024 * 1024;

fn run_submodule_compiler_baseline_config(
    test_type: CompilerTestType,
    relative_path: &'static str,
    config_index: Option<usize>,
) {
    let result = std::thread::Builder::new()
        .stack_size(COMPILER_BASELINE_TEST_STACK_SIZE)
        .spawn(move || {
            run_submodule_compiler_baseline_config_worker(test_type, relative_path, config_index)
        })
        .expect("failed to spawn compiler baseline test worker")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn run_submodule_compiler_baseline_config_worker(
    test_type: CompilerTestType,
    relative_path: &str,
    config_index: Option<usize>,
) {
    if ts_repo::skip_if_no_type_script_submodule() {
        return;
    }

    clean_up_submodule_baselines_once(test_type);

    let suite_name = test_type.string();
    let mut runner = new_compiler_baseline_runner(test_type, true);
    runner.clean_up_local_before_run = false;
    let filename = ts_repo::type_script_submodule_path()
        .join("tests")
        .join("cases")
        .join(suite_name)
        .join(relative_path)
        .to_string_lossy()
        .replace('\\', "/");

    runner.run_test_config(&filename, config_index);
}

fn clean_up_submodule_baselines_once(test_type: CompilerTestType) {
    let cleanup = match test_type {
        CompilerTestType::Regression => &COMPILER_BASELINE_CLEANUP,
        CompilerTestType::Conformance => &CONFORMANCE_BASELINE_CLEANUP,
    };
    cleanup.call_once(|| new_compiler_baseline_runner(test_type, true).clean_up_local());
}

include!(concat!(env!("OUT_DIR"), "/compiler_baselines_generated.rs"));
