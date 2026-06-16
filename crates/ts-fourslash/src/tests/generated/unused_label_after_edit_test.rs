#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_label_after_edit() {
    let mut t = TestingT;
    run_test_unused_label_after_edit(&mut t);
}

fn run_test_unused_label_after_edit(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedLabelAfterEdit") {
        return;
    }
    let content = r"// @allowUnusedLabels: false
myLabel: while (true) {
    if (Math.random() > 0.5) {
        /*marker*/break myLabel;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_number_of_errors_in_current_file(0);
    f.go_to_marker(t, "marker");
    f.delete_at_caret(t, 14);
    f.insert(t, "break;");
    f.verify_number_of_errors_in_current_file(1);
    f.go_to_marker(t, "marker");
    f.delete_at_caret(t, 6);
    f.insert(t, "break myLabel;");
    f.verify_number_of_errors_in_current_file(0);
    done();
}
