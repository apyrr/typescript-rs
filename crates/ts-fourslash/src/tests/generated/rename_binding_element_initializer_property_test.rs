#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_binding_element_initializer_property() {
    let mut t = TestingT;
    run_test_rename_binding_element_initializer_property(&mut t);
}

fn run_test_rename_binding_element_initializer_property(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameBindingElementInitializerProperty") {
        return;
    }
    let content = r#"function f([|{[|{| "contextRangeIndex": 0 |}required|], optional = [|required|]}: {[|[|{| "contextRangeIndex": 3 |}required|]: number,|] optional?: number}|]) {
    console.log("required", [|required|]);
    console.log("optional", optional);
}

f({[|[|{| "contextRangeIndex": 6 |}required|]: 10|]});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[2].clone().into(),
            f.ranges()[5].clone().into(),
            f.ranges()[4].clone().into(),
            f.ranges()[7].clone().into(),
        ],
    );
    done();
}
