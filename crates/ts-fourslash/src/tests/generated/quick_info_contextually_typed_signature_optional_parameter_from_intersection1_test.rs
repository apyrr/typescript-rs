#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_contextually_typed_signature_optional_parameter_from_intersection1() {
    let mut t = TestingT;
    run_test_quick_info_contextually_typed_signature_optional_parameter_from_intersection1(&mut t);
}

fn run_test_quick_info_contextually_typed_signature_optional_parameter_from_intersection1(
    t: &mut TestingT,
) {
    skip_if_failing(t);
    let content = r"// @strict: true
const optionals: ((a?: number) => unknown) & ((b?: string) => unknown) = (
  arg,
) =/**/> {};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "",
        "function(arg: string | number | undefined): void",
        "",
    );
    done();
}
