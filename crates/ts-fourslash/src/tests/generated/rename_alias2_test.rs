#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_alias2() {
    let mut t = TestingT;
    run_test_rename_alias2(&mut t);
}

fn run_test_rename_alias2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"[|module [|{| "contextRangeIndex": 0 |}SomeModule|] { export class SomeClass { } }|]
import M = [|SomeModule|];
import C = M.SomeClass;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "SomeModule");
    done();
}
