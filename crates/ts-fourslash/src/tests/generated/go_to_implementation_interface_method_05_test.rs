#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_method_05() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_method_05(&mut t);
}

fn run_test_go_to_implementation_interface_method_05(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationInterfaceMethod_05") {
        return;
    }
    let content = r"interface Foo {
    hello (): void;
}

class SuperBar implements Foo {
    [|hello|]() {}
}

class Bar extends SuperBar {
    hello2() {}
}

class OtherBar extends SuperBar {
    hello() {}
    hello2() {}
    hello3() {}
}

class NotRelatedToBar {
    hello() {}         // Equivalent to last case, but shares no common ancestors with Bar and so is not returned
    hello2() {}
    hello3() {}
}

class NotBar extends SuperBar {
    hello() {}         // Should not be returned because it is not structurally equivalent to Bar
}

function whatever(x: Bar) {
    x.he/*function_call*/llo()
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["function_call".to_string()]);
    done();
}
