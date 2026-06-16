#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_call_signatures_in_non_generic_types1() {
    let mut t = TestingT;
    run_test_generic_call_signatures_in_non_generic_types1(&mut t);
}

fn run_test_generic_call_signatures_in_non_generic_types1(t: &mut TestingT) {
    if should_skip_if_failing("TestGenericCallSignaturesInNonGenericTypes1") {
        return;
    }
    let content = r"interface WrappedObject<T> { }
interface WrappedArray<T> { }
interface Underscore {
    <T>(list: T[]): WrappedArray<T>;
    <T>(obj: T): WrappedObject<T>;
}
var _: Underscore;
var a: number[];
var /**/b = _(a); ";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var b: WrappedArray<number>", "");
    done();
}
