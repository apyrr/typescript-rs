#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_quick_info_for_intersection_types() {
    let mut t = TestingT;
    run_test_get_quick_info_for_intersection_types(&mut t);
}

fn run_test_get_quick_info_for_intersection_types(t: &mut TestingT) {
    if should_skip_if_failing("TestGetQuickInfoForIntersectionTypes") {
        return;
    }
    let content = r"function f(): string & {(): any} {
	return <any>{};
}
let x = f();
x/**/();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "let x: () => any", "");
    done();
}
