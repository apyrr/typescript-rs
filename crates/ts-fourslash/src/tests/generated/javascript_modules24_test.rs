#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_javascript_modules24() {
    let mut t = TestingT;
    run_test_javascript_modules24(&mut t);
}

fn run_test_javascript_modules24(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: mod.ts
function foo() { return 42; }
namespace foo {
  export function bar (a: string) { return a; }
}
export = foo;
// @Filename: app.ts
import * as foo from "./mod"
foo/*1*/();
foo.bar(/*2*/"test");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_error_exists_before_marker(&f.marker_by_name("1"), 0);
    f.verify_quick_info_is(
        t,
        "(alias) function foo(): number\n(alias) namespace foo\nimport foo",
        "",
    );
    f.go_to_marker(t, "2");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: None,
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
