#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_private_identifier_in_type_reference_no_crash1() {
    let mut t = TestingT;
    run_test_quick_info_private_identifier_in_type_reference_no_crash1(&mut t);
}

fn run_test_quick_info_private_identifier_in_type_reference_no_crash1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoPrivateIdentifierInTypeReferenceNoCrash1") {
        return;
    }
    let content = r#"// @target: esnext
class Foo {
  #prop: string = "";

  method() {
    const test: Foo.#prop/*1*/ = "";
  }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "", "");
    done();
}
