#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_default_lib_dont_work() {
    let mut t = TestingT;
    run_test_rename_default_lib_dont_work(&mut t);
}

fn run_test_rename_default_lib_dont_work(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameDefaultLibDontWork") {
        return;
    }
    let content = r#"// @Filename: file1.ts
[|var [|{| "contextRangeIndex": 0 |}test|] = "foo";|]
console.log([|test|]);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[1].clone().into()]);
    done();
}
