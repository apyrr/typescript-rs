#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_declare_in_module() {
    let mut t = TestingT;
    run_test_remove_declare_in_module(&mut t);
}

fn run_test_remove_declare_in_module(t: &mut TestingT) {
    if should_skip_if_failing("TestRemoveDeclareInModule") {
        return;
    }
    let content = r"/**/export namespace Foo {
    function a(): void {}
}

Foo.a();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.delete_at_caret(t, 7);
    f.verify_number_of_errors_in_current_file(1);
    done();
}
