#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_conditional_types() {
    let mut t = TestingT;
    run_test_formatting_conditional_types(&mut t);
}

fn run_test_formatting_conditional_types(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingConditionalTypes") {
        return;
    }
    let content = r"/*L1*/type Diff1<T, U> = T extends U?never:T;
/*L2*/type Diff2<T, U> = T    extends    U  ?    never   :     T;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "L1");
    f.verify_current_line_content(t, "type Diff1<T, U> = T extends U ? never : T;");
    f.go_to_marker(t, "L2");
    f.verify_current_line_content(t, "type Diff2<T, U> = T extends U ? never : T;");
    done();
}
