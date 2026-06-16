#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_inside_multiline_string() {
    let mut t = TestingT;
    run_test_smart_indent_inside_multiline_string(&mut t);
}

fn run_test_smart_indent_inside_multiline_string(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"window.onload = () => {
    var el = document.getElementById('content\
sometext/*1*/');
    var greeter = new Greeter(el);
    greeter.start();
};

var x = "line1\
line2\
lin/*2*/e3\
line4";

function foo1() {
    function foo2() {
        function foo3() {
            'line1\
lin/*3*/e2';
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_indentation(t, 0);
    f.go_to_marker(t, "2");
    f.verify_indentation(t, 0);
    f.go_to_marker(t, "3");
    f.verify_indentation(t, 0);
    done();
}
