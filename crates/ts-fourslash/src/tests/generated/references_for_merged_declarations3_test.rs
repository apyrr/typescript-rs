#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_merged_declarations3() {
    let mut t = TestingT;
    run_test_references_for_merged_declarations3(&mut t);
}

fn run_test_references_for_merged_declarations3(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForMergedDeclarations3") {
        return;
    }
    let content = r"[|class /*class*/[|testClass|] {
    static staticMethod() { }
    method() { }
}|]

[|module /*module*/[|testClass|] {
    export interface Bar {

    }
}|]

var c1: [|testClass|];
var c2: [|testClass|].Bar;
[|testClass|].staticMethod();
[|testClass|].prototype.method();
[|testClass|].bind(this);
new [|testClass|]();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["module".to_string(), "class".to_string()]);
    done();
}
