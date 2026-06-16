#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_binding_element_initializer_external() {
    let mut t = TestingT;
    run_test_rename_binding_element_initializer_external(&mut t);
}

fn run_test_rename_binding_element_initializer_external(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameBindingElementInitializerExternal") {
        return;
    }
    let content = r#"// @lib: es5
[|const [|{| "contextRangeIndex": 0 |}external|] = true;|]

function f({
    lvl1 = [|external|],
    nested: { lvl2 = [|external|]},
    oldName: newName = [|external|]
}) {}

const {
    lvl1 = [|external|],
    nested: { lvl2 = [|external|]},
    oldName: newName = [|external|]
} = obj;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "external");
    done();
}
