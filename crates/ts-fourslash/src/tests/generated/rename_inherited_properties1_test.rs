#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_inherited_properties1() {
    let mut t = TestingT;
    run_test_rename_inherited_properties1(&mut t);
}

fn run_test_rename_inherited_properties1(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameInheritedProperties1") {
        return;
    }
    let content = r#"class class1 extends class1 {
   [|[|{| "contextRangeIndex": 0 |}propName|]: string;|]
}

var v: class1;
v.[|propName|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "propName");
    done();
}
