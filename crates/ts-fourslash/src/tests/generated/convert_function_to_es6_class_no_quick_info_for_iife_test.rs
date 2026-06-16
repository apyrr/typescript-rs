#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_convert_function_to_es6_class_no_quick_info_for_iife() {
    let mut t = TestingT;
    run_test_convert_function_to_es6_class_no_quick_info_for_iife(&mut t);
}

fn run_test_convert_function_to_es6_class_no_quick_info_for_iife(t: &mut TestingT) {
    if should_skip_if_failing("TestConvertFunctionToEs6Class_noQuickInfoForIIFE") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: /a.js
(/*1*/function () {
   const foo = () => {
        this.x = 10;
   };
   foo;
})();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_code_fix_not_available(t, &[]);
    done();
}
