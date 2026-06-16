#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_property_access_in_write_location5() {
    let mut t = TestingT;
    run_test_quick_info_on_property_access_in_write_location5(&mut t);
}

fn run_test_quick_info_on_property_access_in_write_location5(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnPropertyAccessInWriteLocation5") {
        return;
    }
    let content = r"// @strict: true
interface Serializer {
  set value(v: string | number);
  get value(): string;
}
declare let box: Serializer;
box.value/*1*/ += 10;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) Serializer.value: string | number", "");
    done();
}
