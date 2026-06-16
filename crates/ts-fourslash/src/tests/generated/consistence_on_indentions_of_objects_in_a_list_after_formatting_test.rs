#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_consistence_on_indentions_of_objects_in_a_list_after_formatting() {
    let mut t = TestingT;
    run_test_consistence_on_indentions_of_objects_in_a_list_after_formatting(&mut t);
}

fn run_test_consistence_on_indentions_of_objects_in_a_list_after_formatting(t: &mut TestingT) {
    if should_skip_if_failing("TestConsistenceOnIndentionsOfObjectsInAListAfterFormatting") {
        return;
    }
    let content = r"foo({
}, {/*1*/
});/*2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "}, {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "});");
    done();
}
