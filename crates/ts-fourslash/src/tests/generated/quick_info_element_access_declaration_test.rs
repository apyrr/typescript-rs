#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_element_access_declaration() {
    let mut t = TestingT;
    run_test_quick_info_element_access_declaration(&mut t);
}

fn run_test_quick_info_element_access_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @checkJs: true
// @allowJs: true
// @Filename: a.js
const mod = {};
mod["@@thing1"] = {};
mod["/**/@@thing1"]["@@thing2"] = 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(
        t,
        "module mod[\"@@thing1\"]\n(property) mod[\"@@thing1\"]: typeof mod.@@thing1",
        "",
    );
    done();
}
