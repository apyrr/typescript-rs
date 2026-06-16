#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_method_00() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_method_00(&mut t);
}

fn run_test_go_to_implementation_interface_method_00(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationInterfaceMethod_00") {
        return;
    }
    let content = r#"interface Foo {
    he/*declaration*/llo: () => void
}

var bar: Foo = { [|hello|]: helloImpl };
var baz: Foo = { "[|hello|]": helloImpl };

function helloImpl () {}

function whatever(x: Foo = { [|hello|]() {/**1*/} }) {
    x.he/*function_call*/llo()
}

class Bar {
    x: Foo = { [|hello|]() {/*2*/} }

    constructor(public f: Foo = { [|hello|]() {/**3*/} } ) {}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(
        t,
        &["function_call".to_string(), "declaration".to_string()],
    );
    done();
}
