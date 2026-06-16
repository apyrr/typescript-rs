#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_unused_label_no_suggestion_if_disabled() {
    let mut t = TestingT;
    run_test_code_fix_unused_label_no_suggestion_if_disabled(&mut t);
}

fn run_test_code_fix_unused_label_no_suggestion_if_disabled(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixUnusedLabel_noSuggestionIfDisabled") {
        return;
    }
    let content = r"// @allowUnusedLabels: true
[|foo|]: while (true) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
