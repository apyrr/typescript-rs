#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_exhaustive_case_completions9() {
    let mut t = TestingT;
    run_test_exhaustive_case_completions9(&mut t);
}

fn run_test_exhaustive_case_completions9(t: &mut TestingT) {
    if should_skip_if_failing("TestExhaustiveCaseCompletions9") {
        return;
    }
    let content = r#"// @lib: es5
// @newline: LF
switch (Math.random() ? 123 : 456) {
    case "foo!":
    case/**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
