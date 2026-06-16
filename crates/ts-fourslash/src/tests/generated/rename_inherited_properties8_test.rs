#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_inherited_properties8() {
    let mut t = TestingT;
    run_test_rename_inherited_properties8(&mut t);
}

fn run_test_rename_inherited_properties8(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameInheritedProperties8") {
        return;
    }
    let content = r#"class C implements D {
    [|[|{| "contextRangeIndex": 0 |}prop1|]: string;|]
}

interface D extends C {
    [|[|{| "contextRangeIndex": 2 |}prop1|]: string;|]
}

var c: C;
c.[|prop1|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "prop1");
    done();
}
