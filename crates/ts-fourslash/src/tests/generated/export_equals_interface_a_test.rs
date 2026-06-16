#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_export_equals_interface_a() {
    let mut t = TestingT;
    run_test_export_equals_interface_a(&mut t);
}

fn run_test_export_equals_interface_a(t: &mut TestingT) {
    if should_skip_if_failing("TestExportEqualsInterfaceA") {
        return;
    }
    let content = r"// @Filename: exportEqualsInterface_A.ts
interface A {
    p1: number;
}
export = A;
/**/
var i: I1;
var n: number = i.p1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "import I1 = require(\"exportEqualsInterface_A\");");
    done();
}
