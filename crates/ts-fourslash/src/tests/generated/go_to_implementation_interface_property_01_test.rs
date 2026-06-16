#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_property_01() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_property_01(&mut t);
}

fn run_test_go_to_implementation_interface_property_01(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToImplementationInterfaceProperty_01") {
        return;
    }
    let content = r"interface Foo { hello: number }

class Bar implements Foo {
    [|hello|] = 5 * 9;
}

function whatever(foo: Foo) {
    foo.he/*reference*/llo;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["reference".to_string()]);
    done();
}
