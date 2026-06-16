#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_generic_constraints1() {
    let mut t = TestingT;
    run_test_quick_info_for_generic_constraints1(&mut t);
}

fn run_test_quick_info_for_generic_constraints1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function foo4<T extends Date>(te/**/st: T): T;
function foo4<T extends Date>(test: any): any { return null; }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(parameter) test: T extends Date", "");
    done();
}
