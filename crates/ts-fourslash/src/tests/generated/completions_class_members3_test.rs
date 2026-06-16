#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_class_members3() {
    let mut t = TestingT;
    run_test_completions_class_members3(&mut t);
}

fn run_test_completions_class_members3(t: &mut TestingT) {
    if should_skip_if_failing("TestCompletionsClassMembers3") {
        return;
    }
    let content = r#"interface I {
    method(): void;
}

export class C implements I {
    property = "foo" + "foo"
    /**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
