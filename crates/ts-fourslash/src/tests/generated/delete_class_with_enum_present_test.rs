#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_delete_class_with_enum_present() {
    let mut t = TestingT;
    run_test_delete_class_with_enum_present(&mut t);
}

fn run_test_delete_class_with_enum_present(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"enum Foo { a, b, c }
/**/class Bar { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.delete_at_caret(t, 13);
    f.verify_baseline_document_symbol(t);
    done();
}
