#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_04() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_04(&mut t);
}

fn run_test_go_to_implementation_interface_04(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationInterface_04") {
        return;
    }
    let content = r"interface Fo/*interface_definition*/o {
    (a: number): void
}

var bar: Foo = [|(a) => {/**0*/}|];

function whatever(x: Foo = [|(a) => {/**1*/}|] ) {
}

class Bar {
    x: Foo = [|(a) => {/**2*/}|]

    constructor(public f: Foo = [|function(a) {}|] ) {}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["interface_definition".to_string()]);
    done();
}
