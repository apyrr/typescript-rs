#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_this() {
    let mut t = TestingT;
    run_test_rename_this(&mut t);
}

fn run_test_rename_this(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameThis") {
        return;
    }
    let content = r#"function f([|this|]) {
    return [|this|];
}
this/**/;
const _ = { [|[|{| "contextRangeIndex": 2 |}this|]: 0|] }.[|this|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_rename_failed_at_current_position();
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[0].clone().into(),
            f.ranges()[1].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[4].clone().into(),
        ],
    );
    done();
}
