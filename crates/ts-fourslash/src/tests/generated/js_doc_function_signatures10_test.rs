#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_signatures10() {
    let mut t = TestingT;
    run_test_js_doc_function_signatures10(&mut t);
}

fn run_test_js_doc_function_signatures10(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionSignatures10") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: Foo.js
/**
 * Do some foo things
 * @template T A Foolish template
 * @param {T} x a parameter
 */
function foo(x) {
}

fo/**/o()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(t, "function foo<any>(x: any): void", "Do some foo things");
    done();
}
