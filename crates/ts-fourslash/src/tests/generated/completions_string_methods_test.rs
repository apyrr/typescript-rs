#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_string_methods() {
    let mut t = TestingT;
    run_test_completions_string_methods(&mut t);
}

fn run_test_completions_string_methods(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @lib: es5
var s = "foo"./*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
