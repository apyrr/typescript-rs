#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_generic_object_base_type() {
    let mut t = TestingT;
    run_test_generic_object_base_type(&mut t);
}

fn run_test_generic_object_base_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @strict: false
class C<T> {
    constructor(){}
    foo(a: T) {
        return a.toString();
    }
}
var x = new C<string>();
var y: string = x.foo("hi");
/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_no_errors();
    done();
}
