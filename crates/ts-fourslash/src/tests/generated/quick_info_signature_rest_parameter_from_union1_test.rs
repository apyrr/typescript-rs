#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_signature_rest_parameter_from_union1() {
    let mut t = TestingT;
    run_test_quick_info_signature_rest_parameter_from_union1(&mut t);
}

fn run_test_quick_info_signature_rest_parameter_from_union1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoSignatureRestParameterFromUnion1") {
        return;
    }
    let content = r#"declare const rest:
  | ((v: { a: true }, ...rest: string[]) => unknown)
  | ((v: { b: true }) => unknown);

/**/rest({ a: true, b: true }, "foo", "bar");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "",
        "const rest: (v: {\n    a: true;\n} & {\n    b: true;\n}, ...rest: string[]) => unknown",
        "",
    );
    done();
}
