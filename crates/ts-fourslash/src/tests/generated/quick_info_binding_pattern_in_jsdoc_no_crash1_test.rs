#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_binding_pattern_in_jsdoc_no_crash1() {
    let mut t = TestingT;
    run_test_quick_info_binding_pattern_in_jsdoc_no_crash1(&mut t);
}

fn run_test_quick_info_binding_pattern_in_jsdoc_no_crash1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoBindingPatternInJsdocNoCrash1") {
        return;
    }
    let content = r"/** @type {({ /*1*/data: any }?) => { data: string[] }} */
function useQuery({ data }): { data: string[] } {
  return {
    data,
  };
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "", "");
    done();
}
