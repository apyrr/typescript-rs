#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_dynamic_import2() {
    let mut t = TestingT;
    run_test_find_all_references_dynamic_import2(&mut t);
}

fn run_test_find_all_references_dynamic_import2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: foo.ts
[|export function /*1*/[|{| "isWriteAccess": true, "isDefinition": true, "contextRangeIndex": 0 |}bar|]() { return "bar"; }|]
var x = import("./foo");
x.then(foo => {
    foo./*2*/[|bar|]();
})"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    f.verify_baseline_rename_at_ranges_with_text(t, "bar");
    done();
}
