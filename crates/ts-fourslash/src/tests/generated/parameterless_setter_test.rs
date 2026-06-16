#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_parameterless_setter() {
    let mut t = TestingT;
    run_test_parameterless_setter(&mut t);
}

fn run_test_parameterless_setter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class foo {
    get getterOnly() {
        return undefined;
    }
    set setterOnly() { }
}
var obj = new foo();
obj.setterOnly = obj./**/getterOnly;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    done();
}
