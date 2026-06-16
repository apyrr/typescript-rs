#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_across_multiple_projects() {
    let mut t = TestingT;
    run_test_rename_across_multiple_projects(&mut t);
}

fn run_test_rename_across_multiple_projects(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameAcrossMultipleProjects") {
        return;
    }
    let content = r#"//@Filename: a.ts
[|var [|{| "contextRangeIndex": 0 |}x|]: number;|]
//@Filename: b.ts
/// <reference path="a.ts" />
[|x|]++;
//@Filename: c.ts
/// <reference path="a.ts" />
[|x|]++;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "x");
    done();
}
