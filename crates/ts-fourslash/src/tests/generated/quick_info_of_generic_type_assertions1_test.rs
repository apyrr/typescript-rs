#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_of_generic_type_assertions1() {
    let mut t = TestingT;
    run_test_quick_info_of_generic_type_assertions1(&mut t);
}

fn run_test_quick_info_of_generic_type_assertions1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOfGenericTypeAssertions1") {
        return;
    }
    let content = r"function f<T>(x: T): T { return null; }
var /*1*/r = <T>(x: T) => x;
var /*2*/r2 = < <T>(x: T) => T>f;
var a;
var /*3*/r3 = < <T>(x: <A>(y: A) => A) => T>a;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var r: <T>(x: T) => T", "");
    f.verify_quick_info_at(t, "2", "var r2: <T>(x: T) => T", "");
    f.verify_quick_info_at(t, "3", "var r3: <T>(x: <A>(y: A) => A) => T", "");
    done();
}
