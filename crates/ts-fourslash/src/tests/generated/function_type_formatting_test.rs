#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_function_type_formatting() {
    let mut t = TestingT;
    run_test_function_type_formatting(&mut t);
}

fn run_test_function_type_formatting(t: &mut TestingT) {
    if should_skip_if_failing("TestFunctionTypeFormatting") {
        return;
    }
    let content = r"var x: () =>           string/**/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, ";");
    f.verify_current_line_content(t, "var x: () => string;");
    done();
}
