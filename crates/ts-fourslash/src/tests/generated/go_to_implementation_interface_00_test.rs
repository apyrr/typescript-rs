#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_00() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_00(&mut t);
}

fn run_test_go_to_implementation_interface_00(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Fo/*interface_definition*/o {
    hello: () => void
}

interface Baz extends Foo {}

var bar: Foo = [|{|"parts": ["(","object literal",")"], "kind": "interface"|}{ hello: helloImpl /**0*/ }|];
var baz: Foo[] = [|[{ hello: helloImpl /**4*/ }]|];

function helloImpl () {}

function whatever(x: Foo = [|{|"parts": ["(","object literal",")"], "kind": "interface"|}{ hello() {/**1*/} }|] ) {
}

class Bar {
    x: Foo = [|{ hello() {/*2*/} }|]

    constructor(public f: Foo = [|{ hello() {/**3*/} }|] ) {}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["interface_definition".to_string()]);
    done();
}
