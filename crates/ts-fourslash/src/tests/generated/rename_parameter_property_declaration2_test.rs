#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_parameter_property_declaration2() {
    let mut t = TestingT;
    run_test_rename_parameter_property_declaration2(&mut t);
}

fn run_test_rename_parameter_property_declaration2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class Foo {
    constructor([|public [|{| "contextRangeIndex": 0 |}publicParam|]: number|]) {
        let publicParam = [|publicParam|];
        this.[|publicParam|] += 10;
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "publicParam");
    done();
}
