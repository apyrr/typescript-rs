use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::OnceLock,
};

use crate::TestingT;

static FAILING_TESTS: OnceLock<HashSet<String>> = OnceLock::new();

fn failing_tests() -> &'static HashSet<String> {
    FAILING_TESTS.get_or_init(|| {
        let mut failing_tests_set = HashSet::new();

        // Get the path to failingTests.txt relative to this source file
        let mut failing_tests_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        failing_tests_path.pop();
        failing_tests_path.pop();
        failing_tests_path.push("vendor");
        failing_tests_path.push("typescript-go");
        failing_tests_path.push("internal");
        failing_tests_path.push("fourslash");
        failing_tests_path.push("_scripts");
        failing_tests_path.push("failingTests.txt");

        let Ok(file) = File::open(failing_tests_path) else {
            return failing_tests_set;
        };

        let scanner = BufReader::new(file);
        for line in scanner.lines().map_while(Result::ok) {
            let line = line.trim();
            if !line.is_empty() {
                failing_tests_set.insert(line.to_string());
            }
        }
        failing_tests_set
    })
}

// SkipIfFailing checks if the current test is in the failingTests.txt file
// and skips it unless the TSGO_FOURSLASH_IGNORE_FAILING environment variable is set.
// This allows tests to be marked as failing without modifying the test files themselves.
pub fn should_skip_if_failing(test_name: &str) -> bool {
    if env::var("TSGO_FOURSLASH_IGNORE_FAILING").is_ok_and(|value| !value.is_empty()) {
        return false;
    }

    failing_tests().contains(test_name)
}

pub fn skip_if_failing(t: &mut TestingT) {
    t.helper();

    if env::var("TSGO_FOURSLASH_IGNORE_FAILING").is_ok_and(|value| !value.is_empty()) {
        return;
    }

    if failing_tests().contains(t.name()) {
        t.skip("Test is in failingTests.txt");
    }
}
