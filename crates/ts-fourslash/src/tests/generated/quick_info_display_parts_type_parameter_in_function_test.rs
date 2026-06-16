#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_type_parameter_in_function() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_type_parameter_in_function(&mut t);
}

fn run_test_quick_info_display_parts_type_parameter_in_function(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function /*1*/foo</*2*/U>(/*3*/a: /*4*/U) {
    return /*5*/a;
}
/*6*/foo("Hello");
function /*7*/foo2</*8*/U extends string>(/*9*/a: /*10*/U) {
    return /*11*/a;
}
/*12*/foo2("hello");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
