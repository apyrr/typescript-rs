#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_from_contextual_union_type2() {
    let mut t = TestingT;
    run_test_quick_info_from_contextual_union_type2(&mut t);
}

fn run_test_quick_info_from_contextual_union_type2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @strict: true
function test1(arg: { prop: "foo" }) {}
test1({ /*1*/prop: "bar" });

function test2(arg: { prop: "foo" } | undefined) {}
test2({ /*2*/prop: "bar" });"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) prop: \"foo\"", "");
    f.verify_quick_info_at(t, "2", "(property) prop: \"foo\"", "");
    done();
}
