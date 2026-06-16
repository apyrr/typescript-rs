#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_empty_interface_literal() {
    let mut t = TestingT;
    run_test_formatting_on_empty_interface_literal(&mut t);
}

fn run_test_formatting_on_empty_interface_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/    function    foo  (  x  :    {    }    )    {    }

/*2*/foo    (  {     }   )    ;



/*3*/            interface    bar    {
/*4*/                x   :    {     }   ;
/*5*/       y  :       (         )    =>    {     }   ;
/*6*/                                                    }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "function foo(x: {}) { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "foo({});");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "interface bar {");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    x: {};");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    y: () => {};");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "}");
    done();
}
