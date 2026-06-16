#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_function_declaration() {
    let mut t = TestingT;
    run_test_quick_info_for_function_declaration(&mut t);
}

fn run_test_quick_info_for_function_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface A<T> { }

function ma/*makeA*/keA<T>(t: T): A<T> { return null; }

function /*f*/f<T>(t: T) {
    return makeA(t);
}

var x = f(0);
var y = makeA(0);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "makeA", "function makeA<T>(t: T): A<T>", "");
    f.verify_quick_info_at(t, "f", "function f<T>(t: T): A<T>", "");
    done();
}
