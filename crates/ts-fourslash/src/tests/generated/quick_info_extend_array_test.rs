#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_extend_array() {
    let mut t = TestingT;
    run_test_quick_info_extend_array(&mut t);
}

fn run_test_quick_info_extend_array(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Foo<T> extends Array<T> { }
var x: Foo<string>;
var /*1*/r = x[0];
interface Foo2 extends Array<string> { }
var x2: Foo2;
var /*2*/r2 = x2[0];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var r: string", "");
    f.verify_quick_info_at(t, "2", "var r2: string", "");
    done();
}
