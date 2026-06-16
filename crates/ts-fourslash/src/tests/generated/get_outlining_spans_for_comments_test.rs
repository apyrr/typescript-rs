#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_spans_for_comments() {
    let mut t = TestingT;
    run_test_get_outlining_spans_for_comments(&mut t);
}

fn run_test_get_outlining_spans_for_comments(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOutliningSpansForComments") {
        return;
    }
    let content = r#"// @lib: es5
[|/*
    Block comment at the beginning of the file before module:
        line one of the comment
        line two of the comment
        line three
        line four
        line five
*/|]
declare module "m";
[|// Single line comments at the start of the file
// line 2
// line 3
// line 4|]
declare module "n";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_outlining_spans_from_ranges(t);
    done();
}
