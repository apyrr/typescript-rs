#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_override_modifier18() {
    let mut t = TestingT;
    run_test_code_fix_override_modifier18(&mut t);
}

fn run_test_code_fix_override_modifier18(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @noImplicitOverride: true
class A {
    static foo() {}
}
class B extends A {
    [|static foo() {}|]
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixAddOverrideModifier".to_string()]);
    done();
}
