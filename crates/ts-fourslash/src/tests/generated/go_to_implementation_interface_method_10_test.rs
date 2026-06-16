#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_method_10() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_method_10(&mut t);
}

fn run_test_go_to_implementation_interface_method_10(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationInterfaceMethod_10") {
        return;
    }
    let content = r"interface BaseFoo {
	 hello(): void;
}

interface Foo extends BaseFoo {
	 aloha(): void;
}

interface Bar {
 	 hello(): void;
 	 goodbye(): void;
}

class FooImpl implements Foo {
 	 [|hello|]() {/**FooImpl*/}
 	 aloha() {}
}

class BaseFooImpl implements BaseFoo {
 	 hello() {/**BaseFooImpl*/}    // Should not show up
}

class BarImpl implements Bar {
	 [|hello|]() {/**BarImpl*/}
	 goodbye() {}
}

class FooAndBarImpl implements Foo, Bar {
	 [|hello|]() {/**FooAndBarImpl*/}
	 aloha() {}
	 goodbye() {}
}

function someFunction(x: Foo | Bar) {
	 x.he/*function_call0*/llo();
}

function anotherFunction(x: Foo & Bar) {
	 x.he/*function_call1*/llo();
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(
        t,
        &["function_call0".to_string(), "function_call1".to_string()],
    );
    done();
}
