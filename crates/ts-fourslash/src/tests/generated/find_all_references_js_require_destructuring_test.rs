#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_references_js_require_destructuring() {
    let mut t = TestingT;
    run_test_find_all_references_js_require_destructuring(&mut t);
}

fn run_test_find_all_references_js_require_destructuring(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @allowJs: true
// @noEmit: true
// @checkJs: true
// @Filename: foo.js
module.exports = {
    foo: '1'
};
// @Filename: bar.js
const { /*1*/foo: bar } = require('./foo');";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
