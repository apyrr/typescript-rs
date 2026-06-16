#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_method_02() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_method_02(&mut t);
}

fn run_test_go_to_implementation_interface_method_02(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Foo {
    he/*declaration*/llo(): void
}

abstract class AbstractBar implements Foo {
    abstract hello(): void;
}

class Bar extends AbstractBar {
    [|hello|]() {}
}

function whatever(a: AbstractBar) {
    a.he/*function_call*/llo();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(
        t,
        &["function_call".to_string(), "declaration".to_string()],
    );
    done();
}
