#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_default_params_and_contextual_types() {
    let mut t = TestingT;
    run_test_default_params_and_contextual_types(&mut t);
}

fn run_test_default_params_and_contextual_types(t: &mut TestingT) {
    if should_skip_if_failing("TestDefaultParamsAndContextualTypes") {
        return;
    }
    let content = r"// @strict: false
interface FooOptions {
    text?: string;
}
interface Foo {
    bar(xy: string, options?: FooOptions): void;
}
var o: Foo = {
    bar: function (x/*1*/y, opt/*2*/ions = {}) {
        // expect xy to have type string, and options to have type FooOptions in here
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) xy: string", "");
    f.verify_quick_info_at(t, "2", "(parameter) options: FooOptions", "");
    done();
}
