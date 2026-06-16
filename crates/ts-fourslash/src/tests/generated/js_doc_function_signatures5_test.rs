#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_signatures5() {
    let mut t = TestingT;
    run_test_js_doc_function_signatures5(&mut t);
}

fn run_test_js_doc_function_signatures5(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionSignatures5") {
        return;
    }
    let content = r"// @strict: true
// @allowJs: true
// @Filename: Foo.js
/**
 * Filters a path based on a regexp or glob pattern.
 * @param {String} basePath The base path where the search will be performed.
 * @param {String} pattern A string defining a regexp of a glob pattern.
 * @param {String} type The search pattern type, can be a regexp or a glob.
 * @param {Object} options A object containing options to the search.
 * @return {Array} A list containing the filtered paths.
 */
function pathFilter(basePath, pattern, type, options){
//...
}
pathFilter(/**/'foo', 'bar', 'baz', {});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
