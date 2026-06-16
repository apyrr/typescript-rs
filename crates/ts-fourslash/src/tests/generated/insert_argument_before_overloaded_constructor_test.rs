#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_insert_argument_before_overloaded_constructor() {
    let mut t = TestingT;
    run_test_insert_argument_before_overloaded_constructor(&mut t);
}

fn run_test_insert_argument_before_overloaded_constructor(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"alert(/**/100);

class OverloadedMonster {
    constructor();
    constructor(name) { }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "'1', ");
    done();
}
