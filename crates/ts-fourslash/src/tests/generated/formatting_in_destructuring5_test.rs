#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_in_destructuring5() {
    let mut t = TestingT;
    run_test_formatting_in_destructuring5(&mut t);
}

fn run_test_formatting_in_destructuring5(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"let a, b;
/*1*/if (false)[a, b] = [1, 2];
/*2*/if (true)        [a, b] = [1, 2];
/*3*/var a = [1, 2, 3].map(num => num) [0];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "if (false) [a, b] = [1, 2];");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "if (true) [a, b] = [1, 2];");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "var a = [1, 2, 3].map(num => num)[0];");
    done();
}
