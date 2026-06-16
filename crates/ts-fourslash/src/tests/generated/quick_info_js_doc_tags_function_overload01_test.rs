#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_js_doc_tags_function_overload01() {
    let mut t = TestingT;
    run_test_quick_info_js_doc_tags_function_overload01(&mut t);
}

fn run_test_quick_info_js_doc_tags_function_overload01(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoJsDocTagsFunctionOverload01") {
        return;
    }
    let content = r"// @Filename: quickInfoJsDocTagsFunctionOverload01.ts
/**
 * Doc foo
 */
declare function /*1*/foo(): void;

/**
 * Doc foo overloaded
 * @tag Tag text
 */
declare function /*2*/foo(x: number): void";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
