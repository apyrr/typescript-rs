#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_method_03() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_method_03(&mut t);
}

fn run_test_go_to_implementation_interface_method_03(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"interface Foo {
    hello (): void;
}

class Bar extends SuperBar {
    [|hello|]() {}
}

class SuperBar implements Foo {
    hello() {} // should not show up
}

class OtherBar implements Foo {
    hello() {} // should not show up
}

new Bar().hel/*function_call*/lo();
new Bar()["hello"]();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["function_call".to_string()]);
    done();
}
