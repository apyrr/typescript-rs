#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_return_type_of_generic_function1() {
    let mut t = TestingT;
    run_test_return_type_of_generic_function1(&mut t);
}

fn run_test_return_type_of_generic_function1(t: &mut TestingT) {
    if should_skip_if_failing("TestReturnTypeOfGenericFunction1") {
        return;
    }
    let content = r"interface WrappedArray<T> {
    map<U>(iterator: (value: T) => U, context?: any): U[];
}
var x: WrappedArray<string>;
var /**/y = x.map(s => s.length);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var y: number[]", "");
    done();
}
