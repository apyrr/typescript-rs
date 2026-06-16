#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_derived_generic_type_with_constructor() {
    let mut t = TestingT;
    run_test_quick_info_for_derived_generic_type_with_constructor(&mut t);
}

fn run_test_quick_info_for_derived_generic_type_with_constructor(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForDerivedGenericTypeWithConstructor") {
        return;
    }
    let content = r"class A<T> {
    foo() { }
}
class B<T> extends A<T> {
    bar() { }
    constructor() { super() }
}
class B2<T> extends A<T> {
    bar() { }
}
var /*1*/b: B<number>;
var /*2*/b2: B<number>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var b: B<number>", "");
    f.verify_quick_info_at(t, "2", "var b2: B<number>", "");
    done();
}
