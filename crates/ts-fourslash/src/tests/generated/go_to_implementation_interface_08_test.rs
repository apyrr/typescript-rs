#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_08() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_08(&mut t);
}

fn run_test_go_to_implementation_interface_08(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Base {
    hello (): void;
}

interface A extends Base {}
interface B extends C, A {}
interface C extends B, A {}

class X implements B {
    [|hello|]() {}
}

function someFunction(d : A) {
    d.he/*function_call*/llo();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["function_call".to_string()]);
    done();
}
