#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_imports11_fs() {
    let mut t = TestingT;
    run_test_unused_imports11_fs(&mut t);
}

fn run_test_unused_imports11_fs(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedImports11FS") {
        return;
    }
    let content = r#"// @noUnusedLocals: true
// @Filename: file2.ts
[| import f1, * as s from "./file1"; |]
s.f2('hello');
// @Filename: file1.ts
export var v1;
export function f1(n: number){}
export function f2(s: string){};
export default f1;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "import * as s from \"./file1\";", false, 0, 0);
    done();
}
