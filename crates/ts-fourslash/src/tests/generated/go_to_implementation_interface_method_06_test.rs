#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_method_06() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_method_06(&mut t);
}

fn run_test_go_to_implementation_interface_method_06(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationInterfaceMethod_06") {
        return;
    }
    let content = r"interface SuperFoo {
    hello (): void;
}

interface Foo extends SuperFoo {
    someOtherFunction(): void;
}

class Bar implements Foo {
     [|hello|]() {}
     someOtherFunction() {}
}

function createFoo(): Foo {
    return {
        [|hello|]() {},
        someOtherFunction() {}
    };
}

var y: Foo = {
    [|hello|]() {},
    someOtherFunction() {}
};

class FooLike implements SuperFoo {
     hello() {}
     someOtherFunction() {}
}

class NotRelatedToFoo {
     hello() {}                // This case is equivalent to the last case, but is not returned because it does not share a common ancestor with Foo
     someOtherFunction() {}
}

class NotFoo implements SuperFoo {
     hello() {}                // We only want implementations of Foo, even though the function is declared in SuperFoo
}

function (x: Foo) {
    x.he/*function_call*/llo()
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["function_call".to_string()]);
    done();
}
