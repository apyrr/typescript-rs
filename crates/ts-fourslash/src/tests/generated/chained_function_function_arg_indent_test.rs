#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_chained_function_function_arg_indent() {
    let mut t = TestingT;
    run_test_chained_function_function_arg_indent(&mut t);
}

fn run_test_chained_function_function_arg_indent(t: &mut TestingT) {
    if should_skip_if_failing("TestChainedFunctionFunctionArgIndent") {
        return;
    }
    let content = r#"declare var $: any;
$(".contentDiv").each(function (index, element) {/**/
    // <-- ensure cursor is here after return on above
});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "\n");
    f.verify_indentation(t, 4);
    f.insert(t, "}");
    f.verify_indentation(t, 4);
    // keep arguments indented
    done();
}
