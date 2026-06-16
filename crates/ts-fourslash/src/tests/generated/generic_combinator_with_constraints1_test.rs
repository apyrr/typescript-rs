#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_combinator_with_constraints1() {
    let mut t = TestingT;
    run_test_generic_combinator_with_constraints1(&mut t);
}

fn run_test_generic_combinator_with_constraints1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericCombinatorWithConstraints1") {
        return;
    }
    let content = r"function apply<T, U extends Date>(source: T[], selector: (x: T) => U) {
    var /*1*/xs = source.map(selector); // any[]
    var /*2*/xs2 = source.map((x: T, a, b): U => { return null }); // any[] 
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(local var) xs: U[]", "");
    f.verify_quick_info_at(t, "2", "(local var) xs2: U[]", "");
    done();
}
