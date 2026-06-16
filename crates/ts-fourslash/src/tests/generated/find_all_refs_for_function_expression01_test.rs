#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_function_expression01() {
    let mut t = TestingT;
    run_test_find_all_refs_for_function_expression01(&mut t);
}

fn run_test_find_all_refs_for_function_expression01(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsForFunctionExpression01") {
        return;
    }
    let content = r#"// @Filename: file1.ts
var foo = /*1*/function /*2*/foo(a = /*3*/foo(), b = () => /*4*/foo) {
    /*5*/foo(/*6*/foo, /*7*/foo);
}
// @Filename: file2.ts
/// <reference path="file1.ts" />
foo();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
        ],
    );
    done();
}
