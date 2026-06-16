#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_property_override_access4() {
    let mut t = TestingT;
    run_test_code_fix_property_override_access4(&mut t);
}

fn run_test_code_fix_property_override_access4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: true
// @target: esnext
// @lib: esnext
const prop = Symbol.for('foo');

class A {
    [prop] = 1;
}
class B extends A {
    get [prop]() { return 2; }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &vec!["fixPropertyOverrideAccessor".to_string()]);
    done();
}
