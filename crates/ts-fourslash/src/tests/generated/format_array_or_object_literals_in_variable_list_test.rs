#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_array_or_object_literals_in_variable_list() {
    let mut t = TestingT;
    run_test_format_array_or_object_literals_in_variable_list(&mut t);
}

fn run_test_format_array_or_object_literals_in_variable_list(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var v30 = [1, 2], v31, v32, v33 = [0], v34 = {'a': true}, v35;/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "");
    f.verify_current_line_content(
        t,
        "var v30 = [1, 2], v31, v32, v33 = [0], v34 = { 'a': true }, v35;",
    );
    done();
}
