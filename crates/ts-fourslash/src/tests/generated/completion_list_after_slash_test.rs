#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_after_slash() {
    let mut t = TestingT;
    run_test_completion_list_after_slash(&mut t);
}

fn run_test_completion_list_after_slash(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionListAfterSlash") {
        return;
    }
    let content = r"var a = 0;
a/./**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    done();
}
