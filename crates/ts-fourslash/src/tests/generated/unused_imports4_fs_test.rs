#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_imports4_fs() {
    let mut t = TestingT;
    run_test_unused_imports4_fs(&mut t);
}

fn run_test_unused_imports4_fs(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @noUnusedLocals: true
// @Filename: file2.ts
[| import {Calculator, test, test2} from "./file1" |]

var x = new Calculator();
x.handleChar();
test2();
// @Filename: file1.ts
export class Calculator {
    handleChar() {}
}

export function test() {

}

export function test2() {

}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "import {Calculator, test2} from \"./file1\"",
        false,
        0,
        0,
    );
    done();
}
