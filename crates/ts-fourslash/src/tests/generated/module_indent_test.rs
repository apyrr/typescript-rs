#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_module_indent() {
    let mut t = TestingT;
    run_test_module_indent(&mut t);
}

fn run_test_module_indent(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_bof(t);
    f.insert(t, "namespace M {\n");
    f.verify_indentation(t, 4);
    done();
}
