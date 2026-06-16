#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_fixing_type_parameters_quick_info() {
    let mut t = TestingT;
    run_test_fixing_type_parameters_quick_info(&mut t);
}

fn run_test_fixing_type_parameters_quick_info(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
declare function f<T>(x: T, y: (p: T) => T, z: (p: T) => T): T;
var /*1*/result = /*2*/f(0, /*3*/x => null, /*4*/x => x.blahblah);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var result: number", "");
    f.verify_quick_info_at(
        t,
        "2",
        "function f<number>(x: number, y: (p: number) => number, z: (p: number) => number): number",
        "",
    );
    f.verify_quick_info_at(t, "3", "(parameter) x: number", "");
    f.verify_quick_info_at(t, "4", "(parameter) x: number", "");
    done();
}
