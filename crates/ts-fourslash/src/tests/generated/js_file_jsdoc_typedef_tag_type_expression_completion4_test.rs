#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_file_jsdoc_typedef_tag_type_expression_completion4() {
    let mut t = TestingT;
    run_test_js_file_jsdoc_typedef_tag_type_expression_completion4(&mut t);
}

fn run_test_js_file_jsdoc_typedef_tag_type_expression_completion4(t: &mut TestingT) {
    if should_skip_if_failing("TestJsFileJsdocTypedefTagTypeExpressionCompletion4") {
        return;
    }
    let content = r"// @allowJs: true
// @filename: /a.js
const foo = {
    bar: {
        baz: 42,
    }
}
/** @typedef { typeof foo./**/ } Foo */";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
