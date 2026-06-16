#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_interface_method_11() {
    let mut t = TestingT;
    run_test_go_to_implementation_interface_method_11(&mut t);
}

fn run_test_go_to_implementation_interface_method_11(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Foo {
   hel/*reference*/lo(): void;
}

var x = <Foo> { [|hello|]: () => {} };
var y = <Foo> (((({ [|hello|]: () => {} }))));";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["reference".to_string()]);
    done();
}
