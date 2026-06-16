#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_alias_external_module2() {
    let mut t = TestingT;
    run_test_rename_alias_external_module2(&mut t);
}

fn run_test_rename_alias_external_module2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: a.ts
[|module [|{| "contextRangeIndex": 0 |}SomeModule|] { export class SomeClass { } }|]
[|export = [|{| "contextRangeIndex": 2 |}SomeModule|];|]
// @Filename: b.ts
[|import [|{| "contextRangeIndex": 4 |}M|] = require("./a");|]
import C = [|M|].SomeClass;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[5].clone().into(),
            f.ranges()[6].clone().into(),
        ],
    );
    done();
}
