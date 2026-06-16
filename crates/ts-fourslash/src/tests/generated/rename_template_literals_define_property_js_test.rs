#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_template_literals_define_property_js() {
    let mut t = TestingT;
    run_test_rename_template_literals_define_property_js(&mut t);
}

fn run_test_rename_template_literals_define_property_js(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameTemplateLiteralsDefinePropertyJs") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: a.js
let obj = {};

Object.defineProperty(obj, `[|prop|]`, { value: 0 });

obj = {
    [|[`[|{| "contextRangeIndex": 1 |}prop|]`]: 1|]
};

obj.[|prop|];
obj['[|prop|]'];
obj["[|prop|]"];
obj[`[|prop|]`];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "prop");
    done();
}
