#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_string_property_names() {
    let mut t = TestingT;
    run_test_rename_string_property_names(&mut t);
}

fn run_test_rename_string_property_names(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var o = {
    [|[|{| "contextRangeIndex": 0 |}prop|]: 0|]
};

o = {
    [|"[|{| "contextRangeIndex": 2 |}prop|]": 1|]
};

o["[|prop|]"];
o['[|prop|]'];
o.[|prop|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "prop");
    done();
}
