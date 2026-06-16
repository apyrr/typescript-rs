#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_server_commit_characters() {
    let mut t = TestingT;
    run_test_completions_server_commit_characters(&mut t);
}

fn run_test_completions_server_commit_characters(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsServerCommitCharacters") {
        return;
    }
    let content = r#"// @lib: es5
// @Filename: /home/src/workspaces/project/src/index.ts
const a: "aa" | "bb" = "/**/";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_completions(t, &[]);
    done();
}
