#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_selection_jsx_with_binary_expression() {
    let mut t = TestingT;
    run_test_format_selection_jsx_with_binary_expression(&mut t);
}

fn run_test_format_selection_jsx_with_binary_expression(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"//@Filename: file.tsx
function TestWidget() {
    const test = true;
    return (
        <div>
            {test &&
                <div>
 /*1*/                <div>some text</div>/*2*/
                    <div>some text</div>
                    <div>some text</div>
                </div>
            }
            <div>some text</div>
        </div>
    );
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "1", "2");
    f.verify_current_file_content(
        t,
        r"function TestWidget() {
    const test = true;
    return (
        <div>
            {test &&
                <div>
                    <div>some text</div>
                    <div>some text</div>
                    <div>some text</div>
                </div>
            }
            <div>some text</div>
        </div>
    );
}",
    );
    done();
}
