#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_imports2_fs() {
    let mut t = TestingT;
    run_test_unused_imports2_fs(&mut t);
}

fn run_test_unused_imports2_fs(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedImports2FS") {
        return;
    }
    let content = r#"// @noUnusedLocals: true
// @Filename: file2.ts
[|import {test} from "./file1"
import {Calculator} from "./file1"|]
var x = new Calculator();
x.handleChar();
// @Filename: file1.ts
export class Calculator {
   handleChar() {}
}
export function test() {

}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "import {Calculator} from \"./file1\"", true, 0, 0);
    done();
}
