#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_indexer_result_with_constraint() {
    let mut t = TestingT;
    run_test_quick_info_for_indexer_result_with_constraint(&mut t);
}

fn run_test_quick_info_for_indexer_result_with_constraint(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForIndexerResultWithConstraint") {
        return;
    }
    let content = r"function foo<T>(x: T) {
        return x;
}
function other2<T extends Date>(arg: T) {
    var b: { [x: string]: T };
    var /*1*/r2 = foo(b); // just shows T
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(local var) r2: {\n    [x: string]: T;\n}", "");
    done();
}
