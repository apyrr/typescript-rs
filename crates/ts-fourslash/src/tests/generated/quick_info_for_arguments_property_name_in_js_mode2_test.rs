#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_arguments_property_name_in_js_mode2() {
    let mut t = TestingT;
    run_test_quick_info_for_arguments_property_name_in_js_mode2(&mut t);
}

fn run_test_quick_info_for_arguments_property_name_in_js_mode2(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForArgumentsPropertyNameInJsMode2") {
        return;
    }
    let content = r"// @allowJs: true
// @filename: a.js
function /*1*/f(x) {
   arguments;
}

/*2*/f('');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
