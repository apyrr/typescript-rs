#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_remove_exported_class_from_reopened_module() {
    let mut t = TestingT;
    run_test_remove_exported_class_from_reopened_module(&mut t);
}

fn run_test_remove_exported_class_from_reopened_module(t: &mut TestingT) {
    if should_skip_if_failing("TestRemoveExportedClassFromReopenedModule") {
        return;
    }
    let content = r"namespace multiM { }

namespace multiM {
    /*1*/export class c { }
}
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.delete_at_caret(t, 18);
    f.go_to_eof(t);
    f.insert(t, "new multiM.c();");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
