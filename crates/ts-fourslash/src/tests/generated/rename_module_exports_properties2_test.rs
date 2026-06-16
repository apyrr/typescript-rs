#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_module_exports_properties2() {
    let mut t = TestingT;
    run_test_rename_module_exports_properties2(&mut t);
}

fn run_test_rename_module_exports_properties2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"[|class [|{| "contextRangeIndex": 0 |}A|] {}|]
module.exports = { B: [|A|] }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![f.ranges()[1].clone().into(), f.ranges()[2].clone().into()],
    );
    done();
}
