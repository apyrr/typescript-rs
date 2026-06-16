#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_signature_optional_parameter_from_union1() {
    let mut t = TestingT;
    run_test_quick_info_signature_optional_parameter_from_union1(&mut t);
}

fn run_test_quick_info_signature_optional_parameter_from_union1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoSignatureOptionalParameterFromUnion1") {
        return;
    }
    let content = r"// @strict: false
declare const optionals:
  | ((a?: { a: true }) => unknown)
  | ((b?: { b: true }) => unknown);

/**/optionals();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "",
        "const optionals: (arg0?: {\n    a: true;\n} & {\n    b: true;\n}) => unknown",
        "",
    );
    done();
}
