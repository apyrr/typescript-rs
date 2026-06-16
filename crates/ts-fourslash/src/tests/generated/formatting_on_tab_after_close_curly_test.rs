#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_tab_after_close_curly() {
    let mut t = TestingT;
    run_test_formatting_on_tab_after_close_curly(&mut t);
}

fn run_test_formatting_on_tab_after_close_curly(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace Tools {/*1*/
    export enum NodeType {/*2*/
        Error,/*3*/
        Comment,/*4*/
    }   /*5*/
    export enum foob/*6*/
    {
        Blah=1, Bleah=2/*7*/
    }/*8*/
}/*9*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "namespace Tools {");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    export enum NodeType {");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "        Error,");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "        Comment,");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    export enum foob {");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "        Blah = 1, Bleah = 2");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(t, "    }");
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "}");
    done();
}
