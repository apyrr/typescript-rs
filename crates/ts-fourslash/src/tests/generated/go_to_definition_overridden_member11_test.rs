#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_overridden_member11() {
    let mut t = TestingT;
    run_test_go_to_definition_overridden_member11(&mut t);
}

fn run_test_go_to_definition_overridden_member11(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @checkJs: true
// @noEmit: true
// @noImplicitOverride: true
// @filename: a.js
class Foo {
    /*Foo_m*/m() {}
}
class Bar extends Foo {
    /** @[|over{|"name": "1"|}ride|][| se{|"name": "2"|}e {@li{|"name": "3"|}nk https://test.c{|"name": "4"|}om} {|"name": "5"|}description |]*/
    m() {}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
        ],
    );
    done();
}
