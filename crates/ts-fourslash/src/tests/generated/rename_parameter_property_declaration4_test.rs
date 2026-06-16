#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_parameter_property_declaration4() {
    let mut t = TestingT;
    run_test_rename_parameter_property_declaration4(&mut t);
}

fn run_test_rename_parameter_property_declaration4(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameParameterPropertyDeclaration4") {
        return;
    }
    let content = r#"class Foo {
    constructor([|protected { [|{| "contextRangeIndex": 0 |}protectedParam|] }|]) {
        let myProtectedParam = [|protectedParam|];
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[2].clone().into()],
    );
    done();
}
