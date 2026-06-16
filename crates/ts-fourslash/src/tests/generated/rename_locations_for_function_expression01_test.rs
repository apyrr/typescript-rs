#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_locations_for_function_expression01() {
    let mut t = TestingT;
    run_test_rename_locations_for_function_expression01(&mut t);
}

fn run_test_rename_locations_for_function_expression01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var x = [|function [|{| "contextRangeIndex": 0 |}f|](g: any, h: any) {
    [|f|]([|f|], g);
}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "f");
    done();
}
