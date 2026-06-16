#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_fix_exact_optional_unassignable_properties9() {
    let mut t = TestingT;
    run_test_fix_exact_optional_unassignable_properties9(&mut t);
}

fn run_test_fix_exact_optional_unassignable_properties9(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strictNullChecks: true
// @exactOptionalPropertyTypes: true
interface IAny {
    a?: any
}
interface J {
    a?: number | undefined
}
declare var iany: IAny
declare var j: J
iany/**/ = j";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
