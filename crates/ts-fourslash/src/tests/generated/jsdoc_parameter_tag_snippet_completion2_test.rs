#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_parameter_tag_snippet_completion2() {
    let mut t = TestingT;
    run_test_jsdoc_parameter_tag_snippet_completion2(&mut t);
}

fn run_test_jsdoc_parameter_tag_snippet_completion2(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocParameterTagSnippetCompletion2") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: a.ts
/**
 * /*b*/
 */
function bb(b: string) {}
// @Filename: b.js
/**
 * /*jb*/
 */
function bb(b) {}

/**
 * 
 * @p/*jc*/
 */
function cc({ b: { a, c } = { a: 1, c: 3 } }) {

}

/**
 * 
 * @p/*jd*/
 */
function dd(...a) {}

/**
 * @p/*z*/
 */
function zz(a = 3) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
