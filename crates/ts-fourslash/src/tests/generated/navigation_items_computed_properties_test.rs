#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_items_computed_properties() {
    let mut t = TestingT;
    run_test_navigation_items_computed_properties(&mut t);
}

fn run_test_navigation_items_computed_properties(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noLib: true
[|{| "name": "C", "kind": "class" |}class C {
    [|{| "name": "foo", "kind": "method", "containerName": "C", "containerKind": "class" |}foo() { }|]
    ["hi" + "bye"]() { }
    [|{| "name": "bar", "kind": "method", "containerName": "C", "containerKind": "class" |}bar() { }|]
}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    for range in f.ranges() {
        f.verify_workspace_symbol(&[workspace_symbol_case_from_range_with_pattern(
            &range,
            range_marker_data(&range)
                .data
                .get("name")
                .unwrap()
                .to_string(),
        )]);
    }
    done();
}
