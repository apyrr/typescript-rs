#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_object_spread() {
    let mut t = TestingT;
    run_test_rename_object_spread(&mut t);
}

fn run_test_rename_object_spread(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameObjectSpread") {
        return;
    }
    let content = r#"interface A1 { [|[|{| "contextRangeIndex": 0 |}a|]: number|] };
interface A2 { [|[|{| "contextRangeIndex": 2 |}a|]?: number|] };
let a1: A1;
let a2: A2;
let a12 = { ...a1, ...a2 };
a12.[|a|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[4].clone().into(),
        ],
    );
    done();
}
