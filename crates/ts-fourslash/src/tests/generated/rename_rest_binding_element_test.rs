#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_rest_binding_element() {
    let mut t = TestingT;
    run_test_rename_rest_binding_element(&mut t);
}

fn run_test_rename_rest_binding_element(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameRestBindingElement") {
        return;
    }
    let content = r#"interface I {
    a: number;
    b: number;
    c: number;
}
function foo([|{ a, ...[|{| "contextRangeIndex": 0 |}rest|] }: I|]) {
    [|rest|];
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[1].clone().into()]);
    done();
}
