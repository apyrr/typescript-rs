#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_class_member_anonymous_class() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_class_member_anonymous_class(&mut t);
}

fn run_test_code_fix_class_implement_class_member_anonymous_class(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
class A {
    foo() {
        return class { x: number; }
    }
    bar() {
        return new class { x: number; }
    }
}
class C implements A {[| |]}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_not_available(t, &[]);
    done();
}
