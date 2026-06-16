#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_for_string_literal_import3() {
    let mut t = TestingT;
    run_test_completion_for_string_literal_import3(&mut t);
}

fn run_test_completion_for_string_literal_import3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @filename: /globals.d.ts
declare module "*.css";
// @filename: /a.ts
import * as foo from "/**/";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
