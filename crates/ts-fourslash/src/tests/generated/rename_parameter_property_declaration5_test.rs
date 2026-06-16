#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_parameter_property_declaration5() {
    let mut t = TestingT;
    run_test_rename_parameter_property_declaration5(&mut t);
}

fn run_test_rename_parameter_property_declaration5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class Foo {
    constructor([|protected [ [|{| "contextRangeIndex": 0 |}protectedParam|] ]|]) {
        let myProtectedParam = [|protectedParam|];
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "protectedParam");
    done();
}
