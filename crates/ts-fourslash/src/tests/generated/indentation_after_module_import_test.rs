#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_indentation_after_module_import() {
    let mut t = TestingT;
    run_test_indentation_after_module_import(&mut t);
}

fn run_test_indentation_after_module_import(t: &mut TestingT) {
    if should_skip_if_failing("TestIndentationAfterModuleImport") {
        return;
    }
    let content = r#"declare module "fs" { };
import im = module("fs");/**/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "\n");
    f.verify_indentation(t, 0);
    done();
}
