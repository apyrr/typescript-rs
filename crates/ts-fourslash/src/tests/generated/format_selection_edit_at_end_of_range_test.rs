#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_selection_edit_at_end_of_range() {
    let mut t = TestingT;
    run_test_format_selection_edit_at_end_of_range(&mut t);
}

fn run_test_format_selection_edit_at_end_of_range(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/var x = 1;/*2*/
void 0;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    {
        let mut opts = f.get_options();
        opts.format_code_settings.semicolons = lsutil::SemicolonPreference::Remove;
        f.configure(t, opts);
    }
    f.format_selection(t, "1", "2");
    f.verify_current_file_content(
        t,
        r"var x = 1
void 0;",
    );
    done();
}
