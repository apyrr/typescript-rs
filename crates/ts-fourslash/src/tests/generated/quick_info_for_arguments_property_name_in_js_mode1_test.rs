#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_arguments_property_name_in_js_mode1() {
    let mut t = TestingT;
    run_test_quick_info_for_arguments_property_name_in_js_mode1(&mut t);
}

fn run_test_quick_info_for_arguments_property_name_in_js_mode1(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForArgumentsPropertyNameInJsMode1") {
        return;
    }
    let content = r"// @allowJs: true
// @filename: a.js
const foo = {
    f1: (params) => { }
}

function /*1*/f2(x) {
   foo.f1({ x, arguments: [] });
}

/*2*/f2('');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
