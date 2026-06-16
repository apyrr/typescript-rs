#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigate_items_const() {
    let mut t = TestingT;
    run_test_navigate_items_const(&mut t);
}

fn run_test_navigate_items_const(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noLib: true
const [|{| "name": "c", "kind": "const" |}c = 10|];
function foo() {
    const [|{| "name": "d", "kind": "const", "containerName": "foo", "containerKind": "function" |}d = 10|];
}"#;
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
