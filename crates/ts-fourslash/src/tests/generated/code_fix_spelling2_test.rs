#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_spelling2() {
    let mut t = TestingT;
    run_test_code_fix_spelling2(&mut t);
}

fn run_test_code_fix_spelling2(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixSpelling2") {
        return;
    }
    let content = r"[|function foo<T extends number | string>(x: T) {
    return x.toStrang();
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "function foo<T extends number | string>(x: T) {\n    return x.toString();\n}",
        false,
        0,
        0,
    );
    done();
}
