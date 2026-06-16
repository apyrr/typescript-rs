#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_object_literal_open_curly_single_line() {
    let mut t = TestingT;
    run_test_formatting_object_literal_open_curly_single_line(&mut t);
}

fn run_test_formatting_object_literal_open_curly_single_line(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"
let obj1 =
{ x: 10 };

let obj2 =
    // leading trivia
{ y: 10 };
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"
let obj1 =
    { x: 10 };

let obj2 =
    // leading trivia
    { y: 10 };
",
    );
    done();
}
