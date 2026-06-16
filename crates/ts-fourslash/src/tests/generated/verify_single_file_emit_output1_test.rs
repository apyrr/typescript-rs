#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_verify_single_file_emit_output1() {
    let mut t = TestingT;
    run_test_verify_single_file_emit_output1(&mut t);
}

fn run_test_verify_single_file_emit_output1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: verifySingleFileEmitOutput1_file0.ts
export class A {
}
export class Z {
}
// @Filename: verifySingleFileEmitOutput1_file1.ts
import f = require("./verifySingleFileEmitOutput1_file0");
var /**/b = new f.A();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var b: f.A", "");
    done();
}
