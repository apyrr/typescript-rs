#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_module() {
    let mut t = TestingT;
    run_test_smart_indent_module(&mut t);
}

fn run_test_smart_indent_module(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentModule") {
        return;
    }
    let content = r"namespace Foo {
    /*insideModule*/
}
/*afterModule*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "insideModule");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "afterModule");
    f.verify_indentation(t, 0);
    done();
}
