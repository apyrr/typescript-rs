#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_expression_type_not_changed_via_deletion() {
    let mut t = TestingT;
    run_test_quickinfo_expression_type_not_changed_via_deletion(&mut t);
}

fn run_test_quickinfo_expression_type_not_changed_via_deletion(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"type TypeEq<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;

const /*2*/test1: TypeEq<number[], [number, ...number[]]> = false;

declare const foo: [number, ...number[]];
declare const bar: number[];

const /*1*/test2: TypeEq<typeof foo, typeof bar> = false;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_quick_info_is(t, "const test2: false", "");
    f.go_to_marker(t, "2");
    f.verify_quick_info_is(t, "const test1: false", "");
    done();
}
