#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_after_property_name_in_class() {
    let mut t = TestingT;
    run_test_completions_after_property_name_in_class(&mut t);
}

fn run_test_completions_after_property_name_in_class(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @filename: /a.js
class C1 {
    async #fo/*a*/
}
class C2 {
    async fo/*b*/
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
