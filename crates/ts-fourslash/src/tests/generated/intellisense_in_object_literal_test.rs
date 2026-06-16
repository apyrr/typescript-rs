#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_intellisense_in_object_literal() {
    let mut t = TestingT;
    run_test_intellisense_in_object_literal(&mut t);
}

fn run_test_intellisense_in_object_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestIntellisenseInObjectLiteral") {
        return;
    }
    let content = r#"var x = 3;

class Foo {
    static something() {
        return { "prop": /**/x };
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var x: number", "");
    done();
}
