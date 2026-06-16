#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_indentation_in_jsx3() {
    let mut t = TestingT;
    run_test_indentation_in_jsx3(&mut t);
}

fn run_test_indentation_in_jsx3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@Filename: file.tsx
function foo() {
   return (
        <div>
hello
goodbye
        </div>
    )
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_current_file_content(
        t,
        r"function foo() {
   return (
        <div>
hello
goodbye
        </div>
    )
}",
    );
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"function foo() {
    return (
        <div>
            hello
            goodbye
        </div>
    )
}",
    );
    done();
}
