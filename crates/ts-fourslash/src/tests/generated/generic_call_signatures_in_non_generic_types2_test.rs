#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_call_signatures_in_non_generic_types2() {
    let mut t = TestingT;
    run_test_generic_call_signatures_in_non_generic_types2(&mut t);
}

fn run_test_generic_call_signatures_in_non_generic_types2(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericCallSignaturesInNonGenericTypes2") {
        return;
    }
    let content = r"interface WrappedArray<T> { }
interface Underscore {
    <T>(list: T[]): WrappedArray<T>;
}
var _: Underscore;
var a: number[];
var /**/b = _(a);  // WrappedArray<any>, should be WrappedArray<number>";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var b: WrappedArray<number>", "");
    done();
}
