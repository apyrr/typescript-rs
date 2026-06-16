#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_undeclared_property_accesses() {
    let mut t = TestingT;
    run_test_code_fix_undeclared_property_accesses(&mut t);
}

fn run_test_code_fix_undeclared_property_accesses(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixUndeclaredPropertyAccesses") {
        return;
    }
    let content = r#"interface I { x: number; }
let i: I;
i.y;
i.foo();
enum E { a,b }
let e: typeof E;
e.a;
e.c;
let obj = { a: 1, b: "asdf"};
obj.c;
type T<U> = I | U;
let t: T<number>;
t.x;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(t, None);
    done();
}
