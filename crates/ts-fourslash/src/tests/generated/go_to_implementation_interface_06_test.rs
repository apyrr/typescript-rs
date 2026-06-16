#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_06() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_06(&mut t);
}

fn run_test_go_to_implementation_interface_06(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationInterface_06") {
        return;
    }
    let content = r"interface Fo/*interface_definition*/o {
    new (a: number): SomeOtherType;
}

interface SomeOtherType {}

let x: Foo = [|class { constructor (a: number) {} }|];
let y = <Foo> [|class { constructor (a: number) {} }|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["interface_definition".to_string()]);
    done();
}
