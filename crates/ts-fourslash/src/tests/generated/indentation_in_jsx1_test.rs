#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_indentation_in_jsx1() {
    let mut t = TestingT;
    run_test_indentation_in_jsx1(&mut t);
}

fn run_test_indentation_in_jsx1(t: &mut TestingT) {
    if should_skip_if_failing("TestIndentationInJsx1") {
        return;
    }
    let content = r"//@Filename: file.tsx
(function () {
    return (
        <div>
            <div>
            </div>
            /*indent2*/
        </div>
    )
})";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "indent2");
    f.verify_indentation(t, 12);
    done();
}
