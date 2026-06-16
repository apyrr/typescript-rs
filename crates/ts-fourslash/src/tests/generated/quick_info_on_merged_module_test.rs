#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_merged_module() {
    let mut t = TestingT;
    run_test_quick_info_on_merged_module(&mut t);
}

fn run_test_quick_info_on_merged_module(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnMergedModule") {
        return;
    }
    let content = r"// @strict: false
namespace M2 {
    export interface A {
        foo: string;
    }
    var a: A;
    var r = a.foo + a.bar;
}
namespace M2 {
    export interface A {
        bar: number;
    }
    var a: A;
    var r = a.fo/*1*/o + a.bar;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) M2.A.foo: string", "");
    f.verify_no_errors();
    done();
}
