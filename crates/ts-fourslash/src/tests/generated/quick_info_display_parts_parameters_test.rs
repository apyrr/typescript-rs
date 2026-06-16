#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_parameters() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_parameters(&mut t);
}

fn run_test_quick_info_display_parts_parameters(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoDisplayPartsParameters") {
        return;
    }
    let content = r#"/** @return *crunch* */
function /*1*/foo(/*2*/param: string, /*3*/optionalParam?: string, /*4*/paramWithInitializer = "hello", .../*5*/restParam: string[]) {
    /*6*/param = "Hello";
    /*7*/optionalParam = "World";
    /*8*/paramWithInitializer = "Hello";
    /*9*/restParam[0] = "World";
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
