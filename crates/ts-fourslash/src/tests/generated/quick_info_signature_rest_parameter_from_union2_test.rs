#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_signature_rest_parameter_from_union2() {
    let mut t = TestingT;
    run_test_quick_info_signature_rest_parameter_from_union2(&mut t);
}

fn run_test_quick_info_signature_rest_parameter_from_union2(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoSignatureRestParameterFromUnion2") {
        return;
    }
    let content = r#"// @strict: false
declare const rest:
  | ((a?: { a: true }, ...rest: string[]) => unknown)
  | ((b?: { b: true }) => unknown);

/**/rest({ a: true, b: true }, "foo", "bar");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "const rest: (arg0?: {\n    a: true;\n} & {\n    b: true;\n}, ...rest: string[]) => unknown", "");
    done();
}
