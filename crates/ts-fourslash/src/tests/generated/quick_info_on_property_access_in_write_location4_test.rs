#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_property_access_in_write_location4() {
    let mut t = TestingT;
    run_test_quick_info_on_property_access_in_write_location4(&mut t);
}

fn run_test_quick_info_on_property_access_in_write_location4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: true
interface Serializer {
  set value(v: string | number | boolean);
  get value(): string;
}
declare let box: Serializer;
box.value/*1*/ = true;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "(property) Serializer.value: string | number | boolean",
        "",
    );
    done();
}
