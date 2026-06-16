#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_at_prop_with_ambient_declaration_in_js() {
    let mut t = TestingT;
    run_test_quick_info_at_prop_with_ambient_declaration_in_js(&mut t);
}

fn run_test_quick_info_at_prop_with_ambient_declaration_in_js(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @filename: /a.js
class C {
    constructor() {
        this.prop = "";
    }
    declare prop: string;
    method() {
        this.prop.foo/**/
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
