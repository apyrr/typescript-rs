#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_object_binding_pattern_rest_element_with_property_name() {
    let mut t = TestingT;
    run_test_format_object_binding_pattern_rest_element_with_property_name(&mut t);
}

fn run_test_format_object_binding_pattern_rest_element_with_property_name(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const { ...a: b } = {};";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(t, r"const { ...a: b } = {};");
    done();
}
