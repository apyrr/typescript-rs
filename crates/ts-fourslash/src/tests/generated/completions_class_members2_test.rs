#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_class_members2() {
    let mut t = TestingT;
    run_test_completions_class_members2(&mut t);
}

fn run_test_completions_class_members2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface I {
    method(): void;
}

export class C implements I {
    property = "foo"
    /**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
