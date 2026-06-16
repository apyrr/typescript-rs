#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_replace_tabs_with_spaces() {
    let mut t = TestingT;
    run_test_formatting_replace_tabs_with_spaces(&mut t);
}

fn run_test_formatting_replace_tabs_with_spaces(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace Foo {
/*1*/				class Test { }
/*2*/			class Test { }
/*3*/class Test { }
/*4*/			 class Test { }
/*5*/   class Test { }
/*6*/    class Test { }
/*7*/     class Test { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    class Test { }");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "    class Test { }");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    class Test { }");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "    class Test { }");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "    class Test { }");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    class Test { }");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "    class Test { }");
    done();
}
