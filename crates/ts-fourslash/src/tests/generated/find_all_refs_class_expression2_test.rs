#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_class_expression2() {
    let mut t = TestingT;
    run_test_find_all_refs_class_expression2(&mut t);
}

fn run_test_find_all_refs_class_expression2(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsClassExpression2") {
        return;
    }
    let content = r#"// @allowJs: true
// @Filename: /a.js
exports./*0*/A = class {};
// @Filename: /b.js
import { /*1*/A } from "./a";
/*2*/A;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["0".to_string(), "1".to_string(), "2".to_string()]);
    done();
}
