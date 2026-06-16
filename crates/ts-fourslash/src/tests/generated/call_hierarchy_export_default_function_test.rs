#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_call_hierarchy_export_default_function() {
    let mut t = TestingT;
    run_test_call_hierarchy_export_default_function(&mut t);
}

fn run_test_call_hierarchy_export_default_function(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @filename: main.ts
import bar from "./other";

function foo() {
    bar();
}
// @filename: other.ts
export /**/default function () {
    baz();
}

function baz() {
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_baseline_call_hierarchy(t);
    done();
}
