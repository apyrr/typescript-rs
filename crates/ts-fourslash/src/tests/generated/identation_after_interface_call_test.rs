#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_identation_after_interface_call() {
    let mut t = TestingT;
    run_test_identation_after_interface_call(&mut t);
}

fn run_test_identation_after_interface_call(t: &mut TestingT) {
    if should_skip_if_failing("TestIdentationAfterInterfaceCall") {
        return;
    }
    let content = r"interface bah {
    (y: number);
    x: number;
    (z: string);/**/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    done();
}
