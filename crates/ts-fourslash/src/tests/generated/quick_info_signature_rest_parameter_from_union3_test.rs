#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_signature_rest_parameter_from_union3() {
    let mut t = TestingT;
    run_test_quick_info_signature_rest_parameter_from_union3(&mut t);
}

fn run_test_quick_info_signature_rest_parameter_from_union3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare const fn:
  | ((a: { x: number }, b: { x: number }) => number)
  | ((...a: { y: number }[]) => number);

/**/fn();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "const fn: (a: {\n    x: number;\n} & {\n    y: number;\n}, b: {\n    x: number;\n} & {\n    y: number;\n}, ...args: {\n    y: number;\n}[]) => number", "");
    done();
}
