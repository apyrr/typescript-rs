#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_method_01() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_method_01(&mut t);
}

fn run_test_go_to_implementation_interface_method_01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Foo {
    hel/*declaration*/lo(): void;
    okay?: number;
}

class Bar implements Foo {
    [|hello|]() {}
    public sure() {}
}

function whatever(a: Foo) {
    a.he/*function_call*/llo();
}

whatever(new Bar());";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(
        t,
        &["function_call".to_string(), "declaration".to_string()],
    );
    done();
}
