#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_after_aligned_function_argument() {
    let mut t = TestingT;
    run_test_smart_indent_after_aligned_function_argument(&mut t);
}

fn run_test_smart_indent_after_aligned_function_argument(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentAfterAlignedFunctionArgument") {
        return;
    }
    let content = r"function foo(bar,
             blah, baz,
             /**/
) { };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_indentation(t, 13);
    done();
}
