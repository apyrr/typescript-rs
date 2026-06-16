#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_variable_assignments() {
    let mut t = TestingT;
    run_test_format_variable_assignments(&mut t);
}

fn run_test_format_variable_assignments(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatVariableAssignments") {
        return;
    }
    let content = r"let t: number;
t
/*nextlineWithEqual*/=2+2;
t=
/*nextlineWithoutEqual*/2
/*nextline2*/+2;
t
/*addition*/+= 22
/*nextlineSemicolon*/;
t
=t
/*chained*/=t+ 4;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "nextlineWithEqual");
    f.verify_indentation(t, 4);
    f.verify_current_line_content(t, "    = 2 + 2;");
    f.go_to_marker(t, "nextlineWithoutEqual");
    f.verify_indentation(t, 4);
    f.verify_current_line_content(t, "    2");
    f.go_to_marker(t, "nextline2");
    f.verify_indentation(t, 4);
    f.verify_current_line_content(t, "    + 2;");
    f.go_to_marker(t, "addition");
    f.verify_indentation(t, 4);
    f.verify_current_line_content(t, "    += 22");
    f.go_to_marker(t, "nextlineSemicolon");
    f.verify_indentation(t, 4);
    f.verify_current_line_content(t, "    ;");
    f.go_to_marker(t, "chained");
    f.verify_indentation(t, 4);
    f.verify_current_line_content(t, "    = t + 4;");
    done();
}
