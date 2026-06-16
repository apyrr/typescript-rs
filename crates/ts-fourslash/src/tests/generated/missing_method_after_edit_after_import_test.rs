#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_missing_method_after_edit_after_import() {
    let mut t = TestingT;
    run_test_missing_method_after_edit_after_import(&mut t);
}

fn run_test_missing_method_after_edit_after_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace foo {
    export namespace bar { namespace baz { export class boo { } } }
}

import f = /*foo*/foo;

/*delete*/var x;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "foo", "namespace foo", "");
    f.go_to_marker(t, "delete");
    f.delete_at_caret(t, 6);
    f.verify_quick_info_at(t, "foo", "namespace foo", "");
    done();
}
