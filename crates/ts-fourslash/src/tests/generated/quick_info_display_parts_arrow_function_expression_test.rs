#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_arrow_function_expression() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_arrow_function_expression(&mut t);
}

fn run_test_quick_info_display_parts_arrow_function_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsArrowFunctionExpression") {
        return;
    }
    let content = r"var /*1*/x = /*5*/a => 10;
var /*2*/y = (/*6*/a, /*7*/b) => 10;
var /*3*/z = (/*8*/a: number) => 10;
var /*4*/z2 = () => 10;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
