#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_in_object_binding_pattern06() {
    let mut t = TestingT;
    run_test_completion_list_in_object_binding_pattern06(&mut t);
}

fn run_test_completion_list_in_object_binding_pattern06(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    property1: number;
    property2: string;
}

var foo: I;
var { property1, property2, /**/ } = foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("".to_string()), None);
    done();
}
