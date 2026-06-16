#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_getter_and_setter() {
    let mut t = TestingT;
    run_test_quick_info_for_getter_and_setter(&mut t);
}

fn run_test_quick_info_for_getter_and_setter(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Test {
    constructor() {
        this.value;
    }

    /** Getter text */
    get val/*1*/ue() {
        return this.value;
    }

    /** Setter text */
    set val/*2*/ue(value) {
        this.value = value;
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_quick_info_is(t, "(getter) Test.value: any", "Getter text");
    f.go_to_marker(t, "2");
    f.verify_quick_info_is(t, "(setter) Test.value: any", "Setter text");
    done();
}
