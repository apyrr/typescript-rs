#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_parameter_tag_snippet_completion3() {
    let mut t = TestingT;
    run_test_jsdoc_parameter_tag_snippet_completion3(&mut t);
}

fn run_test_jsdoc_parameter_tag_snippet_completion3(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocParameterTagSnippetCompletion3") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: a.js
/**
 * @p/*z*/
 */
function zz(a = 3) {}
/**
 * @p/*y*/
 */
function yy({ a = 3 }) {}
/**
 * @p/*x*/
 */
function xx({ a, o: { b, c: [d, e = 1] }}) {}
/**
 * @p/*w*/
 */
function ww({ a, o: { b, c: [d, e] = [1, true] }}) {}
/**
 * @p/*v*/
 */
function vv({ a = [1, true] }) {}
function random(a) { return a }
/**
 * @p/*u*/
 */
function uu({ a = random() }) {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
