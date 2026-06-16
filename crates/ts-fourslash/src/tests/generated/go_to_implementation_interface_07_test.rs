#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_07() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_07(&mut t);
}

fn run_test_go_to_implementation_interface_07(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Fo/*interface_definition*/o {
    hello (): void;
}

interface Bar {
    hello (): void;
}

let x1: Foo            = [|{ hello ()          { /**typeReference*/ } }|];
let x2: () => Foo      = [|(() => { hello ()   { /**functionType*/} })|];
let x3: Foo | Bar      = [|{ hello ()          { /**unionType*/} }|];
let x4: Foo & (Foo & Bar)      = [|{ hello ()          { /**intersectionType*/} }|];
let x5: [Foo]          = [|[{ hello ()         { /**tupleType*/} }]|];
let x6: (Foo)          = [|{ hello ()          { /**parenthesizedType*/} }|];
let x7: (new() => Foo) = [|class { hello ()    { /**constructorType*/} }|];
let x8: Foo[]          = [|[{ hello ()         { /**arrayType*/} }]|];
let x9: { y: Foo }     = [|{ y: { hello ()     { /**typeLiteral*/} } }|];
let x10 = [|{|"parts": ["(","anonymous local class",")"], "kind": "local class"|}class implements Foo { hello() {} }|]
let x11 = class [|{|"parts": ["(","local class",")"," ","C"], "kind": "local class"|}C|] implements Foo { hello() {} }

// Should not do anything for type predicates
function isFoo(a: any): a is Foo {
    return true;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["interface_definition".to_string()]);
    done();
}
