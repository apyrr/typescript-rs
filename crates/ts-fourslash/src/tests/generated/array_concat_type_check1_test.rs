#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_array_concat_type_check1() {
    let mut t = TestingT;
    run_test_array_concat_type_check1(&mut t);
}

fn run_test_array_concat_type_check1(t: &mut TestingT) {
    if should_skip_if_failing("TestArrayConcatTypeCheck1") {
        return;
    }
    let content = r#"a.concat(/*2*/"hello"/*1*/, 'world');

a.concat(/*3*/'Hello');

var b = new Array/*4*/<>();
b.concat('hello');
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.delete_at_caret(t, 9);
    f.go_to_marker(t, "3");
    f.delete_at_caret(t, 7);
    f.go_to_marker(t, "2");
    f.delete_at_caret(t, 7);
    f.go_to_marker(t, "4");
    done();
}
