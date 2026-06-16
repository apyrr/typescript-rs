#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_edit_invocation_expression_above_interface_declaration() {
    let mut t = TestingT;
    run_test_incremental_edit_invocation_expression_above_interface_declaration(&mut t);
}

fn run_test_incremental_edit_invocation_expression_above_interface_declaration(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @lib: es5
declare function alert(message?: any): void;
/*1*/
interface Foo {
    setISO8601(dString): Date;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.insert(t, "alert(");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("alert(message?: any): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    f.verify_error_exists_after_marker_name("1");
    done();
}
