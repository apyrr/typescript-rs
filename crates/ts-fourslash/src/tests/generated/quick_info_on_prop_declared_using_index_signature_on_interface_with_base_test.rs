#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_prop_declared_using_index_signature_on_interface_with_base() {
    let mut t = TestingT;
    run_test_quick_info_on_prop_declared_using_index_signature_on_interface_with_base(&mut t);
}

fn run_test_quick_info_on_prop_declared_using_index_signature_on_interface_with_base(
    t: &mut TestingT,
) {
    if should_skip_if_failing("TestQuickInfoOnPropDeclaredUsingIndexSignatureOnInterfaceWithBase") {
        return;
    }
    let content = r"interface P {}
interface B extends P {
  [k: string]: number;
}
declare const b: B;
b.t/*1*/est = 10;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(index) B[string]: number", "");
    done();
}
