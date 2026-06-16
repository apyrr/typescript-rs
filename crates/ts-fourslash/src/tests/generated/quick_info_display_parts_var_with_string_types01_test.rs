#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_var_with_string_types01() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_var_with_string_types01(&mut t);
}

fn run_test_quick_info_display_parts_var_with_string_types01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"let /*1*/hello: "hello" | 'hello' = "hello";
let /*2*/world: 'world' = "world";
let /*3*/helloOrWorld: "hello" | 'world';"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
